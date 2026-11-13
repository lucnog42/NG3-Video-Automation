#!/usr/bin/env python3
"""Biblioteca de mídia: todo o acervo da marca num catálogo só, com fila de revisão e tags.

Junta, num `midia.sqlite` em dados/marcas/<marca>/, cada arquivo de todas as fontes:
posts publicados, pastas de cortes e as pastas apontadas na aba Importar. Nada é copiado
nem escrito nas pastas: o catálogo guarda o caminho (relativo ao acervo, ou absoluto numa
pasta apontada) e as anotações.

  * mídia nova de uma pasta importada entra na **fila de revisão** (`novo`); o que já está
    no acervo publicado (posts, cortes) entra como `revisado`;
  * cada subpasta da pasta importada vira tag com origem `pasta`;
  * arquivo movido dentro da fonte é reconhecido pelo hash rápido (tamanho + começo + fim)
    e só troca de caminho, sem perder as tags;
  * arquivo que sumiu (disco desmontado, apagado fora do app) fica marcado `ausente`, nunca
    apagado do catálogo: as tags voltam quando ele reaparece.

Tags vêm de config/marcas/<marca>/tags.toml (categorias da marca) + tags livres + tags de
pasta. O `slug` não muda ao renomear. Os cortes (tabela `corte`) ficam para a próxima fase.

    python3 src/acervo/biblioteca.py sincronizar
    python3 src/acervo/biblioteca.py resumo
"""
from __future__ import annotations

import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))))  # src/

import argparse
import hashlib
import json
import os
import re
import sqlite3
import time
import unicodedata

from nucleo import caminhos, config  # noqa: E402

VIDEO = (".mp4", ".mov", ".m4v", ".mkv", ".webm", ".avi", ".mts")
IMAGEM = (".jpg", ".jpeg", ".png", ".webp", ".heic", ".tif", ".tiff")
PEDACO = 64 * 1024        # bytes lidos do começo e do fim para o hash rápido
FILA = ("novo", "indexado")   # status que ainda esperam revisão

ESQUEMA = """
create table if not exists importacao (
    id integer primary key, fonte_id text, caminho text, data real, n integer default 0);
create table if not exists midia (
    id integer primary key, fonte_id text not null, importacao_id integer, caminho text not null,
    item text default '', subpasta text default '', tipo text not null, tamanho integer default 0,
    hash_rapido text default '', status_import text not null default 'novo', pilar text default '',
    data text default '', ausente integer default 0, criado_em real, revisado_em real,
    unique (fonte_id, caminho));
create index if not exists midia_status on midia (status_import);
create index if not exists midia_hash on midia (hash_rapido);
create table if not exists tag (
    id integer primary key, slug text unique not null, nome text not null, categoria text default '',
    origem text not null);
create table if not exists midia_tag (
    midia_id integer not null, tag_id integer not null, por text not null,
    primary key (midia_id, tag_id));
"""


def caminho_db() -> str:
    return caminhos.dados("midia.sqlite")


def abrir(db: str | None = None) -> sqlite3.Connection:
    alvo = db or caminho_db()
    os.makedirs(os.path.dirname(alvo), exist_ok=True)
    con = sqlite3.connect(alvo, timeout=30)
    con.row_factory = sqlite3.Row
    con.executescript(ESQUEMA)
    return con


def slug(nome: str) -> str:
    s = unicodedata.normalize("NFKD", nome).encode("ascii", "ignore").decode().lower()
    return re.sub(r"[^a-z0-9]+", "-", s).strip("-")


def hash_rapido(caminho: str) -> str:
    """Tamanho + sha1 dos primeiros e últimos 64 KB: barato no HD USB e basta para achar o
    mesmo arquivo com outro nome ou noutra subpasta."""
    try:
        n = os.path.getsize(caminho)
        h = hashlib.sha1(str(n).encode())
        with open(caminho, "rb") as fh:
            h.update(fh.read(PEDACO))
            if n > 2 * PEDACO:
                fh.seek(-PEDACO, os.SEEK_END)
                h.update(fh.read(PEDACO))
        return f"{n}:{h.hexdigest()[:20]}"
    except OSError:
        return ""


