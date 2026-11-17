"""Gerenciador de tags da marca: `python3 -m unittest discover -s testes`."""
import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.join(_os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))), "src"))  # src/

import os
import shutil
import tempfile
import tomllib
import unittest
from unittest import mock
from acervo import biblioteca, cortes, tags_marca  # noqa: E402

TOML = '''# comentário
[categorias.lugar]
nome = "Lugar"
multipla = true
tags = { "camp-nou" = "Camp Nou", "praia" = "Praia" }

[categorias.plano]
nome = "Plano"
multipla = false
tags = { "aberto" = "Aberto" }

[ia.lugar]
camp-nou = "o estádio Camp Nou"
'''


class TagsMarca(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.arq = os.path.join(self.tmp, "tags.toml")
        with open(self.arq, "w", encoding="utf-8") as fh:
            fh.write(TOML)
        self.db = os.path.join(self.tmp, "midia.sqlite")
        self.p = mock.patch.object(biblioteca, "categorias_marca", lambda: tags_marca.ler(self.arq)["categorias"])
        self.p.start()
        con = cortes.abrir(self.db)
        for i in (1, 2, 3):
            con.execute("insert into midia (id, fonte_id, caminho, tipo) values (?, 'f', ?, 'video')", (i, f"/x/{i}.mp4"))
        con.execute("insert into corte (id, midia_id, ini, fim, origem) values (10, 1, 0, 2, 'manual')")
        con.commit()
        con.close()
        biblioteca.aplicar_tags([1, 2], ["camp-nou"], db=self.db)
        biblioteca.aplicar_tags([3], ["Bastidor"], db=self.db)
        con = cortes.abrir(self.db)
        tid = con.execute("select id from tag where slug = 'camp-nou'").fetchone()[0]
        con.execute("insert into corte_tag values (10, ?, 'humano')", (tid,))
        con.commit()
        con.close()

    def tearDown(self):
        self.p.stop()
        shutil.rmtree(self.tmp, ignore_errors=True)

    def painel(self):
        return tags_marca.painel(self.db, self.arq)

    def cat(self, slug):
        return next(c for c in self.painel()["categorias"] if c["slug"] == slug)

    def test_contagens(self):
        lugar = self.cat("lugar")
        cn = next(t for t in lugar["tags"] if t["slug"] == "camp-nou")
        self.assertEqual((cn["midias"], cn["cortes"], cn["n"]), (2, 1, 3))
        self.assertEqual(lugar["n"], 3)
        self.assertEqual([t["slug"] for t in self.painel()["sem_categoria"]], ["bastidor"])

    def test_toml_mantem_ia_e_le_de_volta(self):
        tags_marca.criar_categoria("Momento", arq=self.arq)
        d = tags_marca.ler(self.arq)
        self.assertEqual(d["ia"]["lugar"]["camp-nou"], "o estádio Camp Nou")
        self.assertIn("momento", d["categorias"])
        self.assertTrue(os.listdir(os.path.join(self.tmp, "historico")))
        with self.assertRaises(ValueError):
            tags_marca.criar_categoria("Momento", arq=self.arq)

    def test_criar_em_lote(self):
        criadas = tags_marca.criar_tags("Momento: Treino\nlugar: Tibidabo\nfinal-copa = Final da Copa\n\n# nada",
                                        db=self.db, arq=self.arq)
        self.assertEqual([t["slug"] for t in criadas], ["treino", "tibidabo", "final-copa"])
        d = tags_marca.ler(self.arq)["categorias"]
        self.assertEqual(d["momento"]["tags"], {"treino": "Treino"})
        self.assertIn("tibidabo", d["lugar"]["tags"])
        self.assertIn("final-copa", [t["slug"] for t in self.painel()["sem_categoria"]])

    def test_renomear_mantem_slug(self):
        tags_marca.renomear_tag("camp-nou", "Spotify Camp Nou", db=self.db, arq=self.arq)
        self.assertEqual(tags_marca.ler(self.arq)["categorias"]["lugar"]["tags"]["camp-nou"], "Spotify Camp Nou")
        cn = next(t for t in self.cat("lugar")["tags"] if t["slug"] == "camp-nou")
        self.assertEqual((cn["nome"], cn["midias"]), ("Spotify Camp Nou", 2))

    def test_mover_tag_livre_para_categoria(self):
        tags_marca.criar_categoria("Momento", arq=self.arq)
        tags_marca.mover_tag("bastidor", "momento", db=self.db, arq=self.arq)
        self.assertEqual(tags_marca.ler(self.arq)["categorias"]["momento"]["tags"], {"bastidor": "Bastidor"})
        self.assertEqual(self.painel()["sem_categoria"], [])

    def test_remover_apagar_ou_fundir(self):
        tags_marca.remover_tag("camp-nou", "fundir", "praia", db=self.db, arq=self.arq)
        lugar = {t["slug"]: t for t in self.cat("lugar")["tags"]}
        self.assertNotIn("camp-nou", lugar)
        self.assertEqual((lugar["praia"]["midias"], lugar["praia"]["cortes"]), (2, 1))
        n = tags_marca.remover_tag("praia", "apagar", db=self.db, arq=self.arq)
        self.assertEqual(n, 3)
        self.assertNotIn("praia", tags_marca.ler(self.arq)["categorias"]["lugar"]["tags"])
        with self.assertRaises(ValueError):
            tags_marca.remover_tag("aberto", "jogar-fora", db=self.db, arq=self.arq)

    def test_remover_categoria_deixa_tags_sem_categoria(self):
        tags_marca.remover_categoria("lugar", db=self.db, arq=self.arq)
        self.assertNotIn("lugar", tags_marca.ler(self.arq)["categorias"])
        self.assertIn("camp-nou", [t["slug"] for t in self.painel()["sem_categoria"]])

    def test_toml_com_aspas(self):
        tags_marca.criar_tags('Lugar: Bar "do Zé"', db=self.db, arq=self.arq)
        with open(self.arq, "rb") as fh:
            self.assertEqual(tomllib.load(fh)["categorias"]["lugar"]["tags"]["bar-do-ze"], 'Bar "do Zé"')


if __name__ == "__main__":
    unittest.main()
