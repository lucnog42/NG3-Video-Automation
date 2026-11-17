//! Gerenciador de tags da marca (dentro da aba Importar), importar tags do Lightroom e dos
//! metadados (`src/acervo/lightroom.py`) e grupos de mídias que viram projeto
//! (`src/acervo/grupos.py`), com o roteiro de cada grupo na aba Roteiros.

use crate::app::App;
use crate::midias::Vista;
use crate::ponte::Pedido;
use rune::TextEdit;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TagGer {
    pub slug: String,
    #[serde(default)]
    pub nome: String,
    #[serde(default)]
    pub origem: String,
    #[serde(default)]
    pub midias: u32,
    #[serde(default)]
    pub cortes: u32,
    #[serde(default)]
    pub n: u32,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct CategoriaGer {
    pub slug: String,
    #[serde(default)]
    pub nome: String,
    #[serde(default)]
    pub multipla: bool,
    #[serde(default)]
    pub da_marca: bool,
    #[serde(default)]
    pub tags: Vec<TagGer>,
    #[serde(default)]
    pub n: u32,
}

/// O gerenciador: categorias (da marca e as que vieram do Lightroom) e as tags sem categoria.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct PainelTags {
    #[serde(default)]
    pub categorias: Vec<CategoriaGer>,
    #[serde(default)]
    pub sem_categoria: Vec<TagGer>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Existente {
    #[serde(default)]
    pub origem: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TagPrevia {
    pub slug: String,
    #[serde(default)]
    pub nome: String,
    #[serde(default)]
    pub categoria_nome: String,
    #[serde(default)]
    pub categoria_nova: bool,
    #[serde(default)]
    pub origem: String,
    #[serde(default)]
    pub n: u32,
    /// existe | nova | conflito
    #[serde(default)]
    pub estado: String,
    #[serde(default)]
    pub existente: Option<Existente>,
}

/// O que viria do Lightroom e dos metadados, antes de gravar.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Previa {
    #[serde(default)]
    pub midias: u32,
    #[serde(default)]
    pub com_dados: u32,
    #[serde(default)]
    pub textos: u32,
    #[serde(default)]
    pub tags: Vec<TagPrevia>,
    #[serde(default)]
    pub categorias_novas: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Grupo {
    pub id: i64,
    #[serde(default)]
    pub nome: String,
    #[serde(default)]
    pub videos: u32,
    #[serde(default)]
    pub imagens: u32,
    #[serde(default)]
    pub pastas: Vec<String>,
}

pub struct Organizar {
    // gerenciador de tags
    pub painel: PainelTags,
    /// aba Importar mostrando o gerenciador de tags
    pub ver_tags: bool,
    pub nova_categoria: TextEdit,
    pub lote: TextEdit,
    pub lote_categoria: String,
    /// tag em edição no gerenciador (slug) e os campos dela
    pub tag_aberta: Option<String>,
    pub tag_nome: TextEdit,
    pub fundir_em: TextEdit,
    /// categoria em edição (slug) e o nome novo
    pub cat_aberta: Option<String>,
    pub cat_nome: TextEdit,
    // importar tags de arquivos
    /// janela aberta, com a vista cuja seleção vai ser lida
    pub janela_tags: Option<Vista>,
    pub xmp: bool,
    pub embutidos: bool,
    pub estrelas: bool,
    pub lrcat: TextEdit,
    pub previa: Option<Previa>,
    pub lendo: bool,
    pub gravando: bool,
    /// conflitos: slug → agrupar | separar (padrão agrupar)
    pub escolhas: BTreeMap<String, String>,
    /// slugs desmarcados na prévia
    pub ignorar: BTreeSet<String>,
    // grupos
    pub grupos: Vec<Grupo>,
    pub janela_grupo: Option<Vista>,
    pub grupo_nome: TextEdit,
    /// grupo escolhido na aba Roteiros: ideias e roteiros só com o conteúdo dele
    pub grupo_ativo: Option<i64>,
}

impl Organizar {
    pub fn novo() -> Organizar {
        Organizar {
            painel: PainelTags::default(), ver_tags: false, nova_categoria: TextEdit::new(""), lote: TextEdit::new(""),
            lote_categoria: String::new(), tag_aberta: None, tag_nome: TextEdit::new(""), fundir_em: TextEdit::new(""),
            cat_aberta: None, cat_nome: TextEdit::new(""),
            janela_tags: None, xmp: true, embutidos: true, estrelas: false, lrcat: TextEdit::new(""), previa: None,
            lendo: false, gravando: false, escolhas: BTreeMap::new(), ignorar: BTreeSet::new(),
            grupos: vec![], janela_grupo: None, grupo_nome: TextEdit::new(""), grupo_ativo: None,
        }
    }

    pub fn grupo(&self, id: i64) -> Option<&Grupo> { self.grupos.iter().find(|g| g.id == id) }
}

impl App {
    // ------------------------------------------------------------ gerenciador de tags
    pub fn tags_painel_recarregar(&mut self) { self.pedir(Pedido::TagsPainel, "tags_painel", json!({})); }

    pub fn ver_gerenciador(&mut self, ver: bool) {
        self.x.o.ver_tags = ver;
        if ver {
            self.x.b.ver_pastas = false;
            self.tags_painel_recarregar();
        }
        self.versao += 1;
    }

    pub fn categoria_criar(&mut self) {
        let nome = self.x.o.nova_categoria.text.trim().to_string();
        if nome.is_empty() { return; }
        self.x.o.nova_categoria = TextEdit::new("");
        self.pedir(Pedido::TagsAcao, "tags_categoria", json!({"acao": "criar", "nome": nome}));
    }

    pub fn categoria_abrir(&mut self, slug: &str, nome: &str) {
        self.x.o.cat_aberta = if self.x.o.cat_aberta.as_deref() == Some(slug) { None } else { Some(slug.to_string()) };
        self.x.o.cat_nome = TextEdit::new(nome);
        self.versao += 1;
    }

    pub fn categoria_renomear(&mut self) {
        let Some(slug) = self.x.o.cat_aberta.clone() else { return };
        let nome = self.x.o.cat_nome.text.trim().to_string();
        if nome.is_empty() { return; }
        self.x.o.cat_aberta = None;
        self.pedir(Pedido::TagsAcao, "tags_categoria", json!({"acao": "renomear", "slug": slug, "nome": nome}));
    }

    pub fn categoria_remover(&mut self, slug: &str) {
        self.x.o.cat_aberta = None;
        self.pedir(Pedido::TagsAcao, "tags_categoria", json!({"acao": "remover", "slug": slug}));
    }

    pub fn tags_criar_lote(&mut self) {
        let texto = self.x.o.lote.text.trim().to_string();
        if texto.is_empty() { return; }
        self.pedir(Pedido::TagsAcao, "tags_criar", json!({"texto": texto, "categoria": self.x.o.lote_categoria}));
        self.x.o.lote = TextEdit::new("");
    }

    pub fn tag_abrir(&mut self, slug: &str, nome: &str) {
        self.x.o.tag_aberta = if self.x.o.tag_aberta.as_deref() == Some(slug) { None } else { Some(slug.to_string()) };
        self.x.o.tag_nome = TextEdit::new(nome);
        self.x.o.fundir_em = TextEdit::new("");
        self.versao += 1;
    }

    fn tag_editar(&mut self, dados: Value) {
        self.pedir(Pedido::TagsAcao, "tag_editar", dados);
    }

    pub fn tag_renomear(&mut self) {
        let Some(slug) = self.x.o.tag_aberta.clone() else { return };
        let nome = self.x.o.tag_nome.text.trim().to_string();
        if nome.is_empty() { return; }
        self.tag_editar(json!({"slug": slug, "acao": "renomear", "nome": nome}));
    }

    pub fn tag_mover(&mut self, categoria: &str) {
        let Some(slug) = self.x.o.tag_aberta.clone() else { return };
        self.tag_editar(json!({"slug": slug, "acao": "mover", "categoria": categoria}));
    }

    /// Remove a tag aberta: `fundir` passa as marcações para a tag digitada em "fundir em".
    pub fn tag_remover(&mut self, fundir: bool) {
        let Some(slug) = self.x.o.tag_aberta.clone() else { return };
        let em = self.x.o.fundir_em.text.trim().to_string();
        if fundir && em.is_empty() { self.aviso = Some("escreva o slug ou o nome da tag que recebe as marcações".into()); return; }
        let em = self.slug_de(&em);
        self.x.o.tag_aberta = None;
        self.tag_editar(json!({"slug": slug, "acao": "remover", "destino": if fundir { "fundir" } else { "apagar" }, "em": em}));
    }

    /// O slug de uma tag pelo nome visível ou pelo próprio slug (para "fundir em").
    fn slug_de(&self, nome: &str) -> String {
        let alvo = nome.trim().to_lowercase();
        for c in &self.x.o.painel.categorias {
            if let Some(t) = c.tags.iter().find(|t| t.slug == alvo || t.nome.to_lowercase() == alvo) { return t.slug.clone(); }
        }
        if let Some(t) = self.x.o.painel.sem_categoria.iter().find(|t| t.slug == alvo || t.nome.to_lowercase() == alvo) {
            return t.slug.clone();
        }
        alvo
    }

    // ------------------------------------------------------------ importar tags de arquivos
    pub fn importar_tags_abrir(&mut self, v: Vista) {
        let o = &mut self.x.o;
        o.janela_tags = Some(v);
        o.previa = None;
        o.escolhas.clear();
        o.ignorar.clear();
        self.versao += 1;
    }

    fn dados_importar(&self) -> Option<Value> {
        let v = self.x.o.janela_tags?;
        let ids = self.ids_em_ordem(v);
        if ids.is_empty() { return None; }
        let o = &self.x.o;
        Some(json!({"ids": ids, "xmp": o.xmp, "embutidos": o.embutidos, "estrelas": o.estrelas,
                    "lrcat": o.lrcat.text.trim(), "escolhas": o.escolhas, "ignorar": o.ignorar}))
    }

    pub fn importar_tags_previa(&mut self) {
        let Some(d) = self.dados_importar() else { self.aviso = Some("selecione mídias na grade".into()); return };
        self.x.o.lendo = true;
        self.pedir(Pedido::TagsPrevia, "tags_importar_previa", d);
    }

    pub fn importar_tags_gravar(&mut self) {
        let Some(d) = self.dados_importar() else { return };
        self.x.o.gravando = true;
        self.pedir(Pedido::TagsImportar, "tags_importar", d);
    }

    pub fn previa_escolher(&mut self, slug: &str, separar: bool) {
        self.x.o.escolhas.insert(slug.to_string(), if separar { "separar" } else { "agrupar" }.into());
        self.versao += 1;
    }

    pub fn previa_alternar(&mut self, slug: &str) {
        let ig = &mut self.x.o.ignorar;
        if !ig.remove(slug) { ig.insert(slug.to_string()); }
        self.versao += 1;
    }

    // ------------------------------------------------------------ grupos
    pub fn grupos_recarregar(&mut self) { self.pedir(Pedido::Grupos, "grupos", json!({})); }

    pub fn grupo_abrir_janela(&mut self, v: Vista) {
        let pasta = self.vista(v).filtros.subpasta.clone();
        let nome = pasta.rsplit('/').next().unwrap_or("").to_string();
        self.x.o.grupo_nome = TextEdit::new(&nome);
        self.x.o.janela_grupo = Some(v);
        self.versao += 1;
    }

    /// Cria o grupo com os selecionados (pode misturar pastas); sem seleção, com a pasta aberta.
    pub fn grupo_criar(&mut self) {
        let Some(v) = self.x.o.janela_grupo else { return };
        let ids = self.ids_em_ordem(v);
        let f = &self.vista(v).filtros;
        let (subpasta, fonte) = if ids.is_empty() { (f.subpasta.clone(), f.fonte.clone()) } else { (String::new(), String::new()) };
        if ids.is_empty() && subpasta.is_empty() {
            self.aviso = Some("selecione mídias (de uma ou várias pastas) ou abra uma pasta".into());
            return;
        }
        let nome = self.x.o.grupo_nome.text.trim().to_string();
        self.pedir(Pedido::GrupoCriar, "grupo_criar", json!({"nome": nome, "ids": ids, "subpasta": subpasta, "fonte": fonte}));
    }

    /// Escolhe o grupo na aba Roteiros (de novo no mesmo: volta a todo o acervo).
    pub fn grupo_ativar(&mut self, id: Option<i64>) {
        let id = if id.is_some() && id == self.x.o.grupo_ativo { None } else { id };
        self.x.o.grupo_ativo = id;
        match id {
            Some(g) => self.pedir(Pedido::Grupo(g), "grupo", json!({"id": g})),
            None => self.roteiros_recarregar(),
        }
        self.versao += 1;
    }

    pub fn grupo_apagar(&mut self, id: i64) {
        if self.x.o.grupo_ativo == Some(id) { self.x.o.grupo_ativo = None; }
        self.pedir(Pedido::GrupoAcao, "grupo_editar", json!({"id": id, "acao": "apagar"}));
    }

    // ------------------------------------------------------------ respostas
    pub(crate) fn resposta_organizar(&mut self, p: Pedido, r: Value) {
        match p {
            Pedido::TagsPainel => self.x.o.painel = serde_json::from_value(r).unwrap_or_default(),
            Pedido::TagsAcao => {
                if let Some(pn) = r.get("painel") { self.x.o.painel = serde_json::from_value(pn.clone()).unwrap_or_default(); }
                if let Some(n) = r.get("marcacoes").and_then(Value::as_u64) {
                    self.aviso = Some(format!("tag removida ({n} marcação(ões) apagadas ou movidas)"));
                } else if let Some(c) = r.get("criadas").and_then(Value::as_array) {
                    self.aviso = Some(format!("{} tag(s) criada(s)", c.len()));
                }
                self.tags_recarregar();
            }
            Pedido::TagsPrevia => {
                self.x.o.lendo = false;
                self.x.o.previa = serde_json::from_value(r).ok();
            }
            Pedido::TagsImportar => {
                let o = &mut self.x.o;
                o.gravando = false;
                o.janela_tags = None;
                o.previa = None;
                self.aviso = Some(format!("tags gravadas: {} tag(s) nova(s), {} marcação(ões), {} título(s)/legenda(s)",
                    r["tags_novas"].as_u64().unwrap_or(0), r["marcacoes"].as_u64().unwrap_or(0), r["textos"].as_u64().unwrap_or(0)));
                for v in [Vista::Acervo, Vista::Fila] {
                    if !self.vista(v).itens.is_empty() { self.midias_recarregar(v); }
                }
                self.tags_recarregar();
                if self.x.o.ver_tags { self.tags_painel_recarregar(); }
            }
            Pedido::Grupos => self.x.o.grupos = serde_json::from_value(r["grupos"].clone()).unwrap_or_default(),
            Pedido::GrupoCriar => {
                self.x.o.janela_grupo = None;
                self.x.o.grupo_nome = TextEdit::new("");
                self.aviso = Some(format!("grupo \"{}\" criado com {} mídia(s): está na aba Roteiros, com o campo de roteiro dele",
                                          r["nome"].as_str().unwrap_or(""), r["n"].as_u64().unwrap_or(0)));
                self.grupos_recarregar();
            }
            Pedido::GrupoAcao => self.grupos_recarregar(),
            Pedido::Grupo(id) => {
                if self.x.o.grupo_ativo == Some(id) {
                    self.x.ideias = serde_json::from_value(r["ideias"].clone()).unwrap_or_default();
                }
            }
            _ => {}
        }
        self.versao += 1;
    }

    pub(crate) fn erro_organizar(&mut self, p: Pedido, erro: String) {
        match p {
            Pedido::TagsPrevia => self.x.o.lendo = false,
            Pedido::TagsImportar => self.x.o.gravando = false,
            _ => {}
        }
        self.aviso = Some(erro);
    }
}
