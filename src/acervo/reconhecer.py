#!/usr/bin/env python3
"""Metadados e reconhecimento de imagem: sugestões de tag para fotos e vídeos do catálogo.

Para cada mídia:

  1. **metadados** (o arquivo só é lido): data, câmera, rotação, duração, resolução e GPS — do
     ffprobe nos vídeos e do EXIF nas fotos. Ficam em `midia_meta`; data e câmera entram na busca.
  2. **GPS → lugar**: se as coordenadas caem no raio de um lugar de `lugares.toml` da marca, a
     tag do lugar entra direto (por "metadados"): é medida, não palpite.
  3. **IA (SigLIP, local)**: compara quadros do vídeo (ou a foto) com as tags da marca escritas
     como frase ("uma foto de Camp Nou") e grava as mais prováveis como **sugestões pendentes**
     em `midia_sugestao`. A pessoa aceita ou recusa no painel; recusada não volta. A categoria
     "pessoa" fica de fora: a IA não reconhece gente.

Roda no ambiente `sep` (torch + transformers), como tarefa do painel:

    <python do sep> src/acervo/reconhecer.py --lista ids.txt
"""
from __future__ import annotations

import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))))  # src/

import argparse
import json
import math
import os
import re
import subprocess
import sys
import time

from acervo import biblioteca  # noqa: E402
from nucleo import caminhos, config  # noqa: E402

ESQUEMA = """
create table if not exists midia_meta (midia_id integer not null, chave text not null, valor text,
    primary key (midia_id, chave));
create table if not exists midia_sugestao (midia_id integer not null, tag_id integer not null, score real,
    estado text not null default 'pendente', modelo text default '', criado_em real,
    primary key (midia_id, tag_id));
"""


def abrir(db: str | None = None):
    con = biblioteca.abrir(db)
    con.executescript(ESQUEMA)
    return con


def cfg() -> dict:
    try:
        return config.carregar().get("reconhecimento") or {}
    except Exception:                       # pragma: no cover
        return {}


def _abs(caminho: str) -> str:
    return caminho if os.path.isabs(caminho) else os.path.join(config.carregar()["acervo"]["raiz"], caminho)


# ------------------------------------------------------------ metadados
_ISO6709 = re.compile(r"([+-]\d+(?:\.\d+)?)([+-]\d+(?:\.\d+)?)")


def gps_iso6709(texto: str) -> tuple[float, float] | None:
    """"+41.3809+002.1228+012.000/" (o que celulares gravam no MP4) -> (lat, lon)."""
    m = _ISO6709.search(texto or "")
    if not m:
        return None
    lat, lon = float(m.group(1)), float(m.group(2))
    return (lat, lon) if -90 <= lat <= 90 and -180 <= lon <= 180 else None


def metadados_video(arq: str) -> dict:
    r = subprocess.run(["ffprobe", "-v", "error", "-show_format", "-show_streams", "-of", "json", arq],
                       capture_output=True, text=True, timeout=60)
    d = json.loads(r.stdout or "{}")
    tags = {k.lower(): v for k, v in ((d.get("format") or {}).get("tags") or {}).items()}
    v = next((s for s in d.get("streams") or [] if s.get("codec_type") == "video"), {})
    for k, x in (v.get("tags") or {}).items():
        tags.setdefault(k.lower(), x)
    out = {"duracao": (d.get("format") or {}).get("duration"), "largura": v.get("width"), "altura": v.get("height"),
           "codec": v.get("codec_name"), "fps": v.get("avg_frame_rate")}
    rot = next((sd.get("rotation") for sd in v.get("side_data_list") or [] if "rotation" in sd), tags.get("rotate"))
    if rot not in (None, "", 0, "0"):
        out["rotacao"] = rot
    out["data"] = tags.get("creation_time") or tags.get("com.apple.quicktime.creationdate") or tags.get("date")
    out["camera"] = " ".join(x for x in (tags.get("com.apple.quicktime.make") or tags.get("make") or tags.get("com.android.manufacturer"),
                                         tags.get("com.apple.quicktime.model") or tags.get("model") or tags.get("com.android.model")) if x)
    loc = tags.get("location") or tags.get("com.apple.quicktime.location.iso6709") or tags.get("location-eng")
    g = gps_iso6709(loc) if loc else None
    if g:
        out["lat"], out["lon"] = g
    return {k: v for k, v in out.items() if v not in (None, "")}


