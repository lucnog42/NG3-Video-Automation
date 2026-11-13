"""Separação de voz e música, e a tarefa de transcrever: `python3 -m unittest discover -s testes`.

A detecção (`tem_musica`) precisa de numpy; sem ele, esses testes são pulados. Nada de modelo
real, GPU ou rede: o separador é simulado."""
import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.join(_os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))), "src"))  # src/

import os
import shutil
import tempfile
import unittest
from unittest import mock

from transcricao import separar  # noqa: E402

try:
    import numpy as np
except ImportError:                       # pragma: no cover
    np = None

SR = 16000


def _seg(dur):
    return np.arange(int(SR * dur)) / SR


@unittest.skipUnless(np is not None, "precisa de numpy")
class TemMusica(unittest.TestCase):
    def setUp(self):
        self.rng = np.random.default_rng(7)

    def musica(self, dur=6.0):
        t = _seg(dur)
        acorde = sum(np.sin(2 * np.pi * f * t) for f in (220.0, 277.2, 329.6)) / 3
        return (0.3 * acorde + 0.01 * self.rng.standard_normal(len(t))).astype(np.float32)

    def test_acorde_com_pouco_ruido_e_musica(self):
        r = separar.tem_musica(self.musica(), limiar=0.25)
        self.assertTrue(r["musica"], r)
        self.assertGreater(r["fracao"], 0.8)

    def test_ruido_modulado_nao_e_musica(self):
        t = _seg(6.0)
        x = (0.3 * self.rng.standard_normal(len(t)) * (0.5 + 0.5 * np.sin(2 * np.pi * 0.7 * t))).astype(np.float32)
        r = separar.tem_musica(x, limiar=0.25)
        self.assertFalse(r["musica"], r)

    def test_silencio_nao_e_musica(self):
        r = separar.tem_musica(np.zeros(SR * 5, dtype=np.float32), limiar=0.25)
        self.assertFalse(r["musica"])

    def test_so_mede_fora_da_voz(self):
        # 0–4 s "voz" tonal (marcada como voz pelo VAD), 4–8 s ruído: fora da voz não há música
        tonal = self.musica(4.0)
        ruido = (0.3 * self.rng.standard_normal(SR * 4)).astype(np.float32)
        x = np.concatenate([tonal, ruido])
        self.assertFalse(separar.tem_musica(x, regioes_voz=[(0.0, 4.0)], limiar=0.25)["musica"])
        self.assertTrue(separar.tem_musica(x, regioes_voz=[(4.0, 8.0)], limiar=0.25)["musica"])


