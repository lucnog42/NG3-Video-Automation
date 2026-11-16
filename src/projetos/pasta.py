#!/usr/bin/env python3
"""Projeto do FilmCraft a partir de uma pasta: os vídeos em ordem, só com a fala, legenda presa à
fala, as fotos como b-roll e a identidade da marca (abertura, logo, SFX, encerramento).

É o caminho do botão "Gerar projeto" (`gerar.gerar_de_entradas` + o modelo base da marca), para uma
pasta qualquer — inclusive fora do acervo, como um material que chegou agora:

  1. transcreve o que ainda não tem `.words.json` (o mesmo `legendar.py`, com o refino pela voz);
  2. vídeo sem fala (torcida, entrada em campo) entra inteiro e sem legenda — o Whisper alucina
     ali, então o que ele ouviu é descartado;
  3. vídeo com fala passa pelo silencecut (onde há fala = as palavras, com J/L entre as falas);
  4. as fotos da pasta entram numa faixa "Fotos" por cima do vídeo, a partir da primeira fala que
     menciona uma das palavras de `--gatilho` (padrão: foto) — senão no começo do vídeo mais falado.

Os arquivos de trabalho (transcrição, cortes) ficam em `dados/marcas/<marca>/`; na pasta só entra
o projeto (`<nome>.fcproj` e o `.trechos.json` que o plugin Legendar usa para refazer a legenda).

    python3 src/projetos/pasta.py <pasta>                       # mostra o plano
    python3 src/projetos/pasta.py <pasta> --aplicar
    python3 src/projetos/pasta.py <pasta> --ordem 01.mp4,03.mp4,02.mp4 --titulo "..." --aplicar
"""
from __future__ import annotations

import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))))  # src/

import argparse
import json
import os
import subprocess
import sys
import tempfile
import unicodedata

from nucleo import caminhos  # noqa: E402
from transcricao import legendas  # noqa: E402

AQUI = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.dirname(AQUI)
RAIZ = os.path.dirname(SRC)
SILENCECUT = os.path.join(RAIZ, "plugins", "silencecut", "silencecut.py")
BASE = caminhos.acervo()

VIDEOS = {".mp4", ".mov", ".m4v", ".mkv", ".webm"}
IMAGENS = {".jpg", ".jpeg", ".png"}
PULAR = {"saidas", "silencecut"}            # saídas de render e cortes antigos não são material

# um vídeo "tem fala" quando o Whisper achou palavras em quantidade de gente falando
MIN_PALAVRAS = 8
MIN_POR_MINUTO = 25.0


# ------------------------------------------------------------------ pasta
def abs_(p: str) -> str:
    p = os.path.expanduser(p)
    return os.path.abspath(p if os.path.isabs(p) else os.path.join(BASE, p))


def listar(pasta: str, ordem: list[str] | None = None) -> tuple[list[str], list[str]]:
    """Vídeos e imagens da pasta (e das subpastas, como videos/ e imagens/ de um post), em ordem de
    nome; `ordem` (nomes ou caminhos relativos à pasta) põe esses primeiro, na ordem dada."""
    videos, imagens = [], []
    for raiz, dirs, arqs in os.walk(pasta):
        dirs[:] = sorted(d for d in dirs if d not in PULAR and not d.startswith("."))
        for a in arqs:
            ext = os.path.splitext(a)[1].lower()
            c = os.path.join(raiz, a)
            if ext in VIDEOS and not a.endswith("_sem_silencio.mp4"):
                videos.append(c)
            elif ext in IMAGENS:
                imagens.append(c)
    chave = lambda c: os.path.relpath(c, pasta)  # noqa: E731
    videos.sort(key=chave)
    imagens.sort(key=chave)
    if ordem:
        def pos(c: str) -> tuple[int, str]:
            r = chave(c)
            for i, o in enumerate(ordem):
                if o in (r, os.path.basename(c)):
                    return (i, r)
            return (len(ordem), r)
        videos.sort(key=pos)
        imagens.sort(key=pos)
    return videos, imagens


# ------------------------------------------------------------------ fala
def caminho_words(video: str) -> str:
    from transcricao import legendar
    return legendar.caminho_saida(video, ".words.json")


def palavras_de(video: str) -> list[tuple[float, float, str]]:
    w = caminho_words(video)
    return legendas.ler_words_json(w) if os.path.exists(w) else []


