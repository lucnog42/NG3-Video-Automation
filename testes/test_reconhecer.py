"""Metadados, GPS → lugar e as sugestões da IA de imagem: `python3 -m unittest discover -s testes`.
Sem modelo, sem GPU, sem rede: a IA é simulada; só a regra de decisão e o banco são testados."""
import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.join(_os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))), "src"))  # src/

import os
import shutil
import tempfile
import unittest

from acervo import biblioteca, reconhecer  # noqa: E402

CATS = {"lugar": {"nome": "Lugar", "multipla": True, "tags": {"estadio": "Estádio", "praia": "Praia", "ct": "CT"}},
        "momento": {"nome": "Momento", "multipla": True, "tags": {"jogo": "Jogo", "treino": "Treino"}},
        "plano": {"nome": "Plano", "multipla": False, "tags": {"aberto": "Aberto", "fechado": "Fechado"}},
        "pessoa": {"nome": "Pessoa", "multipla": True, "tags": {"fulano": "Fulano"}}}
LUGARES = {"estadio": {"lat": 41.3809, "lon": 2.1228, "raio_m": 400},
           "cidade": {"lat": 41.3874, "lon": 2.1686, "raio_m": 9000}}


class Reconhecer(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.db = os.path.join(self.tmp, "midia.sqlite")
        self.cats = biblioteca.categorias_marca
        biblioteca.categorias_marca = lambda: CATS
        con = reconhecer.abrir(self.db)
        self.mid = con.execute("insert into midia (fonte_id, caminho, tipo, status_import) values ('f', '/x/a.jpg', 'imagem', 'novo')").lastrowid
        con.commit()
        con.close()

    def tearDown(self):
        biblioteca.categorias_marca = self.cats
        shutil.rmtree(self.tmp, ignore_errors=True)

    def test_gps_do_mp4_e_lugares_no_raio(self):
        self.assertEqual(reconhecer.gps_iso6709("+41.3810+002.1230+012.000/"), (41.381, 2.123))
        self.assertIsNone(reconhecer.gps_iso6709("nada"))
        self.assertEqual(reconhecer.lugares_perto(41.381, 2.123, LUGARES), ["estadio", "cidade"])   # mais perto antes
        self.assertEqual(reconhecer.lugares_perto(41.40, 2.17, LUGARES), ["cidade"])
        self.assertEqual(reconhecer.lugares_perto(40.0, -3.7, LUGARES), [])

    def test_rotulos_sem_pessoa_e_com_frase_da_marca(self):
        rs = reconhecer.rotulos({"categorias": ["lugar", "momento", "plano"], "frase": "foto de {nome}"})
        self.assertNotIn("fulano", [r["slug"] for r in rs])
        self.assertIn("foto de Praia", [r["texto"] for r in rs])

    def test_escolher_relativo_por_categoria(self):
        rs = reconhecer.rotulos({"categorias": ["lugar", "momento", "plano"]})
        lg = {"estadio": 2.0, "praia": -3.0, "ct": -3.0,        # lugar: claro
              "jogo": -2.0, "treino": -2.2,                     # momento: dividido (2 entram: aceita várias)
              "aberto": -8.0, "fechado": -8.5}                   # plano: sem evidência nenhuma
        out = dict(reconhecer.escolher(lg, rs, {"relativo_min": 0.5, "segundo_min": 0.3, "evidencia_min": 0.005}))
        self.assertIn("estadio", out)
        self.assertIn("jogo", out)
        self.assertIn("treino", out)
        self.assertNotIn("aberto", out)
        # por categoria: lugar mais exigente não sugere um lugar duvidoso
        lg2 = {**lg, "estadio": -1.0, "praia": -1.5}
        out2 = dict(reconhecer.escolher(lg2, rs, {"relativo_min": 0.5, "por_categoria": {"lugar": {"relativo_min": 0.8}}}))
        self.assertNotIn("estadio", out2)

    def test_sugestao_aceita_vira_tag_e_recusada_nao_volta(self):
        con = reconhecer.abrir(self.db)
        self.assertEqual(reconhecer.gravar_sugestoes(con, self.mid, [("praia", 0.9), ("jogo", 0.7)], "m"), 2)
        con.commit()
        con.close()
        item = biblioteca.listar({}, db=self.db)["itens"][0]
        self.assertEqual([s["slug"] for s in item["sugestoes"]], ["praia", "jogo"])
        self.assertEqual(item["tags"], [])                                          # sugestão não é tag
        self.assertEqual(biblioteca.listar({"passo": "com_sugestoes"}, db=self.db)["total"], 1)
        self.assertEqual(reconhecer.decidir([self.mid], True, ["praia"], db=self.db), 1)
        self.assertEqual(reconhecer.decidir([self.mid], False, db=self.db), 1)      # recusa o jogo
        item = biblioteca.listar({}, db=self.db)["itens"][0]
        self.assertEqual([(t["slug"], t["por"]) for t in item["tags"]], [("praia", "ia")])
        self.assertEqual(item["sugestoes"], [])
        con = reconhecer.abrir(self.db)
        self.assertEqual(reconhecer.gravar_sugestoes(con, self.mid, [("praia", 0.9), ("jogo", 0.8)], "m"), 0)
        con.commit()
        con.close()

    def test_gps_manda_no_lugar(self):
        from unittest import mock

        class Falso:
            disp = "cpu"

            def __init__(self, *a, **k):
                pass

            def analisar(self, imgs):          # a IA "acha" que é estádio e jogo
                return [3.0 if r["slug"] in ("estadio", "jogo") else -4.0 for r in reconhecer.rotulos(self.c)], [1.0, 0.0]
        Falso.c = {"categorias": ["lugar", "momento", "plano"]}
        c = {**Falso.c, "modelo": "falso", "quadros_por_video": 1}
        with mock.patch.object(reconhecer, "cfg", return_value=c), \
             mock.patch.object(reconhecer, "Modelo", Falso), \
             mock.patch.object(reconhecer, "quadros", return_value=["img"]), \
             mock.patch.object(reconhecer, "metadados_imagem", return_value={"lat": 41.69, "lon": 2.84}), \
             mock.patch.object(reconhecer, "lugares_perto", return_value=["praia"]):
            r = reconhecer.reconhecer([self.mid], db=self.db)[0]
        self.assertEqual(r["gps"], ["praia"])
        item = biblioteca.listar({}, db=self.db)["itens"][0]
        self.assertEqual([(t["slug"], t["por"]) for t in item["tags"]], [("praia", "metadados")])
        self.assertEqual([s["slug"] for s in item["sugestoes"]], ["jogo"])        # estádio não: o GPS manda

    def test_plano_medido_por_rosto_e_drone(self):
        self.assertEqual(reconhecer.plano_medido([0.0, 0.31, 0.4], {}), ("fechado", 0.9))
        self.assertEqual(reconhecer.plano_medido([0.18, 0.0, 0.2], {}), ("medio", 0.85))
        self.assertEqual(reconhecer.plano_medido([0.1], {}), ("aberto", 0.75))
        self.assertIsNone(reconhecer.plano_medido([0.0, 0.0], {}))                     # sem rosto: fica com a IA
        self.assertEqual(reconhecer.plano_medido([0.4], {"camera": "DJI FC3170"}), ("drone", 1.0))

    def test_fronteiras_do_plano_vem_dos_exemplos(self):
        con = reconhecer.abrir(self.db)
        for k, (plano, rosto) in enumerate([("fechado", 0.40)] * 3 + [("medio", 0.12)] * 3 + [("aberto", 0.04)] * 3):
            mid = con.execute("insert into midia (fonte_id, caminho, tipo, status_import) values ('f', ?, 'video', 'novo')",
                              (f"/x/p{k}.mp4",)).lastrowid
            con.execute("insert into midia_meta (midia_id, chave, valor) values (?, 'rosto', ?)", (mid, str(rosto)))
            biblioteca._marcar(con, mid, biblioteca._garantir_tag(con, plano), "humano")
        lim = reconhecer.limiares_aprendidos(con)
        con.close()
        self.assertEqual(lim, {"fechado": 0.26, "medio": 0.08})
        # com essas fronteiras, um rosto de 9% (que o padrão chamaria de aberto) vira médio
        self.assertEqual(reconhecer.plano_medido([0.09], {}), ("aberto", 0.75))
        self.assertEqual(reconhecer.plano_medido([0.09], {}, lim), ("medio", 0.85))

    def test_aprende_com_os_exemplos_da_pessoa(self):
        rs = reconhecer.rotulos({"categorias": ["plano"]})
        con = reconhecer.abrir(self.db)
        ids = []
        for k in range(12):                     # 6 "aberto" perto de [1, 0], 6 "fechado" perto de [0, 1]
            mid = con.execute("insert into midia (fonte_id, caminho, tipo, status_import) values ('f', ?, 'imagem', 'novo')",
                              (f"/x/{k}.jpg",)).lastrowid
            emb = [1.0, 0.1] if k < 6 else [0.1, 1.0]
            reconhecer.gravar_impressao(con, mid, emb, "m")
            biblioteca._marcar(con, mid, biblioteca._garantir_tag(con, "aberto" if k < 6 else "fechado"), "humano")
            ids.append(mid)
        protos = reconhecer.prototipos(con, rs, "m", minimo=5)
        con.close()
        self.assertEqual(sorted(protos), ["aberto", "fechado"])
        # a IA genérica diz "aberto"; os exemplos dizem "fechado" com folga: os exemplos pesam o dobro
        lg = {"aberto": -2.0, "fechado": -2.6}
        out = dict(reconhecer.escolher_com_exemplos(lg, [0.05, 1.0], protos, rs, {"relativo_min": 0.55}))
        self.assertIn("fechado", out)
        self.assertNotIn("aberto", out)
        # sem exemplos suficientes, vale só a IA
        self.assertIn("aberto", dict(reconhecer.escolher_com_exemplos(lg, [0.05, 1.0], {}, rs, {"relativo_min": 0.55})))

    def test_sugestao_de_corte_aceita_vira_tag_do_corte(self):
        from acervo import cortes
        cid = cortes.criar(self.mid, 0, 4, db=self.db)
        con = reconhecer.abrir(self.db)
        self.assertEqual(reconhecer.gravar_sugestoes_corte(con, cid, [("fechado", 0.9), ("jogo", 0.7)], "m"), 2)
        con.commit()
        con.close()
        self.assertEqual([s["slug"] for s in cortes.listar([self.mid], db=self.db)[0]["sugestoes"]], ["fechado", "jogo"])
        self.assertEqual(reconhecer.decidir_cortes([cid], True, ["fechado"], db=self.db), 1)
        self.assertEqual(reconhecer.decidir_cortes([cid], False, db=self.db), 1)
        c = cortes.listar([self.mid], db=self.db)[0]
        self.assertEqual(([t["slug"] for t in c["tags"]], c["sugestoes"]), (["fechado"], []))
        self.assertEqual(biblioteca.listar({}, db=self.db)["itens"][0]["tags"], [])   # o vídeo não ganha a tag do corte

    def test_metadados_gravados_aparecem_na_lista(self):
        con = reconhecer.abrir(self.db)
        reconhecer.gravar_metadados(con, self.mid, {"data": "2025:04:12 10:00:00", "camera": "Nikon D500", "lat": 41.38})
        con.commit()
        con.close()
        meta = biblioteca.listar({}, db=self.db)["itens"][0]["meta"]
        self.assertEqual((meta["camera"], meta["lat"]), ("Nikon D500", "41.38"))


if __name__ == "__main__":
    unittest.main()
