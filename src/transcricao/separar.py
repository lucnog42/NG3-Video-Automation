#!/usr/bin/env python3
"""Separa a voz da música antes de transcrever.

Música de fundo atrapalha o Whisper (palavra inventada, borda errada) e o VAD (a música "é voz").
Aqui:

  1. `tem_musica(wav)`: detecção barata, sem GPU. Olha os quadros **fora da voz** (as regiões do
     `voz.analisar`, ou o áudio todo se não houver): quadro alto e **tonal** (planura espectral
     baixa, como nota musical) conta como música; ruído, torcida e silêncio não. Se a fração de
     quadros musicais passar de `separacao.limiar_musica`, separa.
  2. `separar(video)`: roda o [audio-separator](https://github.com/nomadkaraoke/python-audio-separator)
     no ambiente `sep` (torch/onnx brigam com as versões fixas do ASR), só com o stem de voz, e
     devolve um wav 16 kHz mono no cache da marca. Modelo padrão RoFormer; se falhar (VRAM),
     tenta o leve. O stem de música não é guardado (`guardar_musica = false`).
  3. `preparar(video, wav, modo)`: decide e devolve o áudio para o Whisper + o que foi feito,
     que vai para o words.json (`audio.fonte`: "separada" | "original").

Os tempos continuam valendo para o vídeo original (mesma duração). Sem o ambiente `sep`, a
transcrição segue com o áudio original e o motivo fica registrado.

    python3 src/transcricao/separar.py medir <video>      # só a detecção (precisa de numpy)
    <python do sep> src/transcricao/separar.py rodar <wav> <pasta> <modelo>   # uso interno
"""
from __future__ import annotations

import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))))  # src/

import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile

from nucleo import caminhos  # noqa: E402

SR = 16000
JANELA = 1024            # 64 ms a 16 kHz
PASSO = 512              # 32 ms
TONAL = 0.10             # planura espectral abaixo da qual o quadro é "nota" (e não ruído)
ALTO_DB = -30.0          # dBFS: pausa da fala mais alta que isso tem som de fundo (música costuma ficar entre -25 e -5)


def cfg() -> dict:
    try:
        from nucleo import config
        return config.carregar().get("separacao") or {}
    except Exception:                       # pragma: no cover
        return {}


# ------------------------------------------------------------ detecção
def _carregar(wav_ou_array):
    import numpy as np
    if not isinstance(wav_ou_array, str):
        return np.asarray(wav_ou_array, dtype=np.float32)
    r = subprocess.run(["ffmpeg", "-nostdin", "-hide_banner", "-loglevel", "error", "-i", wav_ou_array,
                        "-vn", "-ac", "1", "-ar", str(SR), "-f", "f32le", "-"], capture_output=True)
    if r.returncode != 0:
        raise RuntimeError(f"ffmpeg não decodificou {wav_ou_array}")
    return np.frombuffer(r.stdout, dtype=np.float32).copy()


def tem_musica(wav_ou_array, regioes_voz: list[tuple[float, float]] | None = None,
               limiar: float | None = None) -> dict:
    """{"fracao": 0–1, "musica": bool, "quadros": n}: quanto do áudio fora da voz é tonal e alto."""
    import numpy as np
    x = _carregar(wav_ou_array)
    limiar = float(cfg().get("limiar_musica", 0.25) if limiar is None else limiar)
    if len(x) < JANELA * 4:
        return {"fracao": 0.0, "musica": False, "quadros": 0}
    n = 1 + (len(x) - JANELA) // PASSO
    idx = np.arange(JANELA)[None, :] + PASSO * np.arange(n)[:, None]
    q = x[idx] * np.hanning(JANELA)[None, :].astype(np.float32)
    esp = np.abs(np.fft.rfft(q, axis=1)) ** 2 + 1e-12      # potência: ruído branco ≈ 0,56; nota ≪ 0,1
    # só 100 Hz–5 kHz: onde estão as notas e a voz (fora disso é ronco e chiado)
    f = np.fft.rfftfreq(JANELA, 1 / SR)
    banda = esp[:, (f >= 100) & (f <= 5000)]
    planura = np.exp(np.log(banda).mean(axis=1)) / banda.mean(axis=1)
    rms_db = 20 * np.log10(np.sqrt((q * q).mean(axis=1)) + 1e-9)
    t = (np.arange(n) * PASSO + JANELA / 2) / SR
    fora = np.ones(n, dtype=bool)
    for a, b in regioes_voz or []:
        fora &= ~((t >= a) & (t < b))
    if fora.sum() < 20:              # fala contínua: mede no áudio todo (a voz também é tonal,
        fora[:] = True               # por isso o limiar fica mais alto nesse caso)
        limiar = max(limiar, 0.6)
    # limiar absoluto: música contínua de fundo não tem "silêncio" para servir de referência
    alto = rms_db > ALTO_DB
    musical = fora & alto & (planura < TONAL)
    fracao = float(musical.sum() / max(int(fora.sum()), 1))
    return {"fracao": round(fracao, 3), "musica": fracao >= limiar, "quadros": int(fora.sum())}


