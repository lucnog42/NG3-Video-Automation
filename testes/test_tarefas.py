"""Tarefas destacadas (sobrevivem ao painel): `python3 -m unittest discover -s testes`.
Roda processos de verdade, curtos, do próprio Python (sem GPU, sem rede)."""
import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.join(_os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))), "src"))  # src/

import os
import shutil
import sys
import tempfile
import time
import unittest
from unittest import mock

from painel import tarefas  # noqa: E402


class Tarefas(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.p = mock.patch.object(tarefas, "pasta", return_value=self.tmp)
        self.p.start()

    def tearDown(self):
        self.p.stop()
        shutil.rmtree(self.tmp, ignore_errors=True)

    def esperar(self, tid, ate=15.0):
        fim = time.time() + ate
        while time.time() < fim and tarefas.estado(tarefas.ler(tid)) == "rodando":
            time.sleep(0.05)

    def test_roda_grava_log_e_codigo(self):
        r = tarefas.iniciar([sys.executable, "-c", "print('um'); print('dois')"], "teste", self.tmp)
        self.esperar(r["id"])
        self.assertEqual(tarefas.estado(tarefas.ler(r["id"])), "ok")
        eventos = []
        tarefas.acompanhar(r["id"], eventos.append, passo=0.01)
        linhas = [e["linha"] for e in eventos if "linha" in e]
        self.assertEqual(linhas, ["um", "dois"])
        self.assertTrue(eventos[-1]["fim"] and eventos[-1]["ok"])
        self.assertTrue(tarefas.ler(r["id"])["visto_fim"])

    def test_roda_noutra_sessao(self):
        r = tarefas.iniciar([sys.executable, "-c", "import time; time.sleep(5)"], "teste", self.tmp)
        try:
            self.assertNotEqual(os.getpgid(r["grupo"]), os.getpgid(0))   # fechar o painel não a leva junto
            self.assertEqual(tarefas.estado(tarefas.ler(r["id"])), "rodando")
        finally:
            tarefas.cancelar(r["id"])
            self.esperar(r["id"])

    def test_cancelar_e_falha(self):
        r = tarefas.iniciar([sys.executable, "-c", "import time; time.sleep(30)"], "teste", self.tmp)
        time.sleep(0.3)
        self.assertTrue(tarefas.cancelar(r["id"]))
        self.esperar(r["id"])
        self.assertIn(tarefas.estado(tarefas.ler(r["id"])), ("falhou", "interrompida"))
        self.assertFalse(tarefas.cancelar(r["id"]))
        r2 = tarefas.iniciar([sys.executable, "-c", "raise SystemExit(3)"], "teste", self.tmp)
        self.esperar(r2["id"])
        self.assertEqual(tarefas.ler(r2["id"])["codigo"], 3)

    def test_retomar_mostra_o_fim_uma_vez(self):
        r = tarefas.iniciar([sys.executable, "-c", "print('feito')"], "transcrever", self.tmp)
        self.esperar(r["id"])
        from painel import api
        emitidos = []
        with mock.patch.object(api, "emitir", emitidos.append):
            self.assertEqual(api.m_tarefas_retomar({})["retomadas"], 1)
            fim = time.time() + 5
            while time.time() < fim and not any(e.get("fim") for e in emitidos):
                time.sleep(0.05)
            self.assertTrue(any(e.get("inicio") and e.get("retomada") for e in emitidos))
            self.assertIn("feito", [e.get("linha") for e in emitidos])
            time.sleep(0.1)
            self.assertEqual(api.m_tarefas_retomar({})["retomadas"], 0)     # já visto

    def test_vigia_morto_vira_interrompida(self):
        tid = "20260101000000-abc123"
        tarefas._gravar(tid, {"id": tid, "dono": "x", "cmd": ["true"], "inicio": time.time(), "grupo": 999999999})
        self.assertEqual(tarefas.estado(tarefas.ler(tid)), "interrompida")


if __name__ == "__main__":
    unittest.main()
