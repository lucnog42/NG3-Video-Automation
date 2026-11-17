#!/usr/bin/env python3
"""Tags do Lightroom e dos metadados das mídias (só leitura).

Lê, para as mídias escolhidas do catálogo:

  * o **sidecar `.xmp`** do Lightroom ao lado do arquivo (`foto.xmp` ou `foto.jpg.xmp`):
    keywords (`dc:subject`), keywords hierárquicas (`lr:hierarchicalSubject`), título,
    legenda, cidade, país, estrelas e rótulo de cor;
  * o **XMP embutido** no próprio arquivo (fotos e vídeos exportados) e o **IPTC** das fotos
    JPEG (keywords, título, legenda, cidade, país);
  * o **catálogo `.lrcat`** (SQLite), lido numa cópia temporária, nunca no original: keywords
    (com a árvore de pais) e coleções de cada foto.

Nada é escrito nos arquivos lidos. O resultado vira tag no catálogo (`midia.sqlite`):

  * hierarquia `Categoria|Tag`: o primeiro nível é a categoria da marca (pelo slug ou pelo
    nome), o último é a tag; categoria que a marca não tem aparece como "nova";
  * keyword solta vira tag sem categoria; cidade e país viram tag de Lugar;
  * título e legenda vão para `midia_texto` (entram na busca, não viram tag);
  * estrelas e cor só viram tag com a opção ligada;
  * origem `lightroom` (sidecar e catálogo) ou `metadados` (embutido no arquivo).

Antes de gravar, `previa()` mostra o mapeamento. Quando o mesmo slug já existe no catálogo com
outra origem (uma tag de pasta, por exemplo), a pessoa escolhe por tag: **agrupar** (uma tag
só, padrão) ou **manter separadas** (a importada vira `<slug>-<origem>`).

    python3 src/acervo/lightroom.py previa --ids 1,2,3 [--lrcat catalogo.lrcat]
"""
from __future__ import annotations

import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))))  # src/

import argparse
import json
import os
import shutil
import sqlite3
import struct
import tempfile
import xml.etree.ElementTree as ET

from acervo import biblioteca  # noqa: E402
from nucleo import config  # noqa: E402

ESQUEMA = """
create table if not exists midia_texto (midia_id integer not null, campo text not null, texto text,
    origem text not null, primary key (midia_id, campo, origem));
"""

NS = {
    "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
    "dc": "http://purl.org/dc/elements/1.1/",
    "lr": "http://ns.adobe.com/lightroom/1.0/",
    "xmp": "http://ns.adobe.com/xap/1.0/",
    "photoshop": "http://ns.adobe.com/photoshop/1.0/",
    "iptc": "http://iptc.org/std/Iptc4xmpCore/1.0/xmlns/",
}
SEP_HIER = "|"
LUGAR = "lugar"                 # categoria que recebe cidade e país
ORIGENS = ("lightroom", "metadados")
LEITURA_EMBUTIDO = 4 * 1024 * 1024   # bytes do começo (e do fim) onde o XMP embutido costuma estar


def abrir(db: str | None = None) -> sqlite3.Connection:
    con = biblioteca.abrir(db)
    con.executescript(ESQUEMA)
    return con


# ------------------------------------------------------------------ XMP
def _vazio() -> dict:
    return {"keywords": [], "hier": [], "titulo": "", "legenda": "", "cidade": "", "pais": "",
            "estrelas": None, "cor": "", "colecoes": []}


def _q(ns: str, nome: str) -> str:
    return "{" + NS[ns] + "}" + nome


def _itens(el) -> list[str]:
    """Os rdf:li de um Bag/Seq/Alt (ou o texto direto do elemento)."""
    if el is None:
        return []
    lis = [(li.text or "").strip() for li in el.iter(_q("rdf", "li"))]
    if lis:
        return [x for x in lis if x]
    t = (el.text or "").strip()
    return [t] if t else []


