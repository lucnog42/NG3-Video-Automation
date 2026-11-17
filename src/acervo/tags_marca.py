#!/usr/bin/env python3
"""Gerenciador de tags da marca (fica dentro da aba Importar).

As categorias e as tags da marca moram em `config/marcas/<marca>/tags.toml`; as marcações, no
`midia.sqlite`. Aqui se cria, renomeia, move, funde e remove categorias e tags, com as
contagens de entradas (mídias + cortes) de cada uma.

  * renomear muda só o nome visível: o `slug` fica, para não quebrar as marcações;
  * remover uma tag pede destino: apagar as marcações ou fundir numa outra tag;
  * criar em lote: uma linha por tag, `Categoria: Nome`, `slug = Nome` ou só o nome;
  * toda gravação do tags.toml é atômica e guarda a versão anterior em
    `dados/marcas/<marca>/historico/tags/`. A seção `[ia.*]` (frases da IA de imagem) é mantida.

    python3 src/acervo/tags_marca.py
"""
from __future__ import annotations

import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))))  # src/

import json
import os
import re
import shutil
import time

try:
    import tomllib
except ImportError:                          # pragma: no cover (python < 3.11)
    import tomli as tomllib

from acervo import biblioteca  # noqa: E402
from nucleo import caminhos, config  # noqa: E402

CABECALHO = """# Tags da marca: vocabulário controlado para o import, os cortes e a busca.
# Editado pelo gerenciador de tags do painel (aba Importar); a versão anterior fica em
# dados/marcas/<marca>/historico/tags/. Cada categoria: nome, se aceita várias tags por mídia,
# e a lista (slug = "Nome visível").
"""


def arquivo() -> str:
    return caminhos.raiz("config", "marcas", caminhos.marca(), "tags.toml")


def ler(arq: str | None = None) -> dict:
    alvo = arq or arquivo()
    if not os.path.exists(alvo):
        return {"categorias": {}}
    with open(alvo, "rb") as fh:
        d = tomllib.load(fh)
    d.setdefault("categorias", {})
    return d


# ------------------------------------------------------------------ TOML
def _str(s: str) -> str:
    return json.dumps(str(s), ensure_ascii=False)


def _chave(k: str) -> str:
    return k if re.fullmatch(r"[A-Za-z0-9_-]+", k) else _str(k)


def _valor(v) -> str:
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, (int, float)):
        return repr(v)
    if isinstance(v, dict):
        return "{ " + ", ".join(f"{_chave(k)} = {_valor(x)}" for k, x in v.items()) + " }" if v else "{}"
    if isinstance(v, list):
        return "[" + ", ".join(_valor(x) for x in v) + "]"
    return _str(v)


def para_toml(d: dict) -> str:
    """Serializa o tags.toml (categorias e seções extras, como [ia.*]) num TOML legível."""
    linhas = [CABECALHO]
    for cs, c in (d.get("categorias") or {}).items():
        linhas.append(f"[categorias.{_chave(cs)}]")
        linhas.append(f"nome = {_str(c.get('nome') or cs)}")
        linhas.append(f"multipla = {_valor(bool(c.get('multipla', True)))}")
        for k, v in c.items():
            if k not in ("nome", "multipla", "tags"):
                linhas.append(f"{_chave(k)} = {_valor(v)}")
        linhas.append(f"tags = {_valor(dict(c.get('tags') or {}))}")
        linhas.append("")
    for secao, conteudo in d.items():
        if secao == "categorias" or not isinstance(conteudo, dict):
            continue
        for sub, vals in conteudo.items():
            if isinstance(vals, dict):
                linhas.append(f"[{_chave(secao)}.{_chave(sub)}]")
                linhas += [f"{_chave(k)} = {_valor(v)}" for k, v in vals.items()]
                linhas.append("")
    return "\n".join(linhas).rstrip() + "\n"