def _tipo(caminho: str) -> str:
    ext = os.path.splitext(caminho)[1].lower()
    return "video" if ext in VIDEO else "imagem" if ext in IMAGEM else ""


# --------------------------------------------------------------- entradas
def _entradas(f: dict) -> list[dict]:
    """Os arquivos de uma fonte, no formato do catálogo."""
    tipo_fonte = f.get("tipo") or "pasta"
    out = []
    if tipo_fonte == "posts":
        from acervo import listas
        for p in listas.montar_dados(limite_fala=0, limite_legenda=0):
            for m in p.get("midias") or []:
                t = _tipo(m)
                if t:
                    out.append({"caminho": m, "tipo": t, "item": p.get("pasta") or "", "subpasta": "",
                                "pilar": p.get("pilar") or "", "data": p.get("data") or ""})
    elif tipo_fonte == "cortes":
        from acervo import listas
        for c in listas.montar_cortes(limite_fala=0):
            for v in c.get("videos") or []:
                out.append({"caminho": v, "tipo": "video", "item": c.get("arquivo") or "", "subpasta": "",
                            "pilar": c.get("pilar") or "", "data": ""})
    else:
        from acervo import fontes as F
        raiz = os.path.expanduser(f.get("raiz") or "")
        for arq, t in F.arquivos({**f, "raiz": raiz}):
            sub = os.path.relpath(os.path.dirname(arq), raiz)
            sub = "" if sub == "." else sub
            ano = re.search(r"(20\d\d)", sub + os.path.basename(arq))
            out.append({"caminho": arq, "tipo": t, "item": sub, "subpasta": sub, "pilar": "",
                        "data": ano.group(1) if ano else ""})
    return out


def _e_importada(f: dict) -> bool:
    return f.get("tipo") not in ("posts", "cortes")


def sincronizar(so: str | None = None, db: str | None = None, fontes_cfg: list[dict] | None = None) -> dict:
    """Põe no catálogo o que apareceu em cada fonte e marca o que sumiu. Devolve, por fonte,
    quantos novos, movidos e ausentes."""
    con = abrir(db)
    agora = time.time()
    feitos = {}
    for f in fontes_cfg if fontes_cfg is not None else (config.carregar(recarregar=True).get("fontes") or []):
        if so and f["id"] != so:
            continue
        if f.get("raiz") and not os.path.isdir(os.path.expanduser(f["raiz"])):
            feitos[f["id"]] = {"novos": 0, "movidos": 0, "ausentes": 0, "erro": "pasta inacessível"}
            continue
        importada = _e_importada(f)
        entradas = _entradas(f)
        vistos = {e["caminho"] for e in entradas}
        ja = {r["caminho"]: r for r in con.execute(
            "select id, caminho, hash_rapido, ausente from midia where fonte_id = ?", (f["id"],))}
        # o que sumiu: candidato a "movido" se reaparecer com o mesmo hash
        sumidos = {r["hash_rapido"]: r for c, r in ja.items() if c not in vistos and r["hash_rapido"]}
        novos = movidos = 0
        imp_id = None
        for e in entradas:
            r = ja.get(e["caminho"])
            if r is not None:
                if r["ausente"]:
                    con.execute("update midia set ausente = 0 where id = ?", (r["id"],))
                continue
            h = hash_rapido(e["caminho"] if os.path.isabs(e["caminho"]) else
                            os.path.join(config.carregar()["acervo"]["raiz"], e["caminho"])) if importada else ""
            antigo = sumidos.pop(h, None) if h else None
            if antigo is not None:
                con.execute("update midia set caminho = ?, subpasta = ?, item = ?, ausente = 0 where id = ?",
                            (e["caminho"], e["subpasta"], e["item"], antigo["id"]))
                movidos += 1
                continue
            if importada and imp_id is None:
                imp_id = con.execute("insert into importacao (fonte_id, caminho, data) values (?, ?, ?)",
                                     (f["id"], f.get("raiz") or "", agora)).lastrowid
            tam = 0
            try:
                tam = os.path.getsize(e["caminho"] if os.path.isabs(e["caminho"]) else
                                      os.path.join(config.carregar()["acervo"]["raiz"], e["caminho"]))
            except OSError:
                pass
            mid = con.execute(
                "insert into midia (fonte_id, importacao_id, caminho, item, subpasta, tipo, tamanho, hash_rapido, "
                "status_import, pilar, data, criado_em, revisado_em) values (?,?,?,?,?,?,?,?,?,?,?,?,?)",
                (f["id"], imp_id, e["caminho"], e["item"], e["subpasta"], e["tipo"], tam, h,
                 "novo" if importada else "revisado", e["pilar"], e["data"], agora,
                 None if importada else agora)).lastrowid
            for parte in [p for p in e["subpasta"].split(os.sep) if p]:
                _marcar(con, mid, _garantir_tag(con, parte, origem="pasta"), "pasta")
            novos += 1
        ausentes = 0
        for c, r in ja.items():
            if c not in vistos and not r["ausente"] and not con.execute(
                    "select 1 from midia where id = ? and caminho != ?", (r["id"], c)).fetchone():
                con.execute("update midia set ausente = 1 where id = ?", (r["id"],))
                ausentes += 1
        if imp_id is not None:
            con.execute("update importacao set n = ? where id = ?", (novos, imp_id))
        feitos[f["id"]] = {"novos": novos, "movidos": movidos, "ausentes": ausentes}
    con.commit()
    con.close()
    return feitos