def _graus(v) -> float:
    d, m, s = (float(x) for x in v)
    return d + m / 60 + s / 3600


def metadados_imagem(arq: str) -> dict:
    """EXIF pelo Pillow (no ambiente sep). Sem Pillow ou sem EXIF: só o que der."""
    try:
        from PIL import ExifTags, Image
    except ImportError:
        return {}
    out: dict = {}
    with Image.open(arq) as im:
        out["largura"], out["altura"] = im.size
        ex = im.getexif()
        if not ex:
            return out
        nomes = {ExifTags.TAGS.get(k, k): v for k, v in ex.items()}
        sub = ex.get_ifd(0x8769) or {}
        nomes.update({ExifTags.TAGS.get(k, k): v for k, v in sub.items()})
        out["data"] = nomes.get("DateTimeOriginal") or nomes.get("DateTime")
        out["camera"] = " ".join(str(x).strip() for x in (nomes.get("Make"), nomes.get("Model")) if x)
        if nomes.get("Orientation") not in (None, 1):
            out["rotacao"] = nomes.get("Orientation")
        gps = ex.get_ifd(0x8825) or {}
        try:
            if gps.get(2) and gps.get(4):
                lat, lon = _graus(gps[2]), _graus(gps[4])
                if gps.get(1) == "S":
                    lat = -lat
                if gps.get(3) == "W":
                    lon = -lon
                out["lat"], out["lon"] = round(lat, 6), round(lon, 6)
        except (TypeError, ValueError, ZeroDivisionError):
            pass
    return {k: v for k, v in out.items() if v not in (None, "")}


def distancia_m(a: tuple[float, float], b: tuple[float, float]) -> float:
    la1, lo1, la2, lo2 = map(math.radians, (a[0], a[1], b[0], b[1]))
    h = math.sin((la2 - la1) / 2) ** 2 + math.cos(la1) * math.cos(la2) * math.sin((lo2 - lo1) / 2) ** 2
    return 6371000 * 2 * math.asin(math.sqrt(h))


def lugares_perto(lat: float, lon: float, lugares: dict | None = None, raio_padrao: float = 300) -> list[str]:
    """Os slugs dos lugares da marca cujo raio contém o ponto (do mais perto ao mais longe)."""
    lugares = config.carregar().get("lugares") or {} if lugares is None else lugares
    achados = []
    for slug, l in lugares.items():
        try:
            d = distancia_m((lat, lon), (float(l["lat"]), float(l["lon"])))
        except (KeyError, TypeError, ValueError):
            continue
        if d <= float(l.get("raio_m") or raio_padrao):
            achados.append((d, slug))
    return [s for _, s in sorted(achados)]


def gravar_metadados(con, midia_id: int, meta: dict) -> None:
    con.execute("delete from midia_meta where midia_id = ?", (midia_id,))
    con.executemany("insert into midia_meta (midia_id, chave, valor) values (?, ?, ?)",
                    [(midia_id, k, str(v)) for k, v in meta.items()])


# ------------------------------------------------------------ IA
def rotulos(c: dict | None = None) -> list[dict]:
    """As tags das categorias que a IA pode sugerir, com a frase que o modelo compara."""
    c = c or cfg()
    frase = c.get("frase") or "uma foto de {nome}"
    frases = (config.carregar().get("tags") or {}).get("ia") or {}
    out = []
    for cat, d in biblioteca.categorias_marca().items():
        if cat not in (c.get("categorias") or []):
            continue
        for slug, nome in (d.get("tags") or {}).items():
            out.append({"slug": slug, "nome": nome, "categoria": cat, "multipla": bool(d.get("multipla", True)),
                        "texto": (frases.get(cat) or {}).get(slug) or frase.format(nome=nome)})
    return out


