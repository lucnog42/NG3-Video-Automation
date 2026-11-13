#!/usr/bin/env python3
"""Cortes como anotação: entrada/saída num vídeo do catálogo, com tags e nota próprias.

Nenhum arquivo é criado nem alterado: um corte é uma linha em `midia.sqlite` (tabelas `corte`
e `corte_tag`, ao lado das da biblioteca). Três origens:

  * `fala`   ideias da fala: as frases da transcrição (`plano.falas.frases_de`) agrupadas
             enquanto a pausa entre elas for menor que `cortes.fala.pausa_ideia`;
  * `cena`   mudanças de cena do PySceneDetect (`ContentDetector`), no ambiente `sep`, que já
             tem o scenedetect e o OpenCV;
  * `manual` entrada e saída marcadas na tela (I / O).

Os automáticos nascem **propostos**. Aceitar, ajustar a borda, dividir ou juntar tira o
"proposto" e protege o corte: reprocessar só troca os propostos que ninguém tocou, e não
recria um proposto que a pessoa descartou. Um corte manual nunca é sobrescrito.

Tag do corte e tag do vídeo são níveis separados (dá para copiar as do vídeo para o corte).

    python3 src/acervo/cortes.py propor --origem fala --id 12 --id 13
    python3 src/acervo/cortes.py propor --origem cena --lista ids.txt     # tarefa do painel
"""
from __future__ import annotations

import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))))  # src/

import argparse
import json
import os
import sqlite3
import subprocess
import sys
import time

from acervo import biblioteca  # noqa: E402
from nucleo import caminhos, config  # noqa: E402

ORIGENS = ("fala", "cena", "manual")

ESQUEMA = """
create table if not exists corte (
    id integer primary key, midia_id integer not null, ini real not null, fim real not null,
    origem text not null, proposto integer not null default 1, protegido integer not null default 0,
    descartado integer not null default 0, titulo text default '', nota text default '', texto text default '',
    criado_em real, editado_em real);
create index if not exists corte_midia on corte (midia_id);
create table if not exists corte_tag (
    corte_id integer not null, tag_id integer not null, por text not null,
    primary key (corte_id, tag_id));
"""


def abrir(db: str | None = None) -> sqlite3.Connection:
    con = biblioteca.abrir(db)
    con.executescript(ESQUEMA)
    return con


def cfg() -> dict:
    try:
        return config.carregar().get("cortes") or {}
    except Exception:                       # pragma: no cover
        return {}


def _abs(caminho: str) -> str:
    return caminho if os.path.isabs(caminho) else os.path.join(config.carregar()["acervo"]["raiz"], caminho)


# ------------------------------------------------------------ geradores
def ideias_da_fala(palavras: list[tuple[float, float, str]], c: dict | None = None) -> list[tuple[float, float, str]]:
    """Frases agrupadas em ideias pela pausa; ideia curta cola na vizinha, longa é quebrada."""
    from plano import falas
    c = {**{"pausa_ideia": 1.2, "minimo": 2.0, "maximo": 30.0, "folga": 0.15}, **(c or {})}
    frases = falas.frases_de(palavras)
    grupos: list[list] = []
    for f in frases:
        if grupos and f[0] - grupos[-1][-1][1] < c["pausa_ideia"] and f[1] - grupos[-1][0][0] <= c["maximo"]:
            grupos[-1].append(f)
        else:
            grupos.append([f])
    juntos: list[list] = []
    for g in grupos:
        if juntos and (g[-1][1] - g[0][0] < c["minimo"] or juntos[-1][-1][1] - juntos[-1][0][0] < c["minimo"]) \
                and g[-1][1] - juntos[-1][0][0] <= c["maximo"]:
            juntos[-1].extend(g)
        else:
            juntos.append(g)
    fim_video = palavras[-1][1] if palavras else 0
    return [(round(max(0.0, g[0][0] - c["folga"]), 2), round(min(fim_video + c["folga"], g[-1][1] + c["folga"]), 2),
             " ".join(f[2] for f in g)) for g in juntos]