def tem_fala(palavras: list, duracao: float) -> bool:
    """Gente falando, não três palavras soltas que o Whisper tirou da torcida."""
    if len(palavras) < MIN_PALAVRAS:
        return False
    return len(palavras) / max(duracao / 60.0, 1e-6) >= MIN_POR_MINUTO


def transcrever(videos: list[str]) -> int:
    """Roda o legendar.py (venv do ASR, refino pela voz) nos que ainda não têm transcrição."""
    faltam = [v for v in videos if not os.path.exists(caminho_words(v))]
    if not faltam:
        return 0
    cmd = [sys.executable, os.path.join(SRC, "transcricao", "legendar.py"), "--aplicar"]
    for v in faltam:
        cmd += ["--arquivo", v]
    print(f"▶ transcrevendo {len(faltam)} vídeo(s)", flush=True)
    subprocess.run(cmd, check=False)
    return len(faltam)


def pasta_dos_cortes(video: str) -> str:
    """Onde o silencecut grava (.cuts.json, .otio): nos dados da marca, espelhando o caminho."""
    rel = os.path.relpath(video, BASE)
    if rel.startswith(".."):
        rel = os.path.join("externos", os.path.abspath(video).lstrip(os.sep))
    d = caminhos.dados("silencecut", os.path.dirname(rel))
    os.makedirs(d, exist_ok=True)
    return d


def regioes(palavras: list, silencio: float) -> list[list[float]]:
    """Onde há fala: palavras com pausa menor que `silencio` viram uma região."""
    regs: list[list[float]] = []
    for a, b, _ in sorted(palavras):
        if regs and a - regs[-1][1] < silencio:
            regs[-1][1] = max(regs[-1][1], b)
        else:
            regs.append([a, b])
    return regs


def cortar(video: str, palavras: list, preset: str = "podcast", silencio: float = 0.8) -> list[tuple] | None:
    """Trechos de fala com J/L, pelo silencecut: [(som_ini, som_fim, img_ini, img_fim)]."""
    saida = pasta_dos_cortes(video)
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as fh:
        json.dump(regioes(palavras, silencio), fh)
    try:
        r = subprocess.run([sys.executable, SILENCECUT, video, "--preset", preset, "-s", f"{silencio}",
                            "--regioes", fh.name, "-o", saida], capture_output=True, text=True)
    finally:
        os.unlink(fh.name)
    cuts = os.path.join(saida, os.path.splitext(os.path.basename(video))[0] + ".cuts.json")
    if r.returncode != 0 or not os.path.exists(cuts):
        print(f"  {os.path.basename(video)}: silencecut falhou, entra inteiro ({(r.stderr or '').strip()[-160:]})",
              flush=True)
        return None
    with open(cuts, encoding="utf-8") as fh:
        segs = json.load(fh).get("speech_segments") or []
    return [(float(s["start"]), float(s["end"]), float(s.get("v_start", s["start"])),
             float(s.get("v_end", s["end"]))) for s in segs] or None


# ------------------------------------------------------------------ fotos
def _norm(t: str) -> str:
    t = unicodedata.normalize("NFD", t.lower())
    return "".join(c for c in t if c.isalnum() and not unicodedata.combining(c))


def inicio_das_fotos(entradas: list[dict], gatilhos: list[str], dur_total: float) -> float:
    """Segundo da timeline onde as fotos entram: a primeira palavra-gatilho com espaço para todas
    as fotos antes do fim daquele vídeo; senão 1 s depois do começo do vídeo com mais fala."""
    alvo = {_norm(g) for g in gatilhos if g.strip()}
    off, melhor, mais = 0.0, None, -1
    for e in entradas:
        trechos = [(t[0], t[1]) for t in e["trechos"]] if e.get("trechos") else None
        dur = sum(t[1] - t[0] for t in trechos) if trechos else e["duracao"]
        ps = legendas.mapear_palavras(e.get("palavras") or [], trechos)
        for a, _, t in ps:
            if alvo and any(_norm(t).startswith(g) for g in alvo) and a + dur_total <= dur:
                return round(off + a, 3)
        if len(ps) > mais:
            mais, melhor = len(ps), off + min(1.0, dur)
        off += dur
    return round(melhor or 0.0, 3)