# --------------------------------------------------------------------- tags
def categorias_marca() -> dict[str, dict]:
    """{slug_categoria: {"nome", "multipla", "tags": {slug: nome}}} de tags.toml."""
    return (config.carregar().get("tags") or {}).get("categorias") or {}


def _da_marca(s: str) -> tuple[str, str] | None:
    """(nome, categoria) se o slug é uma tag da marca."""
    for cat, c in categorias_marca().items():
        if s in (c.get("tags") or {}):
            return c["tags"][s], cat
    return None


def _garantir_tag(con: sqlite3.Connection, nome_ou_slug: str, origem: str = "livre") -> int:
    s = slug(nome_ou_slug)
    if not s:
        raise ValueError("tag vazia")
    r = con.execute("select id from tag where slug = ?", (s,)).fetchone()
    if r:
        return r["id"]
    marca = _da_marca(s)
    if marca:
        nome, cat, origem = marca[0], marca[1], "marca"
    else:
        nome, cat = nome_ou_slug.strip(), ""
    return con.execute("insert into tag (slug, nome, categoria, origem) values (?, ?, ?, ?)",
                       (s, nome, cat, origem)).lastrowid


def _marcar(con: sqlite3.Connection, midia_id: int, tag_id: int, por: str) -> None:
    con.execute("insert or ignore into midia_tag (midia_id, tag_id, por) values (?, ?, ?)", (midia_id, tag_id, por))


def aplicar_tags(ids: list[int], tags: list[str], remover: bool = False, db: str | None = None) -> int:
    """Põe (ou tira) as tags em todas as mídias de `ids`. Tag desconhecida vira tag livre."""
    ids = [int(i) for i in ids]
    if not ids or not tags:
        return 0
    con = abrir(db)
    n = 0
    for t in tags:
        if remover:
            r = con.execute("select id from tag where slug = ?", (slug(t),)).fetchone()
            if r:
                n += con.executemany("delete from midia_tag where midia_id = ? and tag_id = ?",
                                     [(i, r["id"]) for i in ids]).rowcount
        else:
            tid = _garantir_tag(con, t)
            for i in ids:
                _marcar(con, i, tid, "humano")
                n += 1
    con.commit()
    con.close()
    return n


def revisar(ids: list[int], revisado: bool = True, db: str | None = None) -> int:
    """Marca como revisado (libera para roteiros e b-roll) ou devolve para a fila."""
    con = abrir(db)
    agora = time.time() if revisado else None
    n = con.executemany("update midia set status_import = ?, revisado_em = ? where id = ?",
                        [("revisado" if revisado else "indexado", agora, int(i)) for i in ids]).rowcount
    con.commit()
    con.close()
    return n