def ler_xmp(texto: str | bytes) -> dict:
    """Campos úteis de um pacote XMP (sidecar inteiro ou trecho embutido). Tolerante: XMP
    quebrado devolve o que der."""
    out = _vazio()
    if isinstance(texto, bytes):
        texto = texto.decode("utf-8", "replace")
    ini = texto.find("<x:xmpmeta")
    if ini < 0:
        ini = texto.find("<rdf:RDF")
    fim_tag = "</x:xmpmeta>" if texto.find("</x:xmpmeta>") >= 0 else "</rdf:RDF>"
    fim = texto.find(fim_tag, max(ini, 0))
    if ini < 0 or fim < 0:
        return out
    try:
        raiz = ET.fromstring(texto[ini:fim + len(fim_tag)])
    except ET.ParseError:
        return out
    for desc in raiz.iter(_q("rdf", "Description")):
        # campos simples podem vir como atributo da Description
        at = desc.attrib
        if at.get(_q("xmp", "Rating")) and out["estrelas"] is None:
            out["estrelas"] = _int(at[_q("xmp", "Rating")])
        out["cor"] = out["cor"] or at.get(_q("xmp", "Label"), "")
        out["cidade"] = out["cidade"] or at.get(_q("photoshop", "City"), "")
        out["pais"] = out["pais"] or at.get(_q("photoshop", "Country"), "")
        for el in desc:
            if el.tag == _q("dc", "subject"):
                out["keywords"] += _itens(el)
            elif el.tag == _q("lr", "hierarchicalSubject"):
                out["hier"] += [[p.strip() for p in h.split(SEP_HIER) if p.strip()] for h in _itens(el)]
            elif el.tag == _q("dc", "title") and not out["titulo"]:
                out["titulo"] = (_itens(el) or [""])[0]
            elif el.tag == _q("dc", "description") and not out["legenda"]:
                out["legenda"] = (_itens(el) or [""])[0]
            elif el.tag == _q("photoshop", "City") and not out["cidade"]:
                out["cidade"] = (el.text or "").strip()
            elif el.tag == _q("photoshop", "Country") and not out["pais"]:
                out["pais"] = (el.text or "").strip()
            elif el.tag == _q("xmp", "Rating") and out["estrelas"] is None:
                out["estrelas"] = _int(el.text)
            elif el.tag == _q("xmp", "Label") and not out["cor"]:
                out["cor"] = (el.text or "").strip()
    out["hier"] = [h for h in out["hier"] if h]
    return out


def _int(v) -> int | None:
    try:
        return int(float(str(v).strip()))
    except (TypeError, ValueError):
        return None


def sidecar(arq: str) -> str | None:
    """O .xmp do Lightroom ao lado do arquivo: `foto.xmp` (padrão) ou `foto.jpg.xmp`."""
    base = os.path.splitext(arq)[0]
    for c in (base + ".xmp", base + ".XMP", arq + ".xmp", arq + ".XMP"):
        if os.path.isfile(c):
            return c
    return None


def xmp_embutido(arq: str) -> dict:
    """O pacote XMP dentro do arquivo (JPEG, PNG, TIFF, MP4, MOV...), procurado no começo e no
    fim, onde os programas costumam gravá-lo."""
    try:
        tam = os.path.getsize(arq)
        with open(arq, "rb") as fh:
            blocos = [fh.read(LEITURA_EMBUTIDO)]
            if tam > LEITURA_EMBUTIDO:
                fh.seek(max(LEITURA_EMBUTIDO, tam - LEITURA_EMBUTIDO))
                blocos.append(fh.read())
    except OSError:
        return _vazio()
    for b in blocos:
        i = b.find(b"<x:xmpmeta")
        j = b.find(b"</x:xmpmeta>", i if i >= 0 else 0)
        if i >= 0 and j > i:
            return ler_xmp(b[i:j + len(b"</x:xmpmeta>")])
    return _vazio()


