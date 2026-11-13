//! A biblioteca de mídia (`src/acervo/biblioteca.py`): a aba Acervo (tudo junto) e a fila de
//! revisão da aba Importar. As duas são a mesma grade com filtros diferentes; cada uma guarda a
//! sua página, a sua seleção e os seus filtros.

use crate::app::{Aba, App};
use crate::dados::{Midia, ResumoMidia, TagsMarca};
use crate::ponte::Pedido;
use rune::TextEdit;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const POR_PAGINA: usize = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vista {
    /// aba Acervo: todas as fontes
    Acervo,
    /// aba Importar: só as pastas apontadas, começando pela fila de revisão
    Fila,
}

#[derive(Clone, Debug, Default)]
pub struct FiltrosMidia {
    /// "" | "video" | "imagem"
    pub tipo: String,
    pub fonte: String,
    /// "" | "fila" | "sem_tags" | "com_tags" | "revisado"
    pub passo: String,
    pub importacao: Option<i64>,
    pub subpasta: String,
    pub tags: Vec<String>,
}

pub struct VistaMidia {
    pub filtros: FiltrosMidia,
    pub busca: TextEdit,
    pub itens: Vec<Midia>,
    pub total: usize,
    pub resumo: ResumoMidia,
    pub pagina: usize,
    pub selecao: BTreeSet<i64>,
    pub carregando: bool,
    /// número do último pedido: resposta de pedido anterior (filtro já mudou) é ignorada
    pub seq: u64,
}

impl VistaMidia {
    fn nova(passo: &str) -> VistaMidia {
        VistaMidia {
            filtros: FiltrosMidia { passo: passo.into(), ..Default::default() }, busca: TextEdit::new(""),
            itens: vec![], total: 0, resumo: ResumoMidia::default(), pagina: 0, selecao: BTreeSet::new(),
            carregando: false, seq: 0,
        }
    }
    pub fn n_paginas(&self) -> usize { self.total.div_ceil(POR_PAGINA).max(1) }
    /// Os selecionados que estão na página carregada.
    pub fn selecionados(&self) -> impl Iterator<Item = &Midia> { self.itens.iter().filter(|m| self.selecao.contains(&m.id)) }
}

pub struct Biblioteca {
    pub acervo: VistaMidia,
    pub fila: VistaMidia,
    pub tags: TagsMarca,
    pub nova_tag: TextEdit,
    pub sincronizando: bool,
    /// janela "Importar pasta"
    pub janela_importar: bool,
    /// aba Importar mostrando a lista de pastas apontadas em vez da fila
    pub ver_pastas: bool,
}

impl Biblioteca {
    pub fn novo_vazio() -> Biblioteca {
        Biblioteca {
            acervo: VistaMidia::nova(""), fila: VistaMidia::nova("fila"), tags: TagsMarca::default(),
            nova_tag: TextEdit::new(""), sincronizando: false, janela_importar: false, ver_pastas: false,
        }
    }
}

impl App {
    pub fn vista(&self, v: Vista) -> &VistaMidia {
        match v { Vista::Acervo => &self.x.b.acervo, Vista::Fila => &self.x.b.fila }
    }
    fn vista_mut(&mut self, v: Vista) -> &mut VistaMidia {
        match v { Vista::Acervo => &mut self.x.b.acervo, Vista::Fila => &mut self.x.b.fila }
    }

    fn filtros_json(&self, v: Vista) -> Value {
        let vm = self.vista(v);
        let f = &vm.filtros;
        json!({"importadas": v == Vista::Fila, "tipo": f.tipo, "fonte": f.fonte, "passo": f.passo,
               "importacao": f.importacao, "subpasta": f.subpasta, "tags": f.tags, "busca": vm.busca.text.trim()})
    }

    pub fn midias_recarregar(&mut self, v: Vista) {
        let dados = json!({"filtros": self.filtros_json(v), "limite": POR_PAGINA, "offset": self.vista(v).pagina * POR_PAGINA});
        let vm = self.vista_mut(v);
        vm.seq += 1;
        vm.carregando = true;
        let seq = vm.seq;
        self.pedir(Pedido::Midias(v, seq), "midias", dados);
    }

    /// Muda um filtro e volta para a página 1.
    pub fn midia_filtro(&mut self, v: Vista, f: impl FnOnce(&mut FiltrosMidia)) {
        let vm = self.vista_mut(v);
        f(&mut vm.filtros);
        vm.pagina = 0;
        self.midias_recarregar(v);
    }

    pub fn midia_busca(&mut self, v: Vista) {
        self.vista_mut(v).pagina = 0;
        self.midias_recarregar(v);
    }

    pub fn midia_pagina(&mut self, v: Vista, p: usize) {
        let vm = self.vista_mut(v);
        let p = p.min(vm.n_paginas() - 1);
        if p == vm.pagina { return; }
        vm.pagina = p;
        self.midias_recarregar(v);
    }

    pub fn midia_alternar(&mut self, v: Vista, id: i64) {
        let s = &mut self.vista_mut(v).selecao;
        if !s.remove(&id) { s.insert(id); }
        self.versao += 1;
    }