def tags(db: str | None = None) -> dict:
    """As categorias da marca (com todas as tags do tags.toml, usadas ou não) e as tags de
    pasta e livres que existem no catálogo, cada uma com a contagem de mídias."""
    con = abrir(db)
    conta = {r["slug"]: (r["n"], r["origem"], r["nome"], r["categoria"]) for r in con.execute(
        "select t.slug, t.nome, t.categoria, t.origem, count(mt.midia_id) n from tag t "
        "left join midia_tag mt on mt.tag_id = t.id group by t.id")}
    con.close()
    cats = []
    for cs, c in categorias_marca().items():
        ts = [{"slug": s, "nome": nome, "n": conta.get(s, (0,))[0], "origem": "marca"}
              for s, nome in (c.get("tags") or {}).items()]
        cats.append({"slug": cs, "nome": c.get("nome") or cs, "multipla": bool(c.get("multipla", True)), "tags": ts})
    marca = {t["slug"] for c in cats for t in c["tags"]}
    soltas = [{"slug": s, "nome": v[2], "n": v[0], "origem": v[1]} for s, v in conta.items() if s not in marca]
    soltas.sort(key=lambda t: (-t["n"], t["nome"].lower()))
    return {"categorias": cats,
            "pasta": [t for t in soltas if t["origem"] == "pasta"],
            "livres": [t for t in soltas if t["origem"] != "pasta"]}


# ------------------------------------------------------------------ consulta
def _filtros(f: dict) -> tuple[str, list]:
    onde, args = ["m.ausente = 0"], []
    if f.get("importadas"):
        fontes_imp = [x["id"] for x in config.carregar().get("fontes") or [] if _e_importada(x)]
        onde.append(f"m.fonte_id in ({','.join('?' * len(fontes_imp)) or 'null'})")
        args += fontes_imp
    if f.get("fonte"):
        onde.append("m.fonte_id = ?")
        args.append(f["fonte"])
    if f.get("tipo") in ("video", "imagem"):
        onde.append("m.tipo = ?")
        args.append(f["tipo"])
    if f.get("importacao"):
        onde.append("m.importacao_id = ?")
        args.append(int(f["importacao"]))
    if f.get("subpasta"):
        onde.append("(m.subpasta = ? or m.subpasta like ?)")
        args += [f["subpasta"], f["subpasta"] + os.sep + "%"]
    passo = f.get("passo") or ""
    if passo == "fila":
        onde.append(f"m.status_import in ({','.join('?' * len(FILA))})")
        args += list(FILA)
    elif passo == "revisado":
        onde.append("m.status_import = 'revisado'")
    elif passo == "com_tags":
        onde.append("exists (select 1 from midia_tag mt join tag t on t.id = mt.tag_id "
                    "where mt.midia_id = m.id and t.origem != 'pasta')")
    elif passo == "sem_tags":
        onde.append("not exists (select 1 from midia_tag mt join tag t on t.id = mt.tag_id "
                    "where mt.midia_id = m.id and t.origem != 'pasta')")
    for t in f.get("tags") or []:
        onde.append("exists (select 1 from midia_tag mt join tag t on t.id = mt.tag_id "
                    "where mt.midia_id = m.id and t.slug = ?)")
        args.append(slug(t))
    busca = (f.get("busca") or "").strip().lower()
    for termo in busca.split():
        onde.append("(lower(m.caminho) like ? or lower(m.item) like ? or exists (select 1 from midia_tag mt "
                    "join tag t on t.id = mt.tag_id where mt.midia_id = m.id and (t.slug like ? or lower(t.nome) like ?)))")
        args += [f"%{termo}%"] * 4
    return " and ".join(onde), args