# ------------------------------------------------------------------ IPTC (JPEG)
def _iptc_texto(b: bytes) -> str:
    try:
        return b.decode("utf-8").strip()
    except UnicodeDecodeError:
        return b.decode("latin-1").strip()


def ler_iptc_registros(dados: bytes) -> dict:
    """Registros IPTC-IIM (2:xx) de um bloco: keywords (25), título (5), cidade (90),
    país (101), legenda (120)."""
    out = _vazio()
    i = 0
    while i + 5 <= len(dados):
        if dados[i] != 0x1C:
            i += 1
            continue
        rec, ds, n = dados[i + 1], dados[i + 2], struct.unpack(">H", dados[i + 3:i + 5])[0]
        if n & 0x8000:                  # tamanho estendido: raro em keywords; para aqui
            break
        v = _iptc_texto(dados[i + 5:i + 5 + n])
        i += 5 + n
        if rec != 2 or not v:
            continue
        if ds == 25:
            out["keywords"].append(v)
        elif ds == 5 and not out["titulo"]:
            out["titulo"] = v
        elif ds == 90 and not out["cidade"]:
            out["cidade"] = v
        elif ds == 101 and not out["pais"]:
            out["pais"] = v
        elif ds == 120 and not out["legenda"]:
            out["legenda"] = v
    return out


def iptc_jpeg(arq: str) -> dict:
    """IPTC do segmento APP13 (Photoshop 3.0, recurso 0x0404) de um JPEG."""
    if os.path.splitext(arq)[1].lower() not in (".jpg", ".jpeg"):
        return _vazio()
    try:
        with open(arq, "rb") as fh:
            if fh.read(2) != b"\xff\xd8":
                return _vazio()
            while True:
                m = fh.read(4)
                if len(m) < 4 or m[0] != 0xFF:
                    return _vazio()
                marcador, n = m[1], struct.unpack(">H", m[2:4])[0]
                if marcador in (0xDA, 0xD9):        # começo da imagem: acabaram os metadados
                    return _vazio()
                seg = fh.read(n - 2)
                if marcador == 0xED and seg.startswith(b"Photoshop 3.0\x00"):
                    return _iptc_de_photoshop(seg[14:])
    except OSError:
        return _vazio()


def _iptc_de_photoshop(b: bytes) -> dict:
    i = 0
    while i + 12 <= len(b) and b[i:i + 4] == b"8BIM":
        rid = struct.unpack(">H", b[i + 4:i + 6])[0]
        nl = b[i + 6]
        j = i + 7 + nl
        if (nl + 1) % 2:
            j += 1
        tam = struct.unpack(">I", b[j:j + 4])[0]
        dados = b[j + 4:j + 4 + tam]
        if rid == 0x0404:
            return ler_iptc_registros(dados)
        i = j + 4 + tam + (tam % 2)
    return _vazio()