def cenas_do_video(video: str, c: dict | None = None) -> list[tuple[float, float]]:
    """Mudanças de cena (PySceneDetect no ambiente `sep`). Cena curta junta com a anterior."""
    c = {**{"limiar": 27.0, "minimo": 1.5}, **(c or {})}
    py = caminhos.python_de("sep")
    if not os.path.exists(py):
        raise RuntimeError(f"sem o ambiente com o PySceneDetect em {caminhos.venv('sep')}")
    r = subprocess.run([py, os.path.abspath(__file__), "_cenas", video, str(c["limiar"])],
                       capture_output=True, text=True, timeout=3600)
    if r.returncode != 0:
        raise RuntimeError(f"PySceneDetect falhou: {(r.stderr or r.stdout)[-200:].strip()}")
    cenas = [tuple(x) for x in json.loads(r.stdout.strip().splitlines()[-1])]
    out: list[list[float]] = []
    for a, b in cenas:
        if out and b - a < c["minimo"]:
            out[-1][1] = b
        else:
            out.append([a, b])
    return [(round(a, 2), round(b, 2)) for a, b in out]


def _cenas_aqui(video: str, limiar: float) -> None:
    """Roda dentro do ambiente `sep`: imprime [[ini, fim], ...] em segundos."""
    from scenedetect import ContentDetector, detect
    cenas = detect(video, ContentDetector(threshold=limiar), start_in_scene=True)
    print(json.dumps([[a.get_seconds(), b.get_seconds()] for a, b in cenas]))


# ------------------------------------------------------------ propor
def _sobrepoe(a: tuple[float, float], b: tuple[float, float]) -> float:
    """Fração do menor intervalo coberta pela interseção."""
    inter = max(0.0, min(a[1], b[1]) - max(a[0], b[0]))
    menor = max(min(a[1] - a[0], b[1] - b[0]), 1e-6)
    return inter / menor


def gravar_propostas(midia_id: int, origem: str, novos: list[tuple], db: str | None = None) -> dict:
    """Troca os propostos intocados desta origem pelos `novos` [(ini, fim[, texto])]. Protegidos,
    aceitos e manuais ficam; proposto igual a um descartado (ou a um protegido) não volta."""
    con = abrir(db)
    agora = time.time()
    con.execute("delete from corte where midia_id = ? and origem = ? and proposto = 1 and protegido = 0 "
                "and descartado = 0", (midia_id, origem))
    ficam = [(r[0], r[1]) for r in con.execute(
        "select ini, fim from corte where midia_id = ? and (origem = ? or protegido = 1 or descartado = 1)",
        (midia_id, origem))]
    n = 0
    for x in novos:
        ini, fim = float(x[0]), float(x[1])
        if fim - ini < 0.3 or any(_sobrepoe((ini, fim), f) > 0.8 for f in ficam):
            continue
        con.execute("insert into corte (midia_id, ini, fim, origem, proposto, texto, criado_em) values (?,?,?,?,1,?,?)",
                    (midia_id, ini, fim, origem, x[2] if len(x) > 2 else "", agora))
        n += 1
    con.commit()
    con.close()
    return {"midia": midia_id, "origem": origem, "propostos": n}


def propor(ids: list[int], origem: str, db: str | None = None) -> list[dict]:
    """Propõe cortes de `origem` (fala ou cena) para os vídeos `ids`."""
    if origem not in ("fala", "cena"):
        raise ValueError("origem é fala ou cena")
    con = abrir(db)
    linhas = con.execute(f"select id, caminho from midia where tipo = 'video' and id in ({','.join('?' * len(ids))})",
                         [int(i) for i in ids]).fetchall() if ids else []
    con.close()
    c = cfg().get(origem) or {}
    out = []
    for mid, caminho in linhas:
        try:
            if origem == "fala":
                from transcricao import legendas, legendar
                w = legendar.caminho_saida(_abs(caminho), ".words.json")
                if not os.path.exists(w):
                    out.append({"midia": mid, "origem": origem, "erro": "sem transcrição"})
                    continue
                novos = ideias_da_fala(legendas.ler_words_json(w), c)
            else:
                novos = cenas_do_video(_abs(caminho), c)
            out.append(gravar_propostas(mid, origem, novos, db))
        except Exception as e:
            out.append({"midia": mid, "origem": origem, "erro": str(e)[:200]})
        print(json.dumps(out[-1], ensure_ascii=False), flush=True)
    return out


# ------------------------------------------------------------ editar
def criar(midia_id: int, ini: float, fim: float, nota: str = "", db: str | None = None) -> int:
    ini, fim = sorted((float(ini), float(fim)))
    if fim - ini < 0.2:
        raise ValueError("o corte precisa de pelo menos 0,2 s")
    con = abrir(db)
    cid = con.execute("insert into corte (midia_id, ini, fim, origem, proposto, protegido, nota, criado_em, editado_em) "
                      "values (?, ?, ?, 'manual', 0, 1, ?, ?, ?)",
                      (int(midia_id), ini, fim, nota, time.time(), time.time())).lastrowid
    con.commit()
    con.close()
    return cid


