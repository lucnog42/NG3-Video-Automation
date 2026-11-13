"""Biblioteca de mídia (catálogo, fila de revisão, tags): `python3 -m unittest discover -s testes`."""
import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.join(_os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))), "src"))  # src/

import os
import shutil
import tempfile
import unittest
from acervo import biblioteca  # noqa: E402

CATEGORIAS = {"lugar": {"nome": "Lugar", "multipla": True, "tags": {"praia": "Praia", "estadio": "Estádio"}},
              "momento": {"nome": "Momento", "multipla": True, "tags": {"treino": "Treino"}}}


class Biblioteca(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.pasta = os.path.join(self.tmp, "Viagem")
        for rel, conteudo in [("Treino/a.mp4", b"video-a" * 50), ("Treino/b.jpg", b"foto-b"),
                              ("Jogos/Final/c.mov", b"video-c" * 70), ("solto.mp4", b"video-d" * 30),
                              ("nota.txt", b"nada")]:
            arq = os.path.join(self.pasta, rel)
            os.makedirs(os.path.dirname(arq), exist_ok=True)
            with open(arq, "wb") as fh:
                fh.write(conteudo)
        self.db = os.path.join(self.tmp, "midia.sqlite")
        self.fontes = [{"id": "viagem", "nome": "Viagem", "tipo": "broll", "raiz": self.pasta, "ativo": True}]
        self.cats_orig = biblioteca.categorias_marca
        biblioteca.categorias_marca = lambda: CATEGORIAS

    def tearDown(self):
        biblioteca.categorias_marca = self.cats_orig
        shutil.rmtree(self.tmp, ignore_errors=True)

    def sinc(self):
        return biblioteca.sincronizar(db=self.db, fontes_cfg=self.fontes)["viagem"]

    def lista(self, **f):
        return biblioteca.listar(f, db=self.db)

    def test_import_cai_na_fila_com_tags_de_pasta(self):
        self.assertEqual(self.sinc()["novos"], 4)               # o .txt fica de fora
        r = self.lista(passo="fila")
        self.assertEqual(r["total"], 4)
        self.assertTrue(all(i["status"] == "novo" for i in r["itens"]))
        c = next(i for i in r["itens"] if i["nome"] == "c.mov")
        self.assertEqual({t["slug"] for t in c["tags"]}, {"jogos", "final"})
        self.assertTrue(all(t["origem"] == "pasta" and t["por"] == "pasta" for t in c["tags"]))
        self.assertEqual(self.lista(tipo="imagem")["total"], 1)
        self.assertEqual(self.lista(subpasta="Jogos")["total"], 1)

    def test_sincronizar_de_novo_nao_duplica(self):
        self.sinc()
        self.assertEqual(self.sinc()["novos"], 0)
        self.assertEqual(self.lista()["total"], 4)

    def test_arquivo_movido_mantem_id_e_tags(self):
        self.sinc()
        a = next(i for i in self.lista()["itens"] if i["nome"] == "a.mp4")
        biblioteca.aplicar_tags([a["id"]], ["praia"], db=self.db)
        os.makedirs(os.path.join(self.pasta, "Outra"))
        os.rename(os.path.join(self.pasta, "Treino/a.mp4"), os.path.join(self.pasta, "Outra/a2.mp4"))
        r = self.sinc()
        self.assertEqual((r["novos"], r["movidos"], r["ausentes"]), (0, 1, 0))
        m = next(i for i in self.lista()["itens"] if i["id"] == a["id"])
        self.assertEqual(m["nome"], "a2.mp4")
        self.assertIn("praia", {t["slug"] for t in m["tags"]})

    def test_arquivo_sumido_fica_ausente_e_volta(self):
        self.sinc()
        destino = os.path.join(self.tmp, "fora.mp4")
        os.rename(os.path.join(self.pasta, "solto.mp4"), destino)
        self.assertEqual(self.sinc()["ausentes"], 1)
        self.assertEqual(self.lista()["total"], 3)
        os.rename(destino, os.path.join(self.pasta, "solto.mp4"))
        self.sinc()
        self.assertEqual(self.lista()["total"], 4)

    def test_nada_e_escrito_na_pasta_importada(self):
        antes = sorted(os.path.join(r, a) for r, _, arqs in os.walk(self.pasta) for a in arqs)
        self.sinc()
        biblioteca.aplicar_tags([i["id"] for i in self.lista()["itens"]], ["treino"], db=self.db)
        depois = sorted(os.path.join(r, a) for r, _, arqs in os.walk(self.pasta) for a in arqs)
        self.assertEqual(antes, depois)

    def test_tag_da_marca_e_livre_em_lote(self):
        self.sinc()
        ids = [i["id"] for i in self.lista()["itens"]]
        biblioteca.aplicar_tags(ids[:3], ["Estádio", "chegada no hotel"], db=self.db)
        t = biblioteca.tags(db=self.db)
        lugar = next(c for c in t["categorias"] if c["slug"] == "lugar")
        self.assertEqual(next(x for x in lugar["tags"] if x["slug"] == "estadio")["n"], 3)
        livre = next(x for x in t["livres"] if x["slug"] == "chegada-no-hotel")
        self.assertEqual((livre["n"], livre["origem"], livre["nome"]), (3, "livre", "chegada no hotel"))
        self.assertIn("jogos", {x["slug"] for x in t["pasta"]})
        self.assertEqual(self.lista(tags=["estadio"])["total"], 3)
        self.assertEqual(self.lista(passo="com_tags")["total"], 3)
        self.assertEqual(self.lista(passo="sem_tags")["total"], 1)
        biblioteca.aplicar_tags(ids[:1], ["estadio"], remover=True, db=self.db)
        self.assertEqual(self.lista(tags=["estadio"])["total"], 2)

    def test_tag_de_pasta_com_slug_da_marca_vira_da_marca(self):
        os.makedirs(os.path.join(self.pasta, "Treino", "x"), exist_ok=True)
        self.sinc()
        t = biblioteca.tags(db=self.db)
        mom = next(c for c in t["categorias"] if c["slug"] == "momento")
        self.assertEqual(next(x for x in mom["tags"] if x["slug"] == "treino")["n"], 2)

    def test_revisar_libera_e_devolve(self):
        self.sinc()
        ids = [i["id"] for i in self.lista()["itens"]][:2]
        biblioteca.revisar(ids, db=self.db)
        self.assertEqual(self.lista(passo="revisado")["total"], 2)
        self.assertEqual(self.lista(passo="fila")["total"], 2)
        biblioteca.revisar(ids[:1], revisado=False, db=self.db)
        self.assertEqual(self.lista(passo="fila")["total"], 3)

    def test_busca_e_resumo(self):
        self.sinc()
        self.assertEqual(self.lista(busca="final")["total"], 1)
        r = biblioteca.resumo({"tipo": "video"}, db=self.db)
        self.assertEqual(r["passos"]["todos"], 3)
        self.assertEqual(r["tipos"], {"video": 3, "imagem": 1})       # o grupo do tipo ignora o próprio filtro
        self.assertEqual({s["nome"]: s["n"] for s in r["subpastas"]}, {"Treino": 1, "Jogos": 1})
        self.assertEqual(len(r["importacoes"]), 1)
        self.assertEqual(len(biblioteca.ids({"tipo": "video"}, db=self.db)), 3)


if __name__ == "__main__":
    unittest.main()