def escolher(logits: dict[str, float], rots: list[dict], c: dict | None = None) -> list[tuple[str, float]]:
    """Por categoria: a parte de cada tag na categoria (softmax dos logits só entre elas). O 1º entra
    se levar `relativo_min`; numa categoria que aceita várias, o 2º se levar `segundo_min`. Nada
    entra se a probabilidade absoluta (sigmoide) do 1º for menor que `evidencia_min`."""
    base = {**{"relativo_min": 0.55, "segundo_min": 0.30, "evidencia_min": 0.005}, **(c or {})}
    out = []
    for cat in dict.fromkeys(r["categoria"] for r in rots):
        c = {**base, **((base.get("por_categoria") or {}).get(cat) or {})}
        da_cat = [(logits.get(r["slug"], -1e9), r) for r in rots if r["categoria"] == cat]
        if not da_cat:
            continue
        topo = max(l for l, _ in da_cat)
        soma = sum(math.exp(l - topo) for l, _ in da_cat)
        rel = sorted(((math.exp(l - topo) / soma, l, r) for l, r in da_cat), key=lambda x: -x[0])
        p0, l0, r0 = rel[0]
        if 1 / (1 + math.exp(-l0)) < float(c["evidencia_min"]) or p0 < float(c["relativo_min"]):
            continue
        out.append((r0["slug"], round(p0, 3)))
        if r0["multipla"] and len(rel) > 1 and rel[1][0] >= float(c["segundo_min"]):
            out.append((rel[1][2]["slug"], round(rel[1][0], 3)))
    return out


class Modelo:
    """SigLIP carregado uma vez; os textos das tags são codificados uma vez só."""

    def __init__(self, nome: str, textos: list[str], dispositivo: str = "auto", pasta: str = ""):
        import torch
        from transformers import AutoModel, AutoProcessor
        if pasta:
            os.environ.setdefault("HF_HOME", pasta)
        self.torch = torch
        self.proc = AutoProcessor.from_pretrained(nome)
        self.modelo = AutoModel.from_pretrained(nome).eval()
        self.disp = "cpu"
        if dispositivo in ("auto", "cuda") and torch.cuda.is_available():
            try:
                self.modelo = self.modelo.to("cuda").half()
                self.disp = "cuda"
            except RuntimeError:                # sem VRAM (outro programa usando a placa)
                torch.cuda.empty_cache()
                self.modelo = self.modelo.float().to("cpu")
        with torch.no_grad():
            t = self.proc(text=textos, padding="max_length", return_tensors="pt")
            t = {k: v.to(self.disp) for k, v in t.items()}
            e = self.modelo.get_text_features(**t)
            e = getattr(e, "pooler_output", e)
            self.texto = e / e.norm(dim=-1, keepdim=True)

    def logits(self, imagens) -> list[float]:
        """Para cada tag, o maior logit entre as imagens (o melhor quadro do vídeo)."""
        torch = self.torch
        with torch.no_grad():
            x = self.proc(images=imagens, return_tensors="pt")["pixel_values"].to(self.disp)
            if self.disp == "cuda":
                x = x.half()
            e = self.modelo.get_image_features(pixel_values=x)
            e = getattr(e, "pooler_output", e)
            e = e / e.norm(dim=-1, keepdim=True)
            logits = e @ self.texto.T * self.modelo.logit_scale.exp() + self.modelo.logit_bias
            return logits.float().max(dim=0).values.tolist()


