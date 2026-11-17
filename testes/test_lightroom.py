"""Tags do Lightroom e dos metadados: `python3 -m unittest discover -s testes`."""
import os as _os
import sys as _sys
_sys.path.insert(0, _os.path.join(_os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))), "src"))  # src/

import os
import shutil
import sqlite3
import struct
import tempfile
import unittest
from unittest import mock
from acervo import biblioteca, lightroom  # noqa: E402

CATEGORIAS = {"lugar": {"nome": "Lugar", "multipla": True, "tags": {"camp-nou": "Camp Nou"}},
              "momento": {"nome": "Momento", "multipla": True, "tags": {"treino": "Treino"}}}

XMP = """<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:lr="http://ns.adobe.com/lightroom/1.0/" xmlns:xmp="http://ns.adobe.com/xap/1.0/"
    xmlns:photoshop="http://ns.adobe.com/photoshop/1.0/" xmp:Rating="4" xmp:Label="Red"
    photoshop:City="Barcelona">
   <dc:subject><rdf:Bag><rdf:li>Camp Nou</rdf:li><rdf:li>Treino</rdf:li><rdf:li>Sub-14</rdf:li></rdf:Bag></dc:subject>
   <lr:hierarchicalSubject><rdf:Bag>
     <rdf:li>Lugar|Camp Nou</rdf:li><rdf:li>Momento|Treino</rdf:li><rdf:li>Equipe|Sub-14</rdf:li>
   </rdf:Bag></lr:hierarchicalSubject>
   <dc:title><rdf:Alt><rdf:li xml:lang="x-default">Treino no Camp Nou</rdf:li></rdf:Alt></dc:title>
   <dc:description><rdf:Alt><rdf:li xml:lang="x-default">Primeiro dia</rdf:li></rdf:Alt></dc:description>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>
<?xpacket end="w"?>"""


def jpeg_com_iptc(registros: list[tuple[int, str]]) -> bytes:
    iptc = b"".join(b"\x1c\x02" + bytes([ds]) + struct.pack(">H", len(v.encode())) + v.encode() for ds, v in registros)
    recurso = b"8BIM" + struct.pack(">H", 0x0404) + b"\x00\x00" + struct.pack(">I", len(iptc)) + iptc
    if len(iptc) % 2:
        recurso += b"\x00"
    seg = b"Photoshop 3.0\x00" + recurso
    return b"\xff\xd8" + b"\xff\xed" + struct.pack(">H", len(seg) + 2) + seg + b"\xff\xda\x00\x02" + b"\x00" * 10


def lrcat_falso(caminho: str, raiz: str) -> None:
    con = sqlite3.connect(caminho)
    con.executescript("""
    create table AgLibraryRootFolder (id_local integer, absolutePath text);
    create table AgLibraryFolder (id_local integer, rootFolder integer, pathFromRoot text);
    create table AgLibraryFile (id_local integer, folder integer, baseName text, extension text);
    create table Adobe_images (id_local integer, rootFile integer);
    create table AgLibraryKeyword (id_local integer, name text, parent integer);
    create table AgLibraryKeywordImage (image integer, tag integer);
    create table AgLibraryCollection (id_local integer, name text);
    create table AgLibraryCollectionImage (collection integer, image integer);
    """)
    con.execute("insert into AgLibraryRootFolder values (1, ?)", (raiz.rstrip("/") + "/",))
    con.execute("insert into AgLibraryFolder values (10, 1, 'Fotos/')")
    con.execute("insert into AgLibraryFile values (100, 10, 'c', 'jpg')")
    con.execute("insert into Adobe_images values (1000, 100)")
    con.executemany("insert into AgLibraryKeyword values (?, ?, ?)",
                    [(1, None, None), (2, "Evento", 1), (3, "BCN Cup", 2), (4, "Avulsa", 1)])
    con.executemany("insert into AgLibraryKeywordImage values (1000, ?)", [(3,), (4,)])
    con.execute("insert into AgLibraryCollection values (7, 'Melhores')")
    con.execute("insert into AgLibraryCollectionImage values (7, 1000)")
    con.commit()
    con.close()