def editar(corte_id: int, db: str | None = None, **campos) -> None:
    """ini, fim, titulo, nota. Mexer na borda tira o "proposto" e protege o corte."""
    con = abrir(db)
    r = con.execute("select ini, fim from corte where id = ?", (int(corte_id),)).fetchone()
    if r is None:
        con.close()
        raise ValueError("corte não existe")
    ini = float(campos.get("ini", r["ini"]))
    fim = float(campos.get("fim", r["fim"]))
    ini, fim = sorted((max(0.0, ini), max(0.0, fim)))
    if fim - ini < 0.2:
        con.close()
        raise ValueError("o corte precisa de pelo menos 0,2 s")
    sets = ["ini = ?", "fim = ?", "editado_em = ?"]
    args: list = [ini, fim, time.time()]
    if "ini" in campos or "fim" in campos:
        sets += ["proposto = 0", "protegido = 1"]
    for k in ("titulo", "nota"):
        if k in campos:
            sets.append(f"{k} = ?")
            args.append(str(campos[k]))
    con.execute(f"update corte set {', '.join(sets)} where id = ?", args + [int(corte_id)])
    con.commit()
    con.close()


def aceitar(ids: list[int], db: str | None = None) -> int:
    con = abrir(db)
    n = con.executemany("update corte set proposto = 0, protegido = 1, editado_em = ? where id = ?",
                        [(time.time(), int(i)) for i in ids]).rowcount
    con.commit()
    con.close()
    return n


def descartar(ids: list[int], db: str | None = None) -> int:
    """Manual some de vez; automático fica como descartado (para não voltar ao reprocessar)."""
    con = abrir(db)
    ids = [int(i) for i in ids]
    q = ",".join("?" * len(ids))
    con.execute(f"delete from corte_tag where corte_id in ({q}) and corte_id in "
                f"(select id from corte where origem = 'manual')", ids)
    n = con.execute(f"delete from corte where id in ({q}) and origem = 'manual'", ids).rowcount
    n += con.execute(f"update corte set descartado = 1, proposto = 0 where id in ({q}) and origem != 'manual'", ids).rowcount
    con.commit()
    con.close()
    return n


def dividir(corte_id: int, t: float, db: str | None = None) -> list[int]:
    """Dois cortes no lugar de um, partindo em `t`; as tags vão para os dois."""
    con = abrir(db)
    r = con.execute("select * from corte where id = ?", (int(corte_id),)).fetchone()
    if r is None or not (r["ini"] + 0.2 < t < r["fim"] - 0.2):
        con.close()
        raise ValueError("o ponto de divisão tem de estar dentro do corte")
    agora = time.time()
    con.execute("update corte set fim = ?, proposto = 0, protegido = 1, editado_em = ? where id = ?", (t, agora, r["id"]))
    novo = con.execute("insert into corte (midia_id, ini, fim, origem, proposto, protegido, titulo, nota, texto, "
                       "criado_em, editado_em) values (?, ?, ?, ?, 0, 1, ?, ?, ?, ?, ?)",
                       (r["midia_id"], t, r["fim"], r["origem"], r["titulo"], r["nota"], "", agora, agora)).lastrowid
    con.execute("insert into corte_tag (corte_id, tag_id, por) select ?, tag_id, por from corte_tag where corte_id = ?",
                (novo, r["id"]))
    con.commit()
    con.close()
    return [r["id"], novo]


def juntar(ids: list[int], db: str | None = None) -> int:
    """Um corte do começo do primeiro ao fim do último (mesmo vídeo); tags e notas somadas."""
    ids = [int(i) for i in ids]
    if len(ids) < 2:
        raise ValueError("escolha dois ou mais cortes")
    con = abrir(db)
    rs = con.execute(f"select * from corte where id in ({','.join('?' * len(ids))}) order by ini", ids).fetchall()
    if len({r["midia_id"] for r in rs}) != 1:
        con.close()
        raise ValueError("só dá para juntar cortes do mesmo vídeo")
    fica, resto = rs[0], rs[1:]
    nota = " ".join(x for x in (r["nota"] for r in rs) if x)
    texto = " ".join(x for x in (r["texto"] for r in rs) if x)
    con.execute("update corte set fim = ?, nota = ?, texto = ?, proposto = 0, protegido = 1, editado_em = ? where id = ?",
                (max(r["fim"] for r in rs), nota, texto, time.time(), fica["id"]))
    for r in resto:
        con.execute("insert or ignore into corte_tag (corte_id, tag_id, por) select ?, tag_id, por from corte_tag "
                    "where corte_id = ?", (fica["id"], r["id"]))
        con.execute("delete from corte_tag where corte_id = ?", (r["id"],))
        con.execute("delete from corte where id = ?", (r["id"],))
    con.commit()
    con.close()
    return fica["id"]