def quadros(arq: str, tipo: str, n: int):
    """Imagens para o modelo: a foto, ou n quadros espalhados pelo vídeo (sem os 5% das pontas)."""
    import io
    from PIL import Image
    if tipo == "imagem":
        with Image.open(arq) as im:
            return [im.convert("RGB").copy()]
    try:
        dur = float(json.loads(subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration", "-of", "json",
                                               arq], capture_output=True, text=True, timeout=60).stdout)["format"]["duration"])
    except (ValueError, KeyError, TypeError):
        dur = 0.0
    out = []
    for k in range(max(1, n)):
        t = dur * (0.05 + 0.9 * (k + 0.5) / max(1, n)) if dur else 0.0
        r = subprocess.run(["ffmpeg", "-nostdin", "-loglevel", "error", "-ss", f"{t:.2f}", "-i", arq, "-frames:v", "1",
                            "-vf", "scale=384:-2", "-f", "image2pipe", "-vcodec", "png", "-"], capture_output=True, timeout=120)
        if r.returncode == 0 and r.stdout:
            out.append(Image.open(io.BytesIO(r.stdout)).convert("RGB"))
    return out


def gravar_sugestoes(con, midia_id: int, sugestoes: list[tuple[str, float]], modelo: str) -> int:
    """Pendentes novas; nada para tag que a mídia já tem ou que a pessoa já recusou."""
    ja = {r[0] for r in con.execute("select t.slug from midia_tag mt join tag t on t.id = mt.tag_id where mt.midia_id = ?",
                                    (midia_id,))}
    con.execute("delete from midia_sugestao where midia_id = ? and estado = 'pendente'", (midia_id,))
    n = 0
    for slug, p in sugestoes:
        if slug in ja:
            continue
        tid = biblioteca._garantir_tag(con, slug)
        r = con.execute("select estado from midia_sugestao where midia_id = ? and tag_id = ?", (midia_id, tid)).fetchone()
        if r and r[0] != "pendente":
            continue
        con.execute("insert or replace into midia_sugestao (midia_id, tag_id, score, estado, modelo, criado_em) "
                    "values (?, ?, ?, 'pendente', ?, ?)", (midia_id, tid, p, modelo, time.time()))
        n += 1
    return n


def reconhecer(ids: list[int], db: str | None = None, usar_ia: bool = True) -> list[dict]:
    c = cfg()
    con = abrir(db)
    linhas = con.execute(f"select id, caminho, tipo, nome_exibicao from midia where ausente = 0 and id in "
                         f"({','.join('?' * len(ids))})", [int(i) for i in ids]).fetchall() if ids else []
    con.close()
    rots = rotulos(c)
    modelo = None
    out = []
    print(f"[0/{len(linhas)}] {len(rots)} tags da marca para comparar", flush=True)
    for k, (mid, caminho, tipo, nome) in enumerate(linhas, 1):
        arq = _abs(caminho)
        r: dict = {"midia": mid}
        try:
            meta = metadados_video(arq) if tipo == "video" else metadados_imagem(arq)
            con = abrir(db)
            gravar_metadados(con, mid, meta)
            if "lat" in meta:
                for slug in lugares_perto(float(meta["lat"]), float(meta["lon"]), raio_padrao=float(c.get("raio_gps_m") or 300)):
                    biblioteca._marcar(con, mid, biblioteca._garantir_tag(con, slug), "metadados")
                    r.setdefault("gps", []).append(slug)
            con.commit()
            con.close()
            if usar_ia and rots:
                if modelo is None:
                    pasta = c.get("modelos_dir") or os.path.join(os.path.dirname(caminhos.venv("sep")), "hf")
                    modelo = Modelo(c.get("modelo"), [x["texto"] for x in rots], str(c.get("dispositivo") or "auto"), pasta)
                    print(f"  [modelo] {c.get('modelo')} em {modelo.disp}", flush=True)
                imgs = quadros(arq, tipo, int(c.get("quadros_por_video") or 4))
                if imgs:
                    lg = dict(zip((x["slug"] for x in rots), modelo.logits(imgs)))
                    sug = escolher(lg, rots, c)
                    if r.get("gps"):          # o GPS já disse onde é: a IA não chuta outro lugar
                        de_lugar = {x["slug"] for x in rots if x["categoria"] == "lugar"}
                        sug = [x for x in sug if x[0] not in de_lugar]
                    con = abrir(db)
                    r["sugeridas"] = gravar_sugestoes(con, mid, sug, c.get("modelo") or "")
                    con.commit()
                    con.close()
                    r["melhores"] = sug[:4]
        except Exception as e:
            r["erro"] = f"{type(e).__name__}: {str(e)[:160]}"
        out.append(r)
        nome = nome or os.path.basename(caminho)
        if "erro" in r:
            print(f"[{k}/{len(linhas)}] {nome}: erro {r['erro']}", flush=True)
        else:
            extra = f" · GPS: {', '.join(r['gps'])}" if r.get("gps") else ""
            melhores = ", ".join(f"{s} {p:.2f}" for s, p in r.get("melhores") or [])
            print(f"[{k}/{len(linhas)}] {nome}: {r.get('sugeridas', 0)} sugestão(ões){extra} ({melhores})", flush=True)
    return out