def gravar(d: dict, arq: str | None = None) -> None:
    alvo = arq or arquivo()
    if os.path.exists(alvo):
        hist = os.path.join(os.path.dirname(alvo), "historico") if arq else caminhos.dados("historico", "tags")
        os.makedirs(hist, exist_ok=True)
        shutil.copy2(alvo, os.path.join(hist, f"tags-{time.strftime('%Y%m%d-%H%M%S')}-{time.time_ns() % 10**6}.toml"))
    texto = para_toml(d)
    tomllib.loads(texto)                     # nunca grava um TOML que não lê de volta
    with open(alvo + ".tmp", "w", encoding="utf-8") as fh:
        fh.write(texto)
    os.replace(alvo + ".tmp", alvo)
    try:
        config.carregar(recarregar=True)
    except Exception:                         # pragma: no cover (marca de teste sem pasta)
        pass


# ------------------------------------------------------------------ painel
def _contagens(con) -> dict[str, dict]:
    """{slug: {id, nome, categoria, origem, midias, cortes}} de todas as tags do catálogo."""
    tem_corte = con.execute("select 1 from sqlite_master where name = 'corte_tag'").fetchone()
    cortes = "(select count(*) from corte_tag ct where ct.tag_id = t.id)" if tem_corte else "0"
    return {r["slug"]: dict(r) for r in con.execute(
        f"select t.id, t.slug, t.nome, t.categoria, t.origem, "
        f"(select count(*) from midia_tag mt where mt.tag_id = t.id) midias, {cortes} cortes from tag t")}


def painel(db: str | None = None, arq: str | None = None) -> dict:
    """As categorias (da marca e as que vieram do Lightroom) com as tags e as contagens, e as
    tags sem categoria. `n` = entradas: mídias + cortes."""
    d = ler(arq)
    con = biblioteca.abrir(db)
    conta = _contagens(con)
    entradas = {}
    for r in con.execute("select t.categoria, count(distinct mt.midia_id) n from midia_tag mt "
                         "join tag t on t.id = mt.tag_id where t.categoria != '' group by t.categoria"):
        entradas[r["categoria"]] = r["n"]
    if con.execute("select 1 from sqlite_master where name = 'corte_tag'").fetchone():
        for r in con.execute("select t.categoria, count(distinct ct.corte_id) n from corte_tag ct "
                             "join tag t on t.id = ct.tag_id where t.categoria != '' group by t.categoria"):
            entradas[r["categoria"]] = entradas.get(r["categoria"], 0) + r["n"]
    con.close()

    def tag(s: str, nome: str, origem: str) -> dict:
        c = conta.get(s) or {}
        return {"slug": s, "nome": c.get("nome") or nome, "origem": c.get("origem") or origem,
                "midias": c.get("midias", 0), "cortes": c.get("cortes", 0),
                "n": c.get("midias", 0) + c.get("cortes", 0)}
    cats, vistas = [], set()
    for cs, c in d["categorias"].items():
        ts = [tag(s, nome, "marca") for s, nome in (c.get("tags") or {}).items()]
        vistas |= {t["slug"] for t in ts}
        cats.append({"slug": cs, "nome": c.get("nome") or cs, "multipla": bool(c.get("multipla", True)),
                     "da_marca": True, "tags": ts, "n": entradas.get(cs, 0)})
    por_cat = {c["slug"]: c for c in cats}
    sem = []
    for s, c in conta.items():
        if s in vistas:
            continue
        t = tag(s, c["nome"], c["origem"])
        if c["categoria"]:
            if c["categoria"] not in por_cat:
                por_cat[c["categoria"]] = {"slug": c["categoria"], "nome": c["categoria"].replace("-", " ").capitalize(),
                                           "multipla": True, "da_marca": False, "tags": [],
                                           "n": entradas.get(c["categoria"], 0)}
                cats.append(por_cat[c["categoria"]])
            por_cat[c["categoria"]]["tags"].append(t)
        else:
            sem.append(t)
    for c in cats:
        c["tags"].sort(key=lambda t: (-t["n"], t["nome"].lower()))
    sem.sort(key=lambda t: (-t["n"], t["nome"].lower()))
    return {"categorias": cats, "sem_categoria": sem}


