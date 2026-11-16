"""Projeto de uma pasta (projetos/pasta.py e modelo.adicionar_broll): `python3 -m unittest discover -s testes`."""
import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.join(_os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))), "src"))  # src/

import os
import shutil
import tempfile
import unittest
from projetos import modelo as MF  # noqa: E402
from projetos import pasta as P  # noqa: E402
from projetos import gerar  # noqa: E402
from painel import api  # noqa: E402

FOTO = {"name": "f.jpg", "kind": "Still", "duration": MF.ticks(5), "video": {"width": 1440, "height": 1440},
        "audio_streams": [], "container": "Image", "start_timecode": None, "file_size": 1}


def projeto(dur_video: float = 20.0, dur_ending: float = 5.0) -> dict:
    """V1/A1 com o vídeo e o encerramento (outra mídia), como o construir deixa."""
    motion = {"effect": "motion", "enabled": True, "params": {
        "anchor": {"value": {"Vec2": {"x": 0, "y": 0}}}, "position": {"value": {"Vec2": {"x": 0, "y": 0}}},
        "scale": {"value": {"Float": 100.0}}, "uniform_scale": {"value": {"Bool": True}}}}
    v, e = MF.ticks(dur_video), MF.ticks(dur_ending)
    media = lambda p: {"kind": {"Media": {"media": {"File": {"path": p}}, "info": {}}}, "name": p}  # noqa: E731
    return {"project": {"name": "t", "next_id": 60, "root": {"children": []}, "items": {
        "8": {"id": 8, **media("/v.mp4")}, "26": {"id": 26, **media("/fim.mp4")},
        "50": {"name": "seq", "kind": {"Sequence": {
            "settings": {"width": 1080, "height": 1920},
            "video_tracks": [{"id": 1, "name": "Video 1", "items": [
                {"id": 10, "item": 8, "start": 0, "duration": v, "effects": [motion]},
                {"id": 28, "item": 26, "start": v, "duration": e, "effects": [motion]}]},
                {"id": 2, "name": "Video 2", "items": []}],
            "audio_tracks": [{"id": 4, "items": [{"id": 11, "item": 8, "start": 0, "duration": v},
                                                 {"id": 29, "item": 26, "start": v, "duration": e}]}],
            "caption_tracks": []}}}}}}


class Pasta(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def criar(self, *nomes):
        for n in nomes:
            c = os.path.join(self.tmp, n)
            os.makedirs(os.path.dirname(c), exist_ok=True)
            open(c, "w").close()

    def test_listar_ordena_e_pula_saidas(self):
        self.criar("02.mp4", "01.mp4", "03.MOV", "a.jpg", "saidas/01_sem_silencio.mp4", "silencecut/x.mp4",
                   "videos/04.mp4", "notas.txt")
        v, i = P.listar(self.tmp)
        self.assertEqual([os.path.relpath(x, self.tmp) for x in v], ["01.mp4", "02.mp4", "03.MOV", "videos/04.mp4"])
        self.assertEqual([os.path.basename(x) for x in i], ["a.jpg"])
        v, _ = P.listar(self.tmp, ["03.MOV", "02.mp4"])
        self.assertEqual([os.path.basename(x) for x in v][:3], ["03.MOV", "02.mp4", "01.mp4"])

    def test_torcida_nao_e_fala(self):
        self.assertFalse(P.tem_fala([(1, 2, "Vamos"), (2, 3, "caralho!"), (40, 41, "Aplausos")], 47.0))
        fala = [(i * 0.4, i * 0.4 + 0.3, "p") for i in range(120)]
        self.assertTrue(P.tem_fala(fala, 60.0))

    def test_regioes_juntam_pausas_curtas(self):
        self.assertEqual(P.regioes([(0, 1, "a"), (1.2, 2, "b"), (5, 6, "c")], 0.8), [[0, 2], [5, 6]])

    def test_fotos_entram_na_palavra_gatilho(self):
        ents = [{"video": "a", "duracao": 30.0, "trechos": None, "palavras": []},
                {"video": "b", "duracao": 60.0, "trechos": [(10.0, 50.0, 10.0, 50.0)],
                 "palavras": [(12.0, 12.4, "a"), (20.0, 20.5, "Fotos,"), (48.0, 48.5, "foto")]}]
        # 30 s do 1º vídeo + (20 - 10) dentro do trecho
        self.assertAlmostEqual(P.inicio_das_fotos(ents, ["foto"], 9.0), 40.0)
        # sem espaço depois do gatilho (ou sem gatilho): começo do vídeo mais falado
        self.assertAlmostEqual(P.inicio_das_fotos(ents, ["nada"], 9.0), 31.0)

    def test_trecho_com_jl_sobrevive(self):
        self.assertEqual(gerar._no_video((1.0, 99.0, 1.3, 98.0), 50.0), (1.0, 50.0, 1.3, 50.0))
        self.assertEqual(gerar._no_video((-1.0, 5.0), 50.0), (0.0, 5.0))

    def test_broll_acima_do_v1_e_antes_do_encerramento(self):
        p = projeto(20.0, 5.0)
        n = MF.adicionar_broll(p, [(f"/f{i}.jpg", dict(FOTO)) for i in range(4)], inicio=15.0, duracao=3.0)
        self.assertEqual(n, 4)
        k = MF._seq(p)
        self.assertEqual([t["name"] for t in k["video_tracks"]], ["Video 1", "Fotos", "Video 2"])
        fotos = k["video_tracks"][1]["items"]
        # 12 s de fotos não cabem a partir de 15 s num vídeo de 20 s: a faixa recua para 8 s
        self.assertEqual(fotos[0]["start"], MF.ticks(8.0))
        self.assertEqual(fotos[-1]["start"] + fotos[-1]["duration"], MF.ticks(20.0))
        self.assertEqual(fotos[0]["effects"][0]["params"]["scale"]["value"]["Float"], round(1920 / 1440 * 100, 2))
        ids = [c["id"] for t in k["video_tracks"] + k["audio_tracks"] for c in t["items"]]
        self.assertEqual(len(ids), len(set(ids)))
        self.assertTrue(all(str(c["item"]) in p["project"]["items"] for c in fotos))

    def test_api_recusa_pasta_fora_das_raizes(self):
        r = api.atender({"id": 1, "metodo": "projeto_pasta", "dados": {"pasta": "/etc"}})
        self.assertFalse(r["ok"])


if __name__ == "__main__":
    unittest.main()