# ------------------------------------------------------------------ montar
def planejar(pasta: str, ordem: list[str] | None = None, cortar_silencio: bool = True,
             preset: str = "podcast") -> tuple[list[dict], list[str]]:
    """As entradas do projeto (como as de um roteiro) e as fotos."""
    from projetos import modelo as MF
    videos, imagens = listar(pasta, ordem)
    entradas = []
    for v in videos:
        dur = MF.seg(MF.probe(v)["duration"])
        ps = palavras_de(v)
        fala = tem_fala(ps, dur)
        trechos = cortar(v, ps, preset) if (fala and cortar_silencio) else None
        entradas.append({"video": v, "palavras": ps if fala else [], "trechos": trechos, "duracao": dur,
                         "fala": fala})
    return entradas, imagens


def montar(pasta: str, *, nome: str | None = None, titulo: str | None = None, subtitulo: str = "",
           ordem: list[str] | None = None, transcrever_antes: bool = True, cortar_silencio: bool = True,
           preset: str = "podcast", fotos: bool = True, gatilhos: list[str] | None = None,
           dur_foto: float = 3.0, aplicar: bool = False) -> dict:
    from projetos import gerar
    from nucleo import config
    pasta = abs_(pasta)
    if not os.path.isdir(pasta):
        raise ValueError(f"pasta inexistente: {pasta}")
    nome = nome or os.path.basename(pasta.rstrip(os.sep))
    videos, _ = listar(pasta, ordem)
    if not videos:
        raise ValueError("a pasta não tem vídeo")
    if transcrever_antes and aplicar:
        transcrever(videos)
    entradas, imagens = planejar(pasta, ordem, cortar_silencio, preset)
    if not fotos:
        imagens = []
    inicio = inicio_das_fotos(entradas, gatilhos or ["foto"], dur_foto * len(imagens)) if imagens else 0.0
    plano = {"projeto": os.path.join(pasta, f"{nome}.fcproj"), "fotos_em": inicio, "fotos": imagens,
             "videos": [{"video": e["video"], "fala": e["fala"], "palavras": len(e["palavras"]),
                         "trechos": len(e["trechos"] or []), "duracao": round(e["duracao"], 2)} for e in entradas]}
    if not aplicar:
        return plano
    mont = config.carregar()["montagem"]
    ending = mont.get("ending_padrao")
    logo = mont.get("logo")
    ok, erro = gerar.gerar_de_entradas(
        nome, pasta, entradas, titulo or nome, subtitulo,
        logo=logo if logo and os.path.exists(logo) else None,
        ending=ending if ending and os.path.exists(ending) else None,
        broll={"imagens": imagens, "inicio": inicio, "duracao": dur_foto} if imagens else None)
    plano.update(ok=ok, erro=erro)
    return plano


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("pasta")
    ap.add_argument("--aplicar", action="store_true")
    ap.add_argument("--nome", help="nome do projeto (padrão: o da pasta)")
    ap.add_argument("--titulo")
    ap.add_argument("--subtitulo", default="")
    ap.add_argument("--ordem", help="vídeos (e fotos) nessa ordem, separados por vírgula")
    ap.add_argument("--sem-transcrever", action="store_true", help="usa só as transcrições que já existem")
    ap.add_argument("--sem-cortar", action="store_true", help="vídeos inteiros (sem tirar o silêncio)")
    ap.add_argument("--preset", default="podcast", help="preset do silencecut (margens e J/L)")
    ap.add_argument("--sem-fotos", action="store_true")
    ap.add_argument("--gatilho", action="append", default=[], help="palavra que chama as fotos (repita)")
    ap.add_argument("--foto-s", type=float, default=3.0, help="segundos por foto")
    a = ap.parse_args()
    r = montar(a.pasta, nome=a.nome, titulo=a.titulo, subtitulo=a.subtitulo,
               ordem=[x.strip() for x in a.ordem.split(",")] if a.ordem else None,
               transcrever_antes=not a.sem_transcrever, cortar_silencio=not a.sem_cortar, preset=a.preset,
               fotos=not a.sem_fotos, gatilhos=a.gatilho or ["foto"], dur_foto=a.foto_s, aplicar=a.aplicar)
    for v in r["videos"]:
        estado = f"fala, {v['trechos']} trechos" if v["fala"] else "sem fala (inteiro, sem legenda)"
        print(f"  {os.path.basename(v['video']):<24} {v['duracao']:>7.1f}s  {estado}")
    if r["fotos"]:
        print(f"  {len(r['fotos'])} foto(s) a partir de {r['fotos_em']:.1f}s")
    if "ok" not in r:
        print(f"DRY-RUN → {r['projeto']} (use --aplicar)")
    elif r["ok"]:
        print(f"projeto: {r['projeto']}")
    else:
        sys.exit(f"falhou: {r['erro']}")


if __name__ == "__main__":
    main()
