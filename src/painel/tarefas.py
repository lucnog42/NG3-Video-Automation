#!/usr/bin/env python3
"""Tarefas que sobrevivem ao painel: transcrever, detectar cenas, plugins.

Antes, a saída da tarefa ia por um pipe para o motor (`api.py`); ao fechar o painel o motor
morria e a tarefa caía no próximo `print` (pipe quebrado), no meio de um vídeo. Agora:

  * cada tarefa roda num **processo próprio, numa sessão própria** (`start_new_session`): fechar
    o painel não a derruba;
  * um pequeno vigia (`tarefas.py rodar <id>`) roda o comando, grava a saída em
    `dados/marcas/<marca>/tarefas/<id>.log` e, no fim, o código de saída em `<id>.json`
    (escrita atômica);
  * o motor só **lê** o log e repassa as linhas ao app; ao abrir o painel de novo, as tarefas
    que ainda rodam voltam a aparecer, e as que terminaram com o painel fechado mostram o
    resultado uma vez;
  * cancelar mata o grupo de processos inteiro (o vigia e o comando).
"""
from __future__ import annotations

import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))))  # src/

import json
import os
import secrets
import signal
import subprocess
import sys
import time

from nucleo import caminhos  # noqa: E402

GUARDAR_DIAS = 7


def pasta() -> str:
    # o vigia recebe a pasta de quem o soltou (mesma marca, mesmo lugar)
    p = os.environ.get("NG3_TAREFAS") or caminhos.dados("tarefas")
    os.makedirs(p, exist_ok=True)
    return p


def _arq(tid: str, ext: str) -> str:
    if not tid or "/" in tid or tid.startswith("."):
        raise ValueError("tarefa inválida")
    return os.path.join(pasta(), f"{tid}.{ext}")


def ler(tid: str) -> dict:
    try:
        with open(_arq(tid, "json"), encoding="utf-8") as fh:
            return json.load(fh)
    except (OSError, ValueError):
        return {}


def _gravar(tid: str, d: dict) -> None:
    alvo = _arq(tid, "json")
    with open(alvo + ".tmp", "w", encoding="utf-8") as fh:
        json.dump(d, fh, ensure_ascii=False)
    os.replace(alvo + ".tmp", alvo)


def _vivo(pid) -> bool:
    if not pid:
        return False
    try:
        os.kill(int(pid), 0)
    except (OSError, ValueError):
        return False
    # processo que já terminou mas ninguém esperou (zumbi) não conta como vivo
    try:
        with open(f"/proc/{int(pid)}/stat", encoding="utf-8") as fh:
            return fh.read().split(")")[-1].split()[0] != "Z"
    except OSError:
        return True


def iniciar(cmd: list[str], dono: str, cwd: str, env: dict | None = None) -> dict:
    """Grava a tarefa e solta o vigia numa sessão própria. Devolve o registro."""
    tid = f"{time.strftime('%Y%m%d%H%M%S')}-{secrets.token_hex(3)}"
    reg = {"id": tid, "dono": dono, "cmd": cmd, "cwd": cwd, "inicio": time.time(), "visto_fim": False}
    _gravar(tid, reg)
    amb = {**os.environ, **(env or {}), "PYTHONUNBUFFERED": "1", "NG3_TAREFAS": pasta()}
    vigia = subprocess.Popen([sys.executable, os.path.abspath(__file__), "rodar", tid], cwd=cwd, env=amb,
                             stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                             start_new_session=True)
    reg["grupo"] = vigia.pid
    _gravar(tid, {**ler(tid), "grupo": vigia.pid})
    # quem soltou espera o vigia em segundo plano (senão ele vira zumbi enquanto o motor vive)
    import threading
    threading.Thread(target=vigia.wait, daemon=True).start()
    return reg