class Lightroom(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.mkdtemp()
        self.pasta = os.path.join(self.tmp, "Viagem")
        os.makedirs(os.path.join(self.pasta, "Fotos"))
        with open(os.path.join(self.pasta, "Fotos", "a.jpg"), "wb") as fh:
            fh.write(b"\xff\xd8\xff\xd9")
        with open(os.path.join(self.pasta, "Fotos", "a.xmp"), "w", encoding="utf-8") as fh:
            fh.write(XMP)
        with open(os.path.join(self.pasta, "Fotos", "b.jpg"), "wb") as fh:
            fh.write(jpeg_com_iptc([(25, "Praia"), (101, "Espanha"), (120, "Fim de tarde")]))
        with open(os.path.join(self.pasta, "Fotos", "c.jpg"), "wb") as fh:
            fh.write(b"\xff\xd8\xff\xd9" + b"c" * 50)
        self.db = os.path.join(self.tmp, "midia.sqlite")
        self.fontes = [{"id": "viagem", "nome": "Viagem", "tipo": "broll", "raiz": self.pasta, "ativo": True}]
        self.p = [mock.patch.object(biblioteca, "categorias_marca", lambda: CATEGORIAS)]
        for p in self.p:
            p.start()
        biblioteca.sincronizar(db=self.db, fontes_cfg=self.fontes)
        con = biblioteca.abrir(self.db)
        self.ids = {os.path.basename(r["caminho"]): r["id"] for r in con.execute("select id, caminho from midia")}
        con.close()

    def tearDown(self):
        for p in self.p:
            p.stop()
        shutil.rmtree(self.tmp, ignore_errors=True)

    def tags_de(self, nome):
        con = biblioteca.abrir(self.db)
        out = {r["slug"]: (r["categoria"], r["origem"], r["por"]) for r in con.execute(
            "select t.slug, t.categoria, t.origem, mt.por from midia_tag mt join tag t on t.id = mt.tag_id "
            "where mt.midia_id = ?", (self.ids[nome],))}
        con.close()
        return out

    def test_ler_xmp(self):
        d = lightroom.ler_xmp(XMP)
        self.assertEqual(d["hier"], [["Lugar", "Camp Nou"], ["Momento", "Treino"], ["Equipe", "Sub-14"]])
        self.assertEqual(d["titulo"], "Treino no Camp Nou")
        self.assertEqual((d["estrelas"], d["cor"], d["cidade"]), (4, "Red", "Barcelona"))
        self.assertEqual(lightroom.ler_xmp("lixo <x:xmpmeta quebrado")["keywords"], [])

    def test_hierarquia_vira_categoria(self):
        ts = {t["slug"]: t for t in lightroom.tags_de(lightroom.ler_xmp(XMP), "lightroom")}
        self.assertEqual(ts["camp-nou"]["categoria"], "lugar")
        self.assertFalse(ts["camp-nou"]["categoria_nova"])
        self.assertEqual(ts["sub-14"]["categoria"], "equipe")
        self.assertTrue(ts["sub-14"]["categoria_nova"])
        self.assertEqual(ts["barcelona"]["categoria"], "lugar")       # cidade vira Lugar
        self.assertNotIn("4-estrelas", ts)                             # estrelas desligadas por padrão
        self.assertIn("4-estrelas", {t["slug"] for t in lightroom.tags_de(lightroom.ler_xmp(XMP), "lightroom", True)})

    def test_iptc(self):
        d = lightroom.iptc_jpeg(os.path.join(self.pasta, "Fotos", "b.jpg"))
        self.assertEqual((d["keywords"], d["pais"], d["legenda"]), (["Praia"], "Espanha", "Fim de tarde"))

    def test_previa_nao_grava_e_aplicar_grava(self):
        ids = list(self.ids.values())
        pr = lightroom.previa(ids, db=self.db)
        self.assertEqual(pr["com_dados"], 2)
        self.assertIn("Equipe", pr["categorias_novas"])
        antes = self.tags_de("a.jpg")
        self.assertNotIn("camp-nou", antes)
        r = lightroom.aplicar(ids, db=self.db)
        self.assertGreater(r["marcacoes"], 0)
        a = self.tags_de("a.jpg")
        self.assertEqual(a["camp-nou"][2], "lightroom")
        self.assertEqual(a["sub-14"][:2], ("equipe", "lightroom"))
        self.assertEqual(self.tags_de("b.jpg")["praia"][1:], ("metadados", "metadados"))
        con = biblioteca.abrir(self.db)
        textos = {r["texto"] for r in con.execute("select texto from midia_texto")}
        con.close()
        self.assertEqual(textos, {"Treino no Camp Nou", "Primeiro dia", "Fim de tarde"})
        busca = biblioteca.listar({"busca": "primeiro"}, db=self.db)
        self.assertEqual([m["id"] for m in busca["itens"]], [self.ids["a.jpg"]])

    def test_conflito_agrupar_ou_separar(self):
        con = biblioteca.abrir(self.db)
        con.execute("insert into tag (slug, nome, categoria, origem) values ('praia', 'Praia', '', 'pasta')")
        con.commit()
        con.close()
        pr = lightroom.previa([self.ids["b.jpg"]], db=self.db)
        praia = next(t for t in pr["tags"] if t["slug"] == "praia")
        self.assertEqual(praia["estado"], "conflito")
        self.assertEqual(praia["existente"]["origem"], "pasta")
        lightroom.aplicar([self.ids["b.jpg"]], escolhas={"praia": "separar"}, db=self.db)
        self.assertIn("praia-metadados", self.tags_de("b.jpg"))
        self.assertNotIn("praia", self.tags_de("b.jpg"))
        lightroom.aplicar([self.ids["b.jpg"]], escolhas={"praia": "agrupar"}, db=self.db)
        self.assertIn("praia", self.tags_de("b.jpg"))

    def test_ignorar(self):
        lightroom.aplicar([self.ids["a.jpg"]], ignorar=["sub-14"], db=self.db)
        self.assertNotIn("sub-14", self.tags_de("a.jpg"))
        self.assertIn("camp-nou", self.tags_de("a.jpg"))

    def test_lrcat_lido_em_copia(self):
        cat = os.path.join(self.tmp, "Catalogo.lrcat")
        lrcat_falso(cat, self.pasta)
        antes = os.path.getmtime(cat)
        dados = lightroom.ler_lrcat(cat)
        arq = os.path.normpath(os.path.join(self.pasta, "Fotos", "c.jpg"))
        self.assertEqual(dados[arq]["hier"], [["Evento", "BCN Cup"], ["Avulsa"]])
        self.assertEqual(dados[arq]["colecoes"], ["Melhores"])
        self.assertEqual(os.path.getmtime(cat), antes)
        lightroom.aplicar([self.ids["c.jpg"]], {"lrcat": cat}, db=self.db)
        c = self.tags_de("c.jpg")
        self.assertEqual(c["bcn-cup"][:2], ("evento", "lightroom"))
        self.assertIn("melhores", c)
        self.assertIn("avulsa", c)

    def test_arquivos_nao_mudam(self):
        arqs = [os.path.join(self.pasta, "Fotos", n) for n in ("a.jpg", "a.xmp", "b.jpg")]
        def ler(a):
            with open(a, "rb") as fh:
                return fh.read()
        antes = [ler(a) for a in arqs]
        lightroom.aplicar(list(self.ids.values()), db=self.db)
        self.assertEqual([ler(a) for a in arqs], antes)


if __name__ == "__main__":
    unittest.main()