    pub fn midia_limpar_selecao(&mut self, v: Vista) {
        self.vista_mut(v).selecao.clear();
        self.versao += 1;
    }

    /// Seleciona tudo o que passa no filtro (todas as páginas); se já estava tudo, limpa.
    pub fn midia_selecionar_tudo(&mut self, v: Vista) {
        let vm = self.vista(v);
        if vm.total > 0 && vm.selecao.len() >= vm.total {
            self.midia_limpar_selecao(v);
            return;
        }
        let dados = json!({"filtros": self.filtros_json(v)});
        self.pedir(Pedido::MidiaIds(v), "midia_ids", dados);
    }

    /// Põe (ou tira) uma tag em todos os selecionados.
    pub fn midia_tag(&mut self, v: Vista, tag: &str, remover: bool) {
        let ids: Vec<i64> = self.vista(v).selecao.iter().copied().collect();
        if ids.is_empty() || tag.trim().is_empty() { return; }
        self.pedir(Pedido::MidiaAcao(v), "midia_tags", json!({"ids": ids, "tags": [tag.trim()], "remover": remover}));
    }

    /// A tag digitada no campo: da marca se o nome bater, senão vira tag livre.
    pub fn midia_tag_digitada(&mut self, v: Vista) {
        let t = self.x.b.nova_tag.text.trim().to_string();
        if t.is_empty() { return; }
        self.midia_tag(v, &t, false);
        self.x.b.nova_tag = TextEdit::new("");
    }

    pub fn midia_revisar(&mut self, v: Vista, revisado: bool) {
        let ids: Vec<i64> = self.vista(v).selecao.iter().copied().collect();
        if ids.is_empty() { return; }
        self.pedir(Pedido::MidiaAcao(v), "midia_revisar", json!({"ids": ids, "revisado": revisado}));
        if revisado && v == Vista::Fila { self.vista_mut(v).selecao.clear(); }
    }

    pub fn tags_recarregar(&mut self) { self.pedir(Pedido::Tags, "tags", json!({})); }

    pub fn midia_sincronizar(&mut self) {
        self.x.b.sincronizando = true;
        self.pedir(Pedido::MidiaSincronizar, "midia_sincronizar", json!({}));
    }

    /// Abre o arquivo no programa padrão do sistema (player, visualizador).
    pub fn midia_abrir(&mut self, caminho: &str) {
        self.pedir(Pedido::Abrir, "abrir", json!({"arquivo": caminho}));
    }

    /// Ao entrar na aba Acervo ou Importar.
    pub fn midias_ao_entrar(&mut self, v: Vista) {
        self.midias_recarregar(v);
        self.tags_recarregar();
    }

    /// O motor terminou de varrer as fontes.
    pub fn biblioteca_atualizada(&mut self, r: &Value) {
        self.x.b.sincronizando = false;
        let novos: u64 = r["resultado"].as_object()
            .map(|o| o.values().filter_map(|f| f["novos"].as_u64()).sum()).unwrap_or(0);
        if novos > 0 { self.aviso = Some(format!("{novos} mídia(s) nova(s) no catálogo")); }
        match self.aba {
            Aba::Acervo => self.midias_ao_entrar(Vista::Acervo),
            Aba::Importar => self.midias_ao_entrar(Vista::Fila),
            _ => {}
        }
    }

    pub(crate) fn resposta_midias(&mut self, p: Pedido, r: Value) {
        match p {
            Pedido::Midias(v, seq) => {
                if seq != self.vista(v).seq { return; }
                let itens: Vec<Midia> = serde_json::from_value(r["itens"].clone()).unwrap_or_default();
                for m in &itens {
                    if !self.miniaturas.contains_key(&m.caminho) && !self.falhas.contains(&m.caminho) {
                        self.querem_miniatura.push(m.caminho.clone());
                    }
                }
                if r["sincronizando"].as_bool() == Some(true) { self.x.b.sincronizando = true; }
                let vm = self.vista_mut(v);
                vm.itens = itens;
                vm.total = r["total"].as_u64().unwrap_or(0) as usize;
                vm.resumo = serde_json::from_value(r["resumo"].clone()).unwrap_or_default();
                vm.carregando = false;
            }
            Pedido::MidiaIds(v) => {
                let ids: Vec<i64> = serde_json::from_value(r["ids"].clone()).unwrap_or_default();
                self.vista_mut(v).selecao = ids.into_iter().collect();
            }
            Pedido::MidiaAcao(v) => {
                self.midias_recarregar(v);
                let outra = if v == Vista::Acervo { Vista::Fila } else { Vista::Acervo };
                if !self.vista(outra).itens.is_empty() { self.midias_recarregar(outra); }
                self.tags_recarregar();
            }
            Pedido::Tags => self.x.b.tags = serde_json::from_value(r).unwrap_or_default(),
            Pedido::MidiaSincronizar => {
                if r["comecou"].as_bool() == Some(true) {
                    self.aviso = Some("procurando arquivos novos nas fontes…".into());
                }
            }
            _ => {}
        }
        self.versao += 1;
    }
}