# ------------------------------------------------------------ separação
def disponivel() -> bool:
    return os.path.exists(caminhos.python_de("sep"))


def _chave(video: str) -> str:
    try:
        st = os.stat(video)
        base = f"{os.path.abspath(video)}|{st.st_size}|{int(st.st_mtime)}"
    except OSError:
        base = os.path.abspath(video)
    return hashlib.sha1(base.encode()).hexdigest()[:16]


def cache_voz(video: str) -> str:
    pasta = caminhos.dados(cfg().get("cache") or "cache/separacao")
    os.makedirs(pasta, exist_ok=True)
    return os.path.join(pasta, f"{_chave(video)}_voz.wav")


def separar(video: str) -> tuple[str | None, dict]:
    """(wav 16 kHz mono só com a voz, info). None se não deu: a info diz por quê."""
    alvo = cache_voz(video)
    c = cfg()
    if os.path.exists(alvo):
        return alvo, {"modelo": "cache"}
    if not disponivel():
        return None, {"erro": f"sem o ambiente de separação em {caminhos.venv('sep')}"}
    tmp = tempfile.mkdtemp(prefix="ng3_sep_")
    try:
        entrada = os.path.join(tmp, "entrada.wav")
        # o separador trabalha em 44,1 kHz estéreo (é o que os modelos esperam)
        r = subprocess.run(["ffmpeg", "-y", "-nostdin", "-hide_banner", "-loglevel", "error", "-i", video,
                            "-vn", "-ac", "2", "-ar", "44100", entrada], capture_output=True)
        if r.returncode != 0:
            return None, {"erro": "ffmpeg não extraiu o áudio"}
        ultimo = ""
        for modelo in [m for m in (c.get("modelo"), c.get("modelo_leve")) if m]:
            r = subprocess.run([caminhos.python_de("sep"), os.path.abspath(__file__), "rodar", entrada, tmp, modelo],
                               capture_output=True, text=True, timeout=1800)
            saida = (r.stdout.strip().splitlines() or [""])[-1]
            try:
                arqs = json.loads(saida).get("arquivos") or []
            except ValueError:
                arqs = []
            voz = next((os.path.join(tmp, a) if not os.path.isabs(a) else a for a in arqs), None)
            if r.returncode == 0 and voz and os.path.exists(voz):
                parcial = alvo + ".tmp.wav"
                r2 = subprocess.run(["ffmpeg", "-y", "-nostdin", "-hide_banner", "-loglevel", "error", "-i", voz,
                                     "-ac", "1", "-ar", str(SR), "-c:a", "pcm_s16le", parcial], capture_output=True)
                if r2.returncode == 0:
                    os.replace(parcial, alvo)
                    return alvo, {"modelo": modelo}
            ultimo = (r.stderr or saida)[-300:]
        return None, {"erro": f"separação falhou: {ultimo.strip()}"}
    except subprocess.TimeoutExpired:
        return None, {"erro": "separação passou de 30 min"}
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def preparar(video: str, wav: str, modo: str = "auto", regioes_voz=None) -> tuple[str, dict]:
    """O áudio que vai para o Whisper e o registro do que foi feito.

    modo: "auto" (separa se `tem_musica`), "sempre" ou "nunca"."""
    modo = modo if modo in ("auto", "sempre", "nunca") else str(cfg().get("modo") or "auto")
    info: dict = {"fonte": "original", "modo": modo}
    if modo == "nunca":
        return wav, info
    if modo == "auto":
        try:
            m = tem_musica(wav, regioes_voz)
        except Exception as e:              # sem numpy, áudio estranho: segue com o original
            info["deteccao"] = f"falhou: {str(e)[:80]}"
            return wav, info
        info["musica"] = m["fracao"]
        if not m["musica"]:
            return wav, info
    voz, r = separar(video)
    info.update(r)
    if voz:
        info["fonte"] = "separada"
        return voz, info
    return wav, info


# ------------------------------------------------------------ dentro do ambiente sep
def rodar(entrada: str, pasta: str, modelo: str) -> None:
    """Roda no python do ambiente `sep`: separa só a voz e imprime {"arquivos": [...]}."""
    from audio_separator.separator import Separator
    s = Separator(output_dir=pasta, output_format="WAV", output_single_stem="Vocals")
    s.load_model(model_filename=modelo)
    arqs = s.separate(entrada)
    print(json.dumps({"arquivos": list(arqs)}))


def main() -> None:
    if len(sys.argv) >= 5 and sys.argv[1] == "rodar":
        rodar(sys.argv[2], sys.argv[3], sys.argv[4])
    elif len(sys.argv) >= 3 and sys.argv[1] == "medir":
        from transcricao import voz
        v = voz.analisar(sys.argv[2])            # Silero (no ambiente asr) ou só energia
        r = tem_musica(sys.argv[2], v.regioes)
        print(json.dumps({**r, "vad": v.metodo, "voz_s": round(sum(b - a for a, b in v.regioes), 1),
                          "duracao": round(v.duracao, 1)}, ensure_ascii=False))
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