class Preparar(unittest.TestCase):
    def test_nunca_usa_o_original(self):
        audio, info = separar.preparar("v.mp4", "/tmp/a.wav", "nunca")
        self.assertEqual((audio, info["fonte"]), ("/tmp/a.wav", "original"))

    def test_auto_sem_musica_nao_separa(self):
        with mock.patch.object(separar, "tem_musica", return_value={"fracao": 0.05, "musica": False}), \
             mock.patch.object(separar, "separar") as sep:
            audio, info = separar.preparar("v.mp4", "/tmp/a.wav", "auto")
        sep.assert_not_called()
        self.assertEqual((audio, info["fonte"], info["musica"]), ("/tmp/a.wav", "original", 0.05))

    def test_auto_com_musica_usa_a_voz_separada(self):
        with mock.patch.object(separar, "tem_musica", return_value={"fracao": 0.6, "musica": True}), \
             mock.patch.object(separar, "separar", return_value=("/cache/x_voz.wav", {"modelo": "m"})):
            audio, info = separar.preparar("v.mp4", "/tmp/a.wav", "auto")
        self.assertEqual((audio, info["fonte"], info["modelo"]), ("/cache/x_voz.wav", "separada", "m"))

    def test_sem_ambiente_segue_com_o_original_e_diz_por_que(self):
        with mock.patch.object(separar, "disponivel", return_value=False), \
             mock.patch.object(separar, "cache_voz", return_value="/nao/existe_voz.wav"):
            audio, info = separar.preparar("v.mp4", "/tmp/a.wav", "sempre")
        self.assertEqual((audio, info["fonte"]), ("/tmp/a.wav", "original"))
        self.assertIn("sem o ambiente", info["erro"])

    def test_separar_chama_o_ambiente_e_converte_a_voz(self):
        tmp = tempfile.mkdtemp()
        try:
            alvo = os.path.join(tmp, "v_voz.wav")
            chamadas = []

            def falso(cmd, **kw):
                chamadas.append(cmd)
                r = mock.Mock(returncode=0, stderr="", stdout="")
                if "rodar" in cmd:          # o separador: grava o stem e diz o nome
                    pasta = cmd[cmd.index("rodar") + 2]
                    open(os.path.join(pasta, "entrada_(Vocals).wav"), "wb").close()
                    r.stdout = '{"arquivos": ["entrada_(Vocals).wav"]}'
                else:                       # ffmpeg: cria o arquivo de saída
                    open(cmd[-1], "wb").close()
                return r
            with mock.patch.object(separar, "disponivel", return_value=True), \
                 mock.patch.object(separar, "cache_voz", return_value=alvo), \
                 mock.patch.object(separar, "cfg", return_value={"modelo": "roformer.ckpt", "modelo_leve": "leve.onnx"}), \
                 mock.patch.object(separar.subprocess, "run", side_effect=falso):
                voz, info = separar.separar("/videos/v.mp4")
            self.assertEqual((voz, info["modelo"]), (alvo, "roformer.ckpt"))
            self.assertTrue(os.path.exists(alvo))
            self.assertEqual(sum(1 for c in chamadas if "rodar" in c), 1)     # o leve não foi preciso
        finally:
            shutil.rmtree(tmp, ignore_errors=True)


class Tarefa(unittest.TestCase):
    def chamar(self, dados):
        from painel import api
        return api.atender({"id": 1, "metodo": "midia_transcrever", "dados": dados})

    def test_sem_videos_na_selecao(self):
        from acervo import biblioteca
        with mock.patch.object(biblioteca, "videos", return_value=[]):
            r = self.chamar({"ids": [1, 2]})
        self.assertFalse(r["ok"])
        self.assertIn("imagens não são transcritas", r["erro"])

    def test_sem_ambiente_da_transcricao(self):
        from acervo import biblioteca
        from nucleo import caminhos
        with mock.patch.object(biblioteca, "videos", return_value=["a.mp4"]), \
             mock.patch.object(caminhos, "python_de", return_value="/nao/existe/python"):
            r = self.chamar({"ids": [1]})
        self.assertFalse(r["ok"])
        self.assertIn("falta o ambiente da transcrição", r["erro"])

    def test_monta_o_comando_com_a_lista(self):
        from acervo import biblioteca
        from nucleo import caminhos
        real = caminhos.python_de
        with mock.patch.object(biblioteca, "videos", return_value=["_posts/a.mp4", "/hd/b.mov"]), \
             mock.patch.object(caminhos, "python_de", side_effect=lambda n: __file__ if n == "asr" else real(n)):
            r = self.chamar({"ids": [1, 2], "forcar": True, "separar": "sempre"})
        self.assertTrue(r["ok"], r)
        cmd = r["resultado"]["cmd"]
        self.assertEqual(cmd[:2], ["python3", "src/transcricao/legendar.py"])
        self.assertEqual(cmd[cmd.index("--separar") + 1], "sempre")
        self.assertIn("--forcar", cmd)
        lista = cmd[cmd.index("--lista") + 1]
        with open(lista, encoding="utf-8") as fh:
            self.assertEqual(fh.read().split(), ["_posts/a.mp4", "/hd/b.mov"])
        os.unlink(lista)

    def test_modo_separar_da_tarefa_vence(self):
        from transcricao import legendar
        self.assertEqual(legendar.modo_separar("/x/v.mp4", "nunca"), "nunca")
        self.assertIn(legendar.modo_separar("/x/v.mp4", ""), ("auto", "sempre", "nunca"))


if __name__ == "__main__":
    unittest.main()