# ------------------------------------------------------------------ catálogo .lrcat
def ler_lrcat(caminho: str) -> dict[str, dict]:
    """{caminho absoluto: {"hier": [[...]], "colecoes": [...]}} de um catálogo do Lightroom
    Classic. O original nunca é aberto: copia o .lrcat (e o -wal, se houver) para uma pasta
    temporária e lê a cópia."""
    if not os.path.isfile(caminho):
        raise ValueError(f"catálogo não encontrado: {caminho}")
    tmp = tempfile.mkdtemp(prefix="ng3-lrcat-")
    try:
        copia = os.path.join(tmp, "catalogo.lrcat")
        shutil.copy2(caminho, copia)
        for suf in ("-wal", "-shm"):
            if os.path.exists(caminho + suf):
                shutil.copy2(caminho + suf, copia + suf)
        con = sqlite3.connect(f"file:{copia}?mode=ro", uri=True)
        try:
            return _ler_lrcat(con)
        finally:
            con.close()
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def _ler_lrcat(con: sqlite3.Connection) -> dict[str, dict]:
    arquivos = {}
    for img, raiz, rel, base, ext in con.execute(
            "select i.id_local, r.absolutePath, f.pathFromRoot, a.baseName, a.extension from Adobe_images i "
            "join AgLibraryFile a on a.id_local = i.rootFile join AgLibraryFolder f on f.id_local = a.folder "
            "join AgLibraryRootFolder r on r.id_local = f.rootFolder"):
        nome = base + ("." + ext if ext else "")
        arquivos[img] = os.path.normpath(os.path.join(raiz or "", rel or "", nome))
    kws = {kid: (nome, pai) for kid, nome, pai in con.execute("select id_local, name, parent from AgLibraryKeyword")}

    def cadeia(kid) -> list[str]:
        out, vistos = [], set()
        while kid in kws and kid not in vistos:
            vistos.add(kid)
            nome, pai = kws[kid]
            if nome:                    # a raiz da árvore não tem nome
                out.append(nome)
            kid = pai
        return list(reversed(out))
    out: dict[str, dict] = {}
    for img, kid in con.execute("select image, tag from AgLibraryKeywordImage"):
        if img in arquivos:
            h = cadeia(kid)
            if h:
                out.setdefault(arquivos[img], {"hier": [], "colecoes": []})["hier"].append(h)
    try:
        cols = con.execute("select ci.image, c.name from AgLibraryCollectionImage ci "
                           "join AgLibraryCollection c on c.id_local = ci.collection").fetchall()
    except sqlite3.OperationalError:
        cols = []
    for img, nome in cols:
        if img in arquivos and nome:
            out.setdefault(arquivos[img], {"hier": [], "colecoes": []})["colecoes"].append(nome)
    return out


# ------------------------------------------------------------------ mapeamento
def _categoria(nome: str) -> tuple[str, str, bool]:
    """(slug, nome visível, é nova) da categoria de um primeiro nível de hierarquia: casa com a
    categoria da marca pelo slug ou pelo nome."""
    s = biblioteca.slug(nome)
    for cs, c in biblioteca.categorias_marca().items():
        if s in (cs, biblioteca.slug(c.get("nome") or "")):
            return cs, c.get("nome") or cs, False
    return s, nome.strip(), True


def tags_de(dados: dict, origem: str, estrelas: bool = False) -> list[dict]:
    """Tags propostas para uma mídia a partir dos campos lidos: [{slug, nome, categoria,
    categoria_nome, categoria_nova, origem}]."""
    out, folhas = [], set()

    def por(nome: str, cat: tuple[str, str, bool] | None):
        s = biblioteca.slug(nome)
        if not s:
            return
        out.append({"slug": s, "nome": nome.strip(), "categoria": cat[0] if cat else "",
                    "categoria_nome": cat[1] if cat else "", "categoria_nova": bool(cat and cat[2]), "origem": origem})
    for h in dados.get("hier") or []:
        if len(h) >= 2:
            por(h[-1], _categoria(h[0]))
        else:
            por(h[0], None)
        folhas.add(biblioteca.slug(h[-1]))
    for k in dados.get("keywords") or []:
        if biblioteca.slug(k) not in folhas:
            por(k, None)
            folhas.add(biblioteca.slug(k))
    for nome in dados.get("colecoes") or []:
        por(nome, None)
    lugar = _categoria(LUGAR)
    for campo in ("cidade", "pais"):
        if dados.get(campo):
            por(dados[campo], lugar)
    if estrelas:
        if dados.get("estrelas"):
            por(f"{dados['estrelas']} estrelas", None)
        if dados.get("cor"):
            por(f"cor {dados['cor']}", None)
    vistos, unicos = set(), []
    for t in out:
        if t["slug"] not in vistos:
            vistos.add(t["slug"])
            unicos.append(t)
    return unicos


def _juntar(a: dict, b: dict) -> dict:
    out = _vazio()
    for k in ("keywords", "hier", "colecoes"):
        out[k] = list(a.get(k) or []) + list(b.get(k) or [])
    for k in ("titulo", "legenda", "cidade", "pais", "cor"):
        out[k] = a.get(k) or b.get(k) or ""
    out["estrelas"] = a.get("estrelas") if a.get("estrelas") is not None else b.get("estrelas")
    return out