def _rodar(tid: str) -> None:
    """O vigia: roda o comando com a saída no log e grava o código de saída no fim."""
    reg = ler(tid)
    with open(_arq(tid, "log"), "a", encoding="utf-8") as log:
        try:
            p = subprocess.Popen(reg["cmd"], cwd=reg.get("cwd") or None, stdin=subprocess.DEVNULL,
                                 stdout=log, stderr=subprocess.STDOUT)
        except OSError as e:
            log.write(f"não deu para rodar {reg['cmd'][0]}: {e}\n")
            _gravar(tid, {**ler(tid), "fim": time.time(), "codigo": -1})
            return
        _gravar(tid, {**ler(tid), "pid": p.pid})

        def parar(*_):
            p.terminate()
        signal.signal(signal.SIGTERM, parar)
        codigo = p.wait()
    _gravar(tid, {**ler(tid), "fim": time.time(), "codigo": codigo})


def estado(reg: dict) -> str:
    """rodando | ok | falhou | interrompida (o vigia morreu sem gravar o fim)."""
    if "codigo" in reg:
        return "ok" if reg["codigo"] == 0 else "falhou"
    return "rodando" if _vivo(reg.get("grupo")) else "interrompida"


def cancelar(tid: str) -> bool:
    reg = ler(tid)
    g = reg.get("grupo")
    if not g or estado(reg) != "rodando":
        return False
    try:
        os.killpg(int(g), signal.SIGTERM)
    except OSError:
        return False
    return True


def listar() -> list[dict]:
    """As tarefas guardadas, da mais nova para a mais velha (apaga as de mais de 7 dias)."""
    out = []
    limite = time.time() - GUARDAR_DIAS * 86400
    for nome in os.listdir(pasta()):
        if not nome.endswith(".json"):
            continue
        tid = nome[:-5]
        reg = ler(tid)
        if not reg:
            continue
        if reg.get("inicio", 0) < limite and estado(reg) != "rodando":
            for ext in ("json", "log"):
                try:
                    os.unlink(_arq(tid, ext))
                except OSError:
                    pass
            continue
        out.append({**reg, "estado": estado(reg)})
    return sorted(out, key=lambda r: -r.get("inicio", 0))


def acompanhar(tid: str, emitir, ultimas: int | None = None, passo: float = 0.3) -> None:
    """Repassa as linhas do log (as `ultimas`, ao retomar) até a tarefa acabar; então o fim."""
    log = _arq(tid, "log")
    while not os.path.exists(log) and estado(ler(tid)) == "rodando":
        time.sleep(passo)
    pos = 0
    if ultimas is not None and os.path.exists(log):
        with open(log, encoding="utf-8", errors="replace") as fh:
            linhas = fh.readlines()
        for l in linhas[-ultimas:]:
            emitir({"evento": "tarefa", "tarefa": tid, "linha": l.rstrip("\n")})
        pos = os.path.getsize(log)
    resto = ""
    while True:
        reg = ler(tid)
        st = estado(reg)
        if os.path.exists(log):
            with open(log, encoding="utf-8", errors="replace") as fh:
                fh.seek(pos)
                novo = fh.read()
                pos = fh.tell()
            resto += novo
            *prontas, resto = resto.split("\n")
            for l in prontas:
                emitir({"evento": "tarefa", "tarefa": tid, "linha": l})
        if st != "rodando":
            if resto:
                emitir({"evento": "tarefa", "tarefa": tid, "linha": resto})
            if st == "interrompida":
                emitir({"evento": "tarefa", "tarefa": tid, "linha": "(a tarefa parou sem terminar: o computador "
                                                                     "desligou ou o processo foi morto)"})
            emitir({"evento": "tarefa", "tarefa": tid, "fim": True, "ok": st == "ok",
                    "codigo": reg.get("codigo", -1)})
            _gravar(tid, {**ler(tid), "visto_fim": True})
            return
        time.sleep(passo)


def main() -> None:
    if len(sys.argv) >= 3 and sys.argv[1] == "rodar":
        _rodar(sys.argv[2])
    else:
        for r in listar():
            print(f"{r['id']}  {r['estado']:<11} {r['dono']:<12} {' '.join(r['cmd'])[:80]}")


if __name__ == "__main__":
    main()
