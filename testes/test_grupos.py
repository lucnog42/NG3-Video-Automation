"""Grupos que viram projeto e o roteiro só com o conteúdo do grupo (IA simulada)."""
import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.join(_os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))), "src"))  # src/

import json
import os
import shutil
import tempfile
import unittest
from unittest import mock
from acervo import biblioteca, grupos  # noqa: E402
from achar import indice  # noqa: E402
from nucleo import ia  # noqa: E402
from plano import momentos, roteiros  # noqa: E402

PALAVRAS = {
    "a.mp4": [(0.0, 0.4, "Eu"), (0.45, 0.9, "cheguei"), (0.95, 1.6, "em"), (1.65, 2.4, "Barcelona.")],
    "c.mov": [(0.0, 0.5, "O"), (0.55, 1.0, "treino"), (1.05, 1.8, "foi"), (1.85, 2.6, "incrível.")],
}


class Grupos(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.pasta = os.path.join(self.tmp, "Viagem")
        for rel in ("Dia1/a.mp4", "Dia1/b.jpg", "Dia2/c.mov", "Outra/d.mp4"):
            arq = os.path.join(self.pasta, rel)
            os.makedirs(os.path.dirname(arq), exist_ok=True)
            with open(arq, "wb") as fh:
                fh.write(rel.encode() * 20)
        self.db = os.path.join(self.tmp, "midia.sqlite")
        fontes = [{"id": "viagem", "nome": "Viagem", "tipo": "broll", "raiz": self.pasta, "ativo": True}]
        self.p = [mock.patch.object(biblioteca, "caminho_db", lambda: self.db),
                  mock.patch.object(biblioteca, "categorias_marca", lambda: {})]
        for p in self.p:
            p.start()
        biblioteca.sincronizar(db=self.db, fontes_cfg=fontes)
        con = biblioteca.abrir(self.db)
        self.ids = {os.path.basename(r["caminho"]): r["id"] for r in con.execute("select id, caminho from midia")}
        self.sub = {os.path.basename(r["caminho"]): r["subpasta"] for r in con.execute("select caminho, subpasta from midia")}
        con.close()

    def tearDown(self):
        for p in self.p:
            p.stop()
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_criar_de_selecao_misturando_pastas(self):
        g = grupos.criar("Chegada", [self.ids["a.mp4"], self.ids["c.mov"]])
        self.assertEqual((g["nome"], g["videos"], g["imagens"]), ("Chegada", 2, 0))
        self.assertEqual(len(g["pastas"]), 2)
        self.assertEqual([x["id"] for x in grupos.listar()], [g["id"]])

    def test_criar_de_pasta_e_mesma_midia_em_dois_grupos(self):
        g1 = grupos.criar("", subpasta=self.sub["a.mp4"])
        self.assertEqual(g1["n"], 2)
        self.assertEqual(g1["nome"], os.path.basename(self.sub["a.mp4"]))
        g2 = grupos.criar("Outro", [self.ids["a.mp4"]])
        self.assertIn(self.ids["a.mp4"], grupos.ler(g1["id"])["ids"])
        self.assertIn(self.ids["a.mp4"], grupos.ler(g2["id"])["ids"])
        lista = biblioteca.listar({"grupo": g1["id"]}, db=self.db)
        self.assertEqual(lista["total"], 2)

    def test_editar_e_apagar_nao_toca_arquivos(self):
        g = grupos.criar("G", [self.ids["a.mp4"]])
        grupos.adicionar(g["id"], [self.ids["d.mp4"]])
        grupos.tirar(g["id"], [self.ids["a.mp4"]])
        self.assertEqual(grupos.ler(g["id"])["ids"], [self.ids["d.mp4"]])
        grupos.renomear(g["id"], "Novo nome", "nota")
        self.assertEqual(grupos.ler(g["id"])["nome"], "Novo nome")
        grupos.apagar(g["id"])
        self.assertEqual(grupos.listar(), [])
        self.assertTrue(os.path.exists(os.path.join(self.pasta, "Dia1", "a.mp4")))
        with self.assertRaises(ValueError):
            grupos.criar("vazio", [])

    def test_roteiro_do_grupo_usa_so_o_grupo(self):
        g = grupos.criar("Chegada", [self.ids["a.mp4"], self.ids["b.jpg"]])
        pedidos = []

        def conversar(conteudo, sistema, maximo=1500):
            texto = conteudo[0]["text"]
            pedidos.append(texto)
            ids_ = [ln.split("]")[0][1:] for ln in texto.splitlines() if ln.startswith("[")]
            if "Proponha" in texto:
                return json.dumps({"ideias": [{"titulo": "Chegada", "formato": "x", "falas": ids_[:1]}]})
            return json.dumps({"titulo": "Chegada", "partes": [
                {"parte": "gancho", "tipo": "fala", "id": ids_[0], "funcao": "provar", "mostrar": "foto chegada"},
                {"parte": "fim", "tipo": "nova", "texto": "E foi só o começo", "funcao": "so-a-pessoa"}]})
        tmp_rot = os.path.join(self.tmp, "roteiros")
        os.makedirs(tmp_rot)
        with mock.patch.object(ia, "conversar", conversar), \
                mock.patch.object(momentos, "_palavras", lambda v: PALAVRAS.get(os.path.basename(v), [])), \
                mock.patch.object(roteiros, "pasta", lambda: tmp_rot), \
                mock.patch.object(indice, "buscar", lambda termos, limite=40: []), \
                mock.patch("achar.broll._miniatura", lambda c: ""):
            ids_ = roteiros.ideias("chegada", 1, grupo=g["id"])
            self.assertEqual(ids_[0]["grupo"], g["id"])
            self.assertEqual(roteiros.ler_ideias(), [])          # as ideias do grupo ficam à parte
            rot = roteiros.escrever("chegada", ideia=ids_[0]["id"], grupo=g["id"])
        self.assertIn("cheguei em Barcelona", pedidos[-1])
        self.assertNotIn("treino", pedidos[-1])                   # c.mov está fora do grupo
        self.assertEqual(rot["grupo"], g["id"])
        fala = rot["partes"][0]
        self.assertEqual(os.path.basename(fala["video"]), "a.mp4")
        self.assertEqual([os.path.basename(c["arquivo"]) for c in fala["cobertura"]], ["b.jpg"])
        with mock.patch.object(roteiros, "pasta", lambda: tmp_rot):
            self.assertFalse(roteiros.listar()[0]["grupo_mudou"])
            grupos.adicionar(g["id"], [self.ids["c.mov"]])
            self.assertTrue(roteiros.listar()[0]["grupo_mudou"])

    def test_api(self):
        from painel import api
        with mock.patch.object(api, "emitir", lambda *_: None):
            r = api.atender({"id": 1, "metodo": "grupo_criar",
                             "dados": {"nome": "API", "ids": [self.ids["a.mp4"], self.ids["d.mp4"]]}})
            self.assertTrue(r["ok"], r)
            gid = r["resultado"]["id"]
            r = api.atender({"id": 2, "metodo": "grupos", "dados": {}})
            self.assertEqual([g["nome"] for g in r["resultado"]["grupos"]], ["API"])
            r = api.atender({"id": 3, "metodo": "grupo_editar", "dados": {"id": gid, "acao": "sumir"}})
            self.assertFalse(r["ok"])
            r = api.atender({"id": 4, "metodo": "tags_importar_previa",
                             "dados": {"ids": [self.ids["b.jpg"]], "lrcat": "/nao/existe.lrcat"}})
            self.assertFalse(r["ok"])
            r = api.atender({"id": 5, "metodo": "tags_importar_previa", "dados": {"ids": [self.ids["b.jpg"]]}})
            self.assertTrue(r["ok"], r)
            self.assertEqual(r["resultado"]["com_dados"], 0)

    def test_grupo_sem_fala_avisa(self):
        g = grupos.criar("Fotos", [self.ids["b.jpg"]])
        with mock.patch.object(momentos, "_palavras", lambda v: []):
            with self.assertRaises(RuntimeError):
                roteiros.ideias("", 1, grupo=g["id"])


if __name__ == "__main__":
    unittest.main()