# ------------------------------------------------------------------ ações
def _cat_de(d: dict, s: str) -> str | None:
    for cs, c in d["categorias"].items():
        if s in (c.get("tags") or {}):
            return cs
    return None


def criar_categoria(nome: str, multipla: bool = True, arq: str | None = None) -> str:
    cs = biblioteca.slug(nome)
    if not cs:
        raise ValueError("nome de categoria vazio")
    d = ler(arq)
    if cs in d["categorias"]:
        raise ValueError(f"a categoria {nome} já existe")
    d["categorias"][cs] = {"nome": nome.strip(), "multipla": bool(multipla), "tags": {}}
    gravar(d, arq)
    return cs


def renomear_categoria(cs: str, nome: str, arq: str | None = None) -> None:
    d = ler(arq)
    if cs not in d["categorias"]:
        raise ValueError(f"categoria desconhecida: {cs}")
    if not nome.strip():
        raise ValueError("nome vazio")
    d["categorias"][cs]["nome"] = nome.strip()
    gravar(d, arq)


def remover_categoria(cs: str, db: str | None = None, arq: str | None = None) -> int:
    """Tira a categoria da marca. As tags dela ficam, sem categoria (as marcações continuam)."""
    d = ler(arq)
    tags = dict((d["categorias"].pop(cs, None) or {}).get("tags") or {})
    gravar(d, arq)
    con = biblioteca.abrir(db)
    n = con.execute("update tag set categoria = '' where categoria = ?", (cs,)).rowcount
    con.executemany("update tag set origem = 'livre' where slug = ? and origem = 'marca'", [(s,) for s in tags])
    con.commit()
    con.close()
    return n


def criar_tags(texto: str, categoria: str = "", db: str | None = None, arq: str | None = None) -> list[dict]:
    """Cria em lote. Cada linha: `Categoria: Nome`, `slug = Nome` ou só o nome. A categoria da
    linha vale mais que a do parâmetro; categoria que não existe é criada."""
    d = ler(arq)
    criadas = []
    for linha in texto.splitlines():
        linha = linha.strip().strip(",")
        if not linha or linha.startswith("#"):
            continue
        cat = categoria
        if ":" in linha:
            cat, linha = (x.strip() for x in linha.split(":", 1))
        if "=" in linha:
            s, nome = (x.strip().strip('"') for x in linha.split("=", 1))
            s = biblioteca.slug(s)
        else:
            nome, s = linha, biblioteca.slug(linha)
        if not s or not nome:
            continue
        cs = biblioteca.slug(cat) if cat else ""
        if cs:
            if cs not in d["categorias"]:
                d["categorias"][cs] = {"nome": cat.strip(), "multipla": True, "tags": {}}
            antiga = _cat_de(d, s)
            if antiga and antiga != cs:
                d["categorias"][antiga]["tags"].pop(s, None)
            d["categorias"][cs].setdefault("tags", {})[s] = nome
        criadas.append({"slug": s, "nome": nome, "categoria": cs})
    gravar(d, arq)
    con = biblioteca.abrir(db)
    for t in criadas:
        r = con.execute("select id from tag where slug = ?", (t["slug"],)).fetchone()
        if r:
            con.execute("update tag set nome = ?, categoria = ?, origem = case when ? != '' then 'marca' else origem end "
                        "where id = ?", (t["nome"], t["categoria"], t["categoria"], r["id"]))
        else:
            con.execute("insert into tag (slug, nome, categoria, origem) values (?, ?, ?, ?)",
                        (t["slug"], t["nome"], t["categoria"], "marca" if t["categoria"] else "livre"))
    con.commit()
    con.close()
    return criadas


def renomear_tag(s: str, nome: str, db: str | None = None, arq: str | None = None) -> None:
    """Muda só o nome visível; o slug fica."""
    if not nome.strip():
        raise ValueError("nome vazio")
    d = ler(arq)
    cs = _cat_de(d, s)
    if cs:
        d["categorias"][cs]["tags"][s] = nome.strip()
        gravar(d, arq)
    con = biblioteca.abrir(db)
    con.execute("update tag set nome = ? where slug = ?", (nome.strip(), s))
    con.commit()
    con.close()