# ------------------------------------------------------------ aceite (painel, python do sistema)
def decidir(ids: list[int], aceitar: bool, tags: list[str] | None = None, db: str | None = None) -> int:
    """Aceitar vira tag da mídia (por "ia"); recusar fica guardado para não voltar."""
    ids = [int(i) for i in ids]
    if not ids:
        return 0
    con = abrir(db)
    q = ",".join("?" * len(ids))
    sql = (f"select s.midia_id, s.tag_id from midia_sugestao s join tag t on t.id = s.tag_id "
           f"where s.estado = 'pendente' and s.midia_id in ({q})")
    args: list = list(ids)
    if tags:
        sql += f" and t.slug in ({','.join('?' * len(tags))})"
        args += [biblioteca.slug(t) for t in tags]
    pares = con.execute(sql, args).fetchall()
    for mid, tid in pares:
        if aceitar:
            biblioteca._marcar(con, mid, tid, "ia")
        con.execute("update midia_sugestao set estado = ? where midia_id = ? and tag_id = ?",
                    ("aceita" if aceitar else "recusada", mid, tid))
    con.commit()
    con.close()
    return len(pares)


def garantir_venv() -> None:
    """A IA vive no ambiente `sep` (torch + transformers): o painel chama com o python do sistema."""
    try:
        import torch  # noqa: F401
        import transformers  # noqa: F401
        return
    except ImportError:
        pass
    alvo = caminhos.python_de("sep")
    if os.path.exists(alvo) and os.path.realpath(sys.executable) != os.path.realpath(alvo):
        os.environ.setdefault("HF_HUB_DISABLE_SYMLINKS_WARNING", "1")
        os.execv(alvo, [alvo, os.path.abspath(__file__)] + sys.argv[1:])
    sys.exit(f"sem o ambiente da IA em {caminhos.venv('sep')} (torch + transformers)")


def main() -> None:
    garantir_venv()
    ap = argparse.ArgumentParser()
    ap.add_argument("--id", type=int, action="append", default=[])
    ap.add_argument("--lista")
    ap.add_argument("--sem-ia", action="store_true", help="só os metadados e o GPS")
    a = ap.parse_args()
    ids = list(a.id)
    if a.lista:
        with open(a.lista, encoding="utf-8") as fh:
            ids += [int(x) for x in fh.read().split() if x.strip()]
    r = reconhecer(ids, usar_ia=not a.sem_ia)
    ok = sum(1 for x in r if "erro" not in x)
    print(f"{ok} de {len(r)} mídia(s) · {sum(x.get('sugeridas', 0) for x in r)} sugestão(ões) · "
          f"{sum(len(x.get('gps') or []) for x in r)} lugar(es) pelo GPS")


if __name__ == "__main__":
    main()