# ------------------------------------------------------------ tags
def aplicar_tags(ids: list[int], tags: list[str], remover: bool = False, db: str | None = None) -> int:
    ids = [int(i) for i in ids]
    con = abrir(db)
    n = 0
    for t in tags:
        if remover:
            r = con.execute("select id from tag where slug = ?", (biblioteca.slug(t),)).fetchone()
            if r:
                n += con.executemany("delete from corte_tag where corte_id = ? and tag_id = ?",
                                     [(i, r["id"]) for i in ids]).rowcount
        else:
            tid = biblioteca._garantir_tag(con, t)
            n += con.executemany("insert or ignore into corte_tag (corte_id, tag_id, por) values (?, ?, 'humano')",
                                 [(i, tid) for i in ids]).rowcount
    con.commit()
    con.close()
    return n


def copiar_tags_do_video(ids: list[int], db: str | None = None) -> int:
    """As tags do vídeo (menos as de pasta) passam a valer também para o corte."""
    con = abrir(db)
    n = 0
    for i in [int(x) for x in ids]:
        n += con.execute("insert or ignore into corte_tag (corte_id, tag_id, por) "
                         "select c.id, mt.tag_id, 'video' from corte c join midia_tag mt on mt.midia_id = c.midia_id "
                         "join tag t on t.id = mt.tag_id where c.id = ? and t.origem != 'pasta'", (i,)).rowcount
    con.commit()
    con.close()
    return n


# ------------------------------------------------------------ consulta
def listar(midia_ids: list[int], descartados: bool = False, db: str | None = None) -> list[dict]:
    ids = [int(i) for i in midia_ids]
    if not ids:
        return []
    con = abrir(db)
    q = ",".join("?" * len(ids))
    rs = con.execute(f"select * from corte where midia_id in ({q}) {'' if descartados else 'and descartado = 0'} "
                     f"order by midia_id, ini", ids).fetchall()
    cids = [r["id"] for r in rs]
    tags: dict[int, list] = {}
    if cids:
        for t in con.execute(f"select ct.corte_id, ct.por, t.slug, t.nome, t.origem from corte_tag ct join tag t "
                             f"on t.id = ct.tag_id where ct.corte_id in ({','.join('?' * len(cids))})", cids):
            tags.setdefault(t["corte_id"], []).append({"slug": t["slug"], "nome": t["nome"], "origem": t["origem"],
                                                      "por": t["por"]})
    con.close()
    return [{"id": r["id"], "midia": r["midia_id"], "ini": r["ini"], "fim": r["fim"], "origem": r["origem"],
             "proposto": bool(r["proposto"]), "protegido": bool(r["protegido"]), "descartado": bool(r["descartado"]),
             "titulo": r["titulo"], "nota": r["nota"], "texto": r["texto"], "tags": tags.get(r["id"], [])} for r in rs]


def contagem(midia_ids: list[int], db: str | None = None) -> dict[int, dict]:
    """Por vídeo: quantos cortes (sem os descartados) e quantos ainda propostos."""
    ids = [int(i) for i in midia_ids]
    if not ids:
        return {}
    con = abrir(db)
    out = {r[0]: {"cortes": r[1], "propostos": r[2]} for r in con.execute(
        f"select midia_id, count(*), sum(proposto) from corte where descartado = 0 and midia_id in "
        f"({','.join('?' * len(ids))}) group by midia_id", ids)}
    con.close()
    return out


def main() -> None:
    if len(sys.argv) >= 4 and sys.argv[1] == "_cenas":
        _cenas_aqui(sys.argv[2], float(sys.argv[3]))
        return
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd")
    p = sub.add_parser("propor")
    p.add_argument("--origem", required=True, choices=["fala", "cena"])
    p.add_argument("--id", type=int, action="append", default=[])
    p.add_argument("--lista", help="arquivo com um id de mídia por linha")
    a = ap.parse_args()
    if a.cmd == "propor":
        ids = list(a.id)
        if a.lista:
            with open(a.lista, encoding="utf-8") as fh:
                ids += [int(x) for x in fh.read().split() if x.strip()]
        r = propor(ids, a.origem)
        ok = sum(1 for x in r if "erro" not in x)
        print(f"{ok} de {len(r)} vídeo(s) · {sum(x.get('propostos', 0) for x in r)} corte(s) proposto(s)")
    else:
        ap.print_help()


if __name__ == "__main__":
    main()