def ler_midia(arq: str, fontes: dict, lrcat: dict | None = None) -> dict[str, dict]:
    """{"lightroom": campos, "metadados": campos} lidos de um arquivo, conforme as fontes ligadas
    (`xmp` sidecar, `embutidos` XMP/IPTC no arquivo; `lrcat` já lido)."""
    lr, meta = _vazio(), _vazio()
    if fontes.get("xmp", True):
        sc = sidecar(arq)
        if sc:
            try:
                with open(sc, "rb") as fh:
                    lr = _juntar(lr, ler_xmp(fh.read()))
            except OSError:
                pass
    if lrcat:
        d = lrcat.get(os.path.normpath(arq))
        if d:
            lr = _juntar(lr, {"hier": d.get("hier") or [], "colecoes": d.get("colecoes") or []})
    if fontes.get("embutidos", True):
        meta = _juntar(xmp_embutido(arq), iptc_jpeg(arq))
    return {"lightroom": lr, "metadados": meta}


def _tem(d: dict) -> bool:
    return any(d.get(k) for k in ("keywords", "hier", "colecoes", "titulo", "legenda", "cidade", "pais"))


def _ler_tudo(ids: list[int], fontes: dict, estrelas: bool, db: str | None):
    con = abrir(db)
    linhas = con.execute(f"select id, caminho from midia where ausente = 0 and id in ({','.join('?' * len(ids))})",
                         ids).fetchall() if ids else []
    existentes = {r["slug"]: dict(r) for r in con.execute("select id, slug, nome, categoria, origem from tag")}
    con.close()
    lrcat = ler_lrcat(fontes["lrcat"]) if fontes.get("lrcat") else None
    por_midia = []
    for r in linhas:
        arq = r["caminho"] if os.path.isabs(r["caminho"]) else os.path.join(config.carregar()["acervo"]["raiz"],
                                                                         r["caminho"])
        lido = ler_midia(arq, fontes, lrcat)
        tags = tags_de(lido["lightroom"], "lightroom", estrelas) + tags_de(lido["metadados"], "metadados", estrelas)
        textos = [(campo, lido[o][campo], o) for o in ORIGENS for campo in ("titulo", "legenda") if lido[o][campo]]
        por_midia.append({"id": r["id"], "tags": tags, "textos": textos,
                          "com_dados": _tem(lido["lightroom"]) or _tem(lido["metadados"])})
    return por_midia, existentes


def _estado(t: dict, existentes: dict) -> tuple[str, dict | None]:
    e = existentes.get(t["slug"])
    if not e:
        return "nova", None
    if e["origem"] in ("marca", t["origem"]):
        return "existe", e
    return "conflito", e


def previa(ids: list[int], fontes: dict | None = None, estrelas: bool = False, db: str | None = None) -> dict:
    """O mapeamento que seria gravado, sem gravar nada: cada tag com quantas mídias recebem,
    a categoria (e se é nova) e o estado (`existe`, `nova` ou `conflito` com outra origem)."""
    fontes = {"xmp": True, "embutidos": True, "lrcat": "", **(fontes or {})}
    por_midia, existentes = _ler_tudo([int(i) for i in ids], fontes, estrelas, db)
    agreg: dict[tuple[str, str], dict] = {}
    for m in por_midia:
        for t in m["tags"]:
            k = (t["slug"], t["origem"])
            if k not in agreg:
                estado, e = _estado(t, existentes)
                agreg[k] = {**t, "n": 0, "estado": estado,
                            "existente": {"origem": e["origem"], "categoria": e["categoria"], "nome": e["nome"]} if e else None}
            agreg[k]["n"] += 1
    tags = sorted(agreg.values(), key=lambda t: (t["estado"] != "conflito", t["categoria"] == "", t["categoria"],
                                                 -t["n"], t["nome"].lower()))
    return {"midias": len(por_midia), "com_dados": sum(1 for m in por_midia if m["com_dados"]),
            "textos": sum(len(m["textos"]) for m in por_midia), "tags": tags,
            "categorias_novas": sorted({t["categoria_nome"] for t in tags if t["categoria_nova"]})}