def mover_tag(s: str, categoria: str, db: str | None = None, arq: str | None = None) -> None:
    """Põe a tag noutra categoria da marca (ou sem categoria, com `categoria` vazio)."""
    d = ler(arq)
    antiga = _cat_de(d, s)
    con = biblioteca.abrir(db)
    r = con.execute("select nome from tag where slug = ?", (s,)).fetchone()
    nome = (d["categorias"][antiga]["tags"][s] if antiga else None) or (r["nome"] if r else s)
    if antiga:
        d["categorias"][antiga]["tags"].pop(s, None)
    if categoria:
        if categoria not in d["categorias"]:
            raise ValueError(f"categoria desconhecida: {categoria}")
        d["categorias"][categoria].setdefault("tags", {})[s] = nome
    gravar(d, arq)
    if r:
        con.execute("update tag set categoria = ? where slug = ?", (categoria, s))
    else:
        con.execute("insert into tag (slug, nome, categoria, origem) values (?, ?, ?, 'marca')", (s, nome, categoria))
    con.commit()
    con.close()


def fundir(s: str, em: str, db: str | None = None, arq: str | None = None) -> int:
    """Passa todas as marcações (mídias e cortes) de `s` para `em` e remove `s`."""
    if s == em:
        raise ValueError("escolha outra tag para fundir")
    con = biblioteca.abrir(db)
    a = con.execute("select id from tag where slug = ?", (s,)).fetchone()
    b = con.execute("select id from tag where slug = ?", (em,)).fetchone()
    if not b:
        con.close()
        if biblioteca._da_marca(em) is None:
            raise ValueError(f"tag desconhecida: {em}")
        con = biblioteca.abrir(db)
        b = {"id": biblioteca._garantir_tag(con, em)}
        a = con.execute("select id from tag where slug = ?", (s,)).fetchone()
    n = 0
    if a:
        n = con.execute("insert or ignore into midia_tag (midia_id, tag_id, por) "
                        "select midia_id, ?, por from midia_tag where tag_id = ?", (b["id"], a["id"])).rowcount
        if con.execute("select 1 from sqlite_master where name = 'corte_tag'").fetchone():
            n += con.execute("insert or ignore into corte_tag (corte_id, tag_id, por) "
                             "select corte_id, ?, por from corte_tag where tag_id = ?", (b["id"], a["id"])).rowcount
    con.commit()
    con.close()
    remover_tag(s, db=db, arq=arq)
    return n


def remover_tag(s: str, destino: str = "apagar", em: str = "", db: str | None = None, arq: str | None = None) -> int:
    """Remove a tag. `destino`: `apagar` (as marcações somem junto) ou `fundir` (vão para `em`)."""
    if destino == "fundir":
        return fundir(s, em, db, arq)
    if destino != "apagar":
        raise ValueError("destino é apagar ou fundir")
    d = ler(arq)
    cs = _cat_de(d, s)
    if cs:
        d["categorias"][cs]["tags"].pop(s, None)
        gravar(d, arq)
    con = biblioteca.abrir(db)
    r = con.execute("select id from tag where slug = ?", (s,)).fetchone()
    n = 0
    if r:
        n = con.execute("delete from midia_tag where tag_id = ?", (r["id"],)).rowcount
        for tabela in ("corte_tag", "midia_sugestao", "corte_sugestao"):
            if con.execute("select 1 from sqlite_master where name = ?", (tabela,)).fetchone():
                apagadas = con.execute(f"delete from {tabela} where tag_id = ?", (r["id"],)).rowcount
                if tabela == "corte_tag":
                    n += apagadas
        con.execute("delete from tag where id = ?", (r["id"],))
    con.commit()
    con.close()
    return n


def main() -> None:
    print(json.dumps(painel(), ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
