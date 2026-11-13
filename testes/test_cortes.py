"""Cortes como anotação: `python3 -m unittest discover -s testes`. Sem vídeo, GPU nem acervo real."""
import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.join(_os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))), "src"))  # src/

import os
import shutil
import tempfile
import unittest
from unittest import mock

from acervo import biblioteca, cortes  # noqa: E402


def palavras(*frases):
    """[(ini, texto)] -> palavras com 0,3 s cada e 0,05 s entre elas."""
    out = []
    for ini, txt in frases:
        t = ini
        for w in txt.split():
            out.append((t, t + 0.3, w))
            t += 0.35
    return out


class Cortes(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.db = os.path.join(self.tmp, "midia.sqlite")
        con = cortes.abrir(self.db)
        self.mid = con.execute("insert into midia (fonte_id, caminho, tipo, status_import) values "
                               "('f', '/x/v.mp4', 'video', 'novo')").lastrowid
        self.outro = con.execute("insert into midia (fonte_id, caminho, tipo, status_import) values "
                                 "('f', '/x/w.mp4', 'video', 'novo')").lastrowid
        con.commit()
        con.close()
        self.cats = biblioteca.categorias_marca
        biblioteca.categorias_marca = lambda: {}

    def tearDown(self):
        biblioteca.categorias_marca = self.cats
        shutil.rmtree(self.tmp, ignore_errors=True)

    def lista(self, **kw):
        return cortes.listar([self.mid], db=self.db, **kw)

    def test_ideias_da_fala_separam_pela_pausa(self):
        ws = palavras((0.0, "primeira frase bem curta aqui."), (2.5, "segunda frase que vem depois."),
                      (8.0, "outra ideia bem longe no tempo."))
        ideias = cortes.ideias_da_fala(ws, {"pausa_ideia": 1.2, "minimo": 1.0, "maximo": 30.0, "folga": 0.1})
        self.assertEqual(len(ideias), 2)                      # 0–4 s (2 frases) e 8 s em diante
        self.assertLess(ideias[0][1], 8.0)
        self.assertGreaterEqual(ideias[1][0], 7.8)
        self.assertIn("segunda", ideias[0][2])

    def test_ideia_longa_e_quebrada(self):
        frases = [(i * 3.0, "uma frase curta que fala.") for i in range(20)]   # 60 s sem pausa longa
        ideias = cortes.ideias_da_fala(palavras(*frases), {"pausa_ideia": 1.2, "minimo": 2.0, "maximo": 20.0})
        self.assertTrue(all(b - a <= 21.0 for a, b, _ in ideias), ideias)
        self.assertGreater(len(ideias), 2)

    def test_reprocessar_troca_so_os_propostos_intocados(self):
        cortes.gravar_propostas(self.mid, "fala", [(0, 5, "a"), (6, 10, "b"), (11, 15, "c")], db=self.db)
        ids = {c["texto"]: c["id"] for c in self.lista()}
        cortes.aceitar([ids["a"]], db=self.db)
        cortes.editar(ids["b"], ini=6.5, db=self.db)          # mexer na borda protege
        manual = cortes.criar(self.mid, 20, 25, "meu", db=self.db)
        cortes.gravar_propostas(self.mid, "fala", [(0, 5, "a2"), (6, 10, "b2"), (30, 34, "novo")], db=self.db)
        r = {c["id"]: c for c in self.lista()}
        self.assertFalse(r[ids["a"]]["proposto"])
        self.assertEqual(r[ids["b"]]["ini"], 6.5)
        self.assertTrue(r[manual]["protegido"])
        textos = sorted(c["texto"] for c in r.values())
        self.assertEqual(textos, ["", "a", "b", "novo"])      # c sumiu; a2 e b2 batem com protegidos

    def test_descartado_nao_volta_e_manual_some(self):
        cortes.gravar_propostas(self.mid, "cena", [(0, 4), (5, 9)], db=self.db)
        prim = self.lista()[0]["id"]
        cortes.descartar([prim], db=self.db)
        cortes.gravar_propostas(self.mid, "cena", [(0, 4), (5, 9)], db=self.db)
        self.assertEqual([(c["ini"], c["fim"]) for c in self.lista()], [(5, 9)])
        self.assertEqual(len(self.lista(descartados=True)), 2)
        m = cortes.criar(self.mid, 10, 12, db=self.db)
        cortes.descartar([m], db=self.db)
        self.assertEqual(len(self.lista(descartados=True)), 2)   # manual apagado de vez

    def test_dividir_e_juntar_levam_as_tags(self):
        c = cortes.criar(self.mid, 0, 10, db=self.db)
        cortes.aplicar_tags([c], ["gol"], db=self.db)
        a, b = cortes.dividir(c, 4.0, db=self.db)
        r = {x["id"]: x for x in self.lista()}
        self.assertEqual((r[a]["fim"], r[b]["ini"], r[b]["fim"]), (4.0, 4.0, 10))
        self.assertEqual([t["slug"] for t in r[b]["tags"]], ["gol"])
        cortes.aplicar_tags([b], ["comemoracao"], db=self.db)
        j = cortes.juntar([a, b], db=self.db)
        r = self.lista()
        self.assertEqual(len(r), 1)
        self.assertEqual((r[0]["id"], r[0]["ini"], r[0]["fim"]), (j, 0, 10))
        self.assertEqual(sorted(t["slug"] for t in r[0]["tags"]), ["comemoracao", "gol"])
        with self.assertRaises(ValueError):
            cortes.dividir(j, 0.1, db=self.db)                  # fora do corte

    def test_juntar_so_do_mesmo_video(self):
        a = cortes.criar(self.mid, 0, 2, db=self.db)
        b = cortes.criar(self.outro, 0, 2, db=self.db)
        with self.assertRaises(ValueError):
            cortes.juntar([a, b], db=self.db)

    def test_tag_do_corte_independente_e_copia_do_video(self):
        biblioteca.aplicar_tags([self.mid], ["treino"], db=self.db)
        con = biblioteca.abrir(self.db)
        biblioteca._marcar(con, self.mid, biblioteca._garantir_tag(con, "Pasta X", origem="pasta"), "pasta")
        con.commit()
        con.close()
        c = cortes.criar(self.mid, 1, 3, db=self.db)
        self.assertEqual(self.lista()[0]["tags"], [])           # nada herdado sozinho
        cortes.copiar_tags_do_video([c], db=self.db)
        self.assertEqual([t["slug"] for t in self.lista()[0]["tags"]], ["treino"])   # sem a de pasta
        cortes.aplicar_tags([c], ["treino"], remover=True, db=self.db)
        self.assertEqual(self.lista()[0]["tags"], [])
        self.assertIn("treino", [t["slug"] for t in biblioteca.listar({}, db=self.db)["itens"][0]["tags"]])

    def test_contagem_e_corte_minimo(self):
        cortes.gravar_propostas(self.mid, "fala", [(0, 5, "a"), (6, 6.1, "curto")], db=self.db)
        cortes.criar(self.mid, 7, 9, db=self.db)
        self.assertEqual(cortes.contagem([self.mid], db=self.db), {self.mid: {"cortes": 2, "propostos": 1}})
        with self.assertRaises(ValueError):
            cortes.criar(self.mid, 3, 3.1, db=self.db)

    def test_propor_fala_sem_transcricao(self):
        from transcricao import legendar
        with mock.patch.object(cortes, "_abs", side_effect=lambda c: c), \
             mock.patch.object(legendar, "caminho_saida", return_value="/nao/existe.words.json"):
            r = cortes.propor([self.mid], "fala", db=self.db)
        self.assertEqual(r[0]["erro"], "sem transcrição")

    def test_cenas_juntam_as_curtas(self):
        saida = mock.Mock(returncode=0, stdout="[[0, 5], [5, 5.5], [5.5, 12]]", stderr="")
        with mock.patch.object(cortes.caminhos, "python_de", return_value=__file__), \
             mock.patch.object(cortes.subprocess, "run", return_value=saida):
            self.assertEqual(cortes.cenas_do_video("/x/v.mp4", {"limiar": 27, "minimo": 1.5}), [(0, 5.5), (5.5, 12)])


if __name__ == "__main__":
    unittest.main()