def aplicar(ids: list[int], fontes: dict | None = None, estrelas: bool = False, escolhas: dict | None = None,
            ignorar: list[str] | None = None, db: str | None = None) -> dict:
    """Grava o mapeamento da prévia. `escolhas`: {slug: "agrupar" | "separar"} para os conflitos
    (padrão agrupar). `ignorar`: slugs que a pessoa desmarcou na prévia."""
    fontes = {"xmp": True, "embutidos": True, "lrcat": "", **(fontes or {})}
    escolhas, ignorar = escolhas or {}, set(ignorar or [])
    por_midia, existentes = _ler_tudo([int(i) for i in ids], fontes, estrelas, db)
    con = abrir(db)
    ids_tag: dict[tuple[str, str], int] = {}
    n_tags = n_marcas = n_textos = 0
    for m in por_midia:
        for t in m["tags"]:
            if t["slug"] in ignorar:
                continue
            k = (t["slug"], t["origem"])
            if k not in ids_tag:
                estado, e = _estado(t, existentes)
                if estado == "conflito" and escolhas.get(t["slug"]) == "separar":
                    s2 = f"{t['slug']}-{t['origem']}"
                    r = con.execute("select id from tag where slug = ?", (s2,)).fetchone()
                    ids_tag[k] = r["id"] if r else con.execute(
                        "insert into tag (slug, nome, categoria, origem) values (?, ?, ?, ?)",
                        (s2, f"{t['nome']} ({t['origem']})", t["categoria"], t["origem"])).lastrowid
                    n_tags += 0 if r else 1
                elif e:
                    ids_tag[k] = e["id"]
                    if not e["categoria"] and t["categoria"]:
                        con.execute("update tag set categoria = ? where id = ?", (t["categoria"], e["id"]))
                else:
                    tid = con.execute("insert into tag (slug, nome, categoria, origem) values (?, ?, ?, ?)",
                                      (t["slug"], t["nome"], t["categoria"], t["origem"])).lastrowid
                    ids_tag[k] = tid
                    existentes[t["slug"]] = {"id": tid, "slug": t["slug"], "nome": t["nome"],
                                             "categoria": t["categoria"], "origem": t["origem"]}
                    n_tags += 1
            n_marcas += con.execute("insert or ignore into midia_tag (midia_id, tag_id, por) values (?, ?, ?)",
                                    (m["id"], ids_tag[k], t["origem"])).rowcount
        for campo, texto, origem in m["textos"]:
            con.execute("insert or replace into midia_texto (midia_id, campo, texto, origem) values (?, ?, ?, ?)",
                        (m["id"], campo, texto, origem))
            n_textos += 1
    con.commit()
    con.close()
    return {"tags_novas": n_tags, "marcacoes": n_marcas, "textos": n_textos, "midias": len(por_midia)}


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("acao", choices=["previa", "aplicar"])
    ap.add_argument("--ids", required=True, help="ids das mídias, separados por vírgula")
    ap.add_argument("--lrcat", default="")
    ap.add_argument("--sem-xmp", action="store_true")
    ap.add_argument("--sem-embutidos", action="store_true")
    ap.add_argument("--estrelas", action="store_true")
    a = ap.parse_args()
    ids = [int(x) for x in a.ids.split(",") if x.strip()]
    fontes = {"xmp": not a.sem_xmp, "embutidos": not a.sem_embutidos, "lrcat": a.lrcat}
    r = previa(ids, fontes, a.estrelas) if a.acao == "previa" else aplicar(ids, fontes, a.estrelas)
    print(json.dumps(r, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