def listar(filtros: dict | None = None, limite: int = 120, offset: int = 0, db: str | None = None) -> dict:
    """Uma página de mídias com as tags de cada uma, e o total que passa no filtro."""
    f = filtros or {}
    if not os.path.exists(db or caminho_db()):
        return {"itens": [], "total": 0}
    con = abrir(db)
    onde, args = _filtros(f)
    total = con.execute(f"select count(*) from midia m where {onde}", args).fetchone()[0]
    ordem = "m.data desc, m.caminho" if f.get("ordem") != "antigos" else "m.data, m.caminho"
    linhas = con.execute(f"select m.* from midia m where {onde} order by {ordem} limit ? offset ?",
                         args + [int(limite), int(offset)]).fetchall()
    ids = [r["id"] for r in linhas]
    tags_de: dict[int, list] = {}
    if ids:
        for r in con.execute(f"select mt.midia_id, mt.por, t.slug, t.nome, t.origem, t.categoria from midia_tag mt "
                             f"join tag t on t.id = mt.tag_id where mt.midia_id in ({','.join('?' * len(ids))})", ids):
            tags_de.setdefault(r["midia_id"], []).append(
                {"slug": r["slug"], "nome": r["nome"], "origem": r["origem"], "categoria": r["categoria"], "por": r["por"]})
    con.close()
    itens = [{"id": r["id"], "fonte": r["fonte_id"], "caminho": r["caminho"], "nome": os.path.basename(r["caminho"]),
              "item": r["item"], "subpasta": r["subpasta"], "tipo": r["tipo"], "status": r["status_import"],
              "pilar": r["pilar"], "data": r["data"], "mb": round((r["tamanho"] or 0) / 1e6, 1),
              "tags": tags_de.get(r["id"], [])} for r in linhas]
    return {"itens": itens, "total": total}


def ids(filtros: dict | None = None, db: str | None = None) -> list[int]:
    """Todos os ids que passam no filtro ("selecionar tudo")."""
    if not os.path.exists(db or caminho_db()):
        return []
    con = abrir(db)
    onde, args = _filtros(filtros or {})
    out = [r[0] for r in con.execute(f"select m.id from midia m where {onde}", args)]
    con.close()
    return out


def resumo(filtros: dict | None = None, db: str | None = None) -> dict:
    """Contagens para os filtros: por passo, por tipo, por importação, por subpasta e por fonte.
    Cada grupo conta com os outros filtros aplicados, menos o próprio."""
    f = dict(filtros or {})
    if not os.path.exists(db or caminho_db()):
        return {"passos": {}, "tipos": {}, "importacoes": [], "subpastas": [], "fontes": {}}
    con = abrir(db)

    def contar(sem: str, sql: str, por_linha):
        g = {k: v for k, v in f.items() if k != sem}
        onde, args = _filtros(g)
        return [por_linha(r) for r in con.execute(sql.format(onde=onde), args)]

    passos = {}
    for p in ("", "fila", "com_tags", "sem_tags", "revisado"):
        onde, args = _filtros({**f, "passo": p})
        passos[p or "todos"] = con.execute(f"select count(*) from midia m where {onde}", args).fetchone()[0]
    tipos = dict(contar("tipo", "select m.tipo, count(*) from midia m where {onde} group by m.tipo",
                        lambda r: (r[0], r[1])))
    fontes_n = dict(contar("fonte", "select m.fonte_id, count(*) from midia m where {onde} group by m.fonte_id",
                           lambda r: (r[0], r[1])))
    imps = contar("importacao", "select i.id, i.fonte_id, i.data, count(m.id) from midia m join importacao i "
                                "on i.id = m.importacao_id where {onde} group by i.id order by i.data desc",
                  lambda r: {"id": r[0], "fonte": r[1], "data": r[2], "n": r[3],
                             "quando": time.strftime("%d/%m %H:%M", time.localtime(r[2] or 0))})
    subs: dict[str, int] = {}
    for sub, n in contar("subpasta", "select m.subpasta, count(*) from midia m where {onde} and m.subpasta != '' "
                                     "group by m.subpasta", lambda r: (r[0], r[1])):
        topo = sub.split(os.sep)[0]
        subs[topo] = subs.get(topo, 0) + n
    con.close()
    return {"passos": passos, "tipos": tipos, "fontes": fontes_n, "importacoes": imps,
            "subpastas": [{"nome": k, "n": v} for k, v in sorted(subs.items(), key=lambda x: -x[1])]}


def main() -> None:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd")
    s_ = sub.add_parser("sincronizar")
    s_.add_argument("--fonte")
    sub.add_parser("resumo")
    a = ap.parse_args()
    if a.cmd == "sincronizar":
        for fid, r in sincronizar(a.fonte).items():
            print(f"{fid}: {r}")
    else:
        print(json.dumps(resumo(), ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
