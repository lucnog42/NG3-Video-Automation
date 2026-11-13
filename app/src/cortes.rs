//! A aba Cortes (`src/acervo/cortes.py`): biblioteca de vídeos à esquerda, o vídeo aberto com a
//! tira de quadros e a linha do tempo dos cortes no centro, e os cortes à direita. Cortes são
//! anotações (entrada/saída + tags + nota): o arquivo do vídeo nunca muda.

use crate::app::App;
use crate::dados::{Midia, Tira};
use crate::midias::Vista;
use crate::ponte::Pedido;
use rune::{Image, TextEdit};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Quadros da tira dos cortes: mais que os 16 do preview, para marcar entrada e saída.
pub const QUADROS: usize = 60;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TagCorte {
    pub slug: String,
    #[serde(default)]
    pub nome: String,
    #[serde(default)]
    pub origem: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Corte {
    pub id: i64,
    pub midia: i64,
    pub ini: f32,
    pub fim: f32,
    /// fala | cena | manual
    pub origem: String,
    #[serde(default)]
    pub proposto: bool,
    #[serde(default)]
    pub protegido: bool,
    #[serde(default)]
    pub nota: String,
    #[serde(default)]
    pub texto: String,
    #[serde(default)]
    pub tags: Vec<TagCorte>,
}

impl Corte {
    pub fn status(&self) -> &'static str {
        if self.proposto { "proposto" } else if self.origem == "manual" { "manual" } else { "aceito" }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct Contagem {
    #[serde(default)]
    pub cortes: usize,
    #[serde(default)]
    pub propostos: usize,
}

pub struct CortesUi {
    /// o vídeo aberto no centro
    pub video: Option<Midia>,
    pub cortes: Vec<Corte>,
    /// os cortes de todos os vídeos selecionados na lista (lado direito, "Selecionados")
    pub dos_selecionados: Vec<Corte>,
    pub ver_selecionados: bool,
    pub contagem: HashMap<i64, Contagem>,
    pub quadro: usize,
    /// entrada marcada com I, à espera da saída (O)
    pub entrada: Option<f32>,
    pub sel: Option<i64>,
    pub nova_tag: TextEdit,
    pub nota: TextEdit,
    pub tira: Option<(Tira, Image)>,
    pub caminhos: Vec<String>,
    pub imagens: HashMap<usize, Image>,
    pub propondo: bool,
}

impl CortesUi {
    pub fn novo() -> CortesUi {
        CortesUi {
            video: None, cortes: vec![], dos_selecionados: vec![], ver_selecionados: false, contagem: HashMap::new(),
            quadro: 0, entrada: None, sel: None, nova_tag: TextEdit::new(""), nota: TextEdit::new(""), tira: None,
            caminhos: vec![], imagens: HashMap::new(), propondo: false,
        }
    }
    pub fn tempo(&self) -> f32 {
        self.tira.as_ref().and_then(|(t, _)| t.tempos.get(self.quadro).copied()).unwrap_or(0.0)
    }
    pub fn duracao(&self) -> f32 { self.tira.as_ref().map_or(0.0, |(t, _)| t.duracao) }
    pub fn corte(&self, id: i64) -> Option<&Corte> {
        self.cortes.iter().chain(self.dos_selecionados.iter()).find(|c| c.id == id)
    }
}

impl App {
    fn cu(&mut self) -> &mut CortesUi { &mut self.x.c }

    /// Ao entrar na aba: a lista de vídeos (só vídeos) e as tags da marca.
    pub fn cortes_ao_entrar(&mut self) {
        self.x.b.cortes.filtros.tipo = "video".into();
        self.midias_ao_entrar(Vista::Cortes);
        if let Some(v) = self.x.c.video.as_ref().map(|m| m.id) { self.cortes_recarregar(v); }
    }

    /// Quantos cortes cada vídeo da página tem (chamado quando a lista chega).
    pub fn cortes_contar(&mut self) {
        let ids: Vec<i64> = self.x.b.cortes.itens.iter().map(|m| m.id).collect();
        if !ids.is_empty() {
            self.pedir(Pedido::CortesContagem, "cortes_de", json!({"midias": ids, "so_contagem": true}));
        }
    }

    pub fn cortes_abrir(&mut self, m: Midia) {
        self.x.player.parar();
        let v = m.caminho.clone();
        let id = m.id;
        let c = self.cu();
        if c.video.as_ref().map(|x| x.id) != Some(id) {
            c.tira = None;
            c.caminhos.clear();
            c.imagens.clear();
            c.quadro = 0;
            c.entrada = None;
            c.sel = None;
        }
        c.video = Some(m);
        c.ver_selecionados = false;
        self.pedir(Pedido::CorteTira(v.clone()), "tira", json!({"arquivo": v, "n": QUADROS, "px": 120}));
        self.pedir(Pedido::CorteQuadros(v.clone()), "quadros", json!({"arquivo": v, "n": QUADROS, "px": 720}));
        self.cortes_recarregar(id);
    }

    pub fn cortes_recarregar(&mut self, midia: i64) {
        self.pedir(Pedido::CortesDe(midia), "cortes_de", json!({"midias": [midia]}));
    }

    pub fn cortes_dos_selecionados(&mut self) {
        let ids: Vec<i64> = self.x.b.cortes.selecao.iter().copied().collect();
        self.x.c.ver_selecionados = true;
        if ids.is_empty() { self.x.c.dos_selecionados.clear(); return; }
        self.pedir(Pedido::CortesSelecionados, "cortes_de", json!({"midias": ids}));
    }

    /// Vai para o quadro `i` da tira e decodifica ele e os vizinhos.
    pub fn cortes_quadro(&mut self, i: usize) {
        let c = &mut self.x.c;
        let n = c.tira.as_ref().map_or(QUADROS, |(t, _)| t.n);
        c.quadro = i.min(n.saturating_sub(1));
        let q = c.quadro;
        for k in [q, q + 1, q.wrapping_sub(1)] {
            if let Some(p) = self.x.c.caminhos.get(k).cloned() && !self.x.c.imagens.contains_key(&k) {
                let im = self.imagens.file(&p, 480.0, self.escala);
                self.x.c.imagens.insert(k, im);
            }
        }
        self.versao += 1;
    }

    pub fn cortes_passo(&mut self, d: i32) {
        let n = self.x.c.tira.as_ref().map_or(0, |(t, _)| t.n);
        if n == 0 { return; }
        let q = (self.x.c.quadro as i32 + d).clamp(0, n as i32 - 1) as usize;
        self.cortes_quadro(q);
    }

    /// O quadro mais perto do tempo `t`.
    pub fn cortes_ir(&mut self, t: f32) {
        let Some((tr, _)) = self.x.c.tira.as_ref() else { return };
        let q = tr.tempos.iter().enumerate()
            .min_by(|a, b| (a.1 - t).abs().total_cmp(&(b.1 - t).abs())).map_or(0, |(i, _)| i);
        self.cortes_quadro(q);
    }

    pub fn cortes_selecionar(&mut self, id: i64) {
        let Some(c) = self.x.c.corte(id).cloned() else { return };
        self.x.c.sel = if self.x.c.sel == Some(id) { None } else { Some(id) };
        self.x.c.nota = TextEdit::new(&c.nota);
        if self.x.c.video.as_ref().map(|m| m.id) == Some(c.midia) { self.cortes_ir(c.ini); }
        self.versao += 1;
    }

    /// I: com um corte escolhido, a entrada dele vai para o quadro atual; sem, marca o começo.
    pub fn cortes_entrada(&mut self) {
        let t = self.cortes_tempo();
        match self.x.c.sel {
            Some(id) => self.corte_acao("corte_editar", json!({"id": id, "ini": t})),
            None => { self.x.c.entrada = Some(t); self.versao += 1; }
        }
    }

    /// O: com um corte escolhido, ajusta a saída; com a entrada marcada, cria o corte manual.
    pub fn cortes_saida(&mut self) {
        let t = self.cortes_tempo();
        if let Some(id) = self.x.c.sel {
            self.corte_acao("corte_editar", json!({"id": id, "fim": t}));
            return;
        }
        let Some(ini) = self.x.c.entrada.take() else {
            self.aviso = Some("marque a entrada (I) antes da saída (O)".into());
            return;
        };
        let Some(m) = self.x.c.video.as_ref().map(|m| m.id) else { return };
        if (t - ini).abs() < 0.2 { self.aviso = Some("entrada e saída no mesmo ponto".into()); return; }
        self.corte_acao("corte_criar", json!({"midia": m, "ini": ini.min(t), "fim": ini.max(t)}));
    }

    /// Empurra uma borda do corte escolhido (`lado` "ini" ou "fim") em `d` segundos.
    pub fn cortes_borda(&mut self, lado: &str, d: f32) {
        let Some(c) = self.x.c.sel.and_then(|id| self.x.c.corte(id)).cloned() else { return };
        let v = if lado == "ini" { (c.ini + d).max(0.0) } else { c.fim + d };
        self.corte_acao("corte_editar", json!({"id": c.id, lado: v}));
    }

    pub fn corte_dividir_aqui(&mut self) {
        let Some(id) = self.x.c.sel else { return };
        let t = self.cortes_tempo();
        self.corte_acao("corte_dividir", json!({"id": id, "t": t}));
    }

    /// Junta o corte escolhido com o seguinte do mesmo vídeo.
    pub fn corte_juntar_proximo(&mut self) {
        let Some(c) = self.x.c.sel.and_then(|id| self.x.c.corte(id)).cloned() else { return };
        let prox = self.x.c.cortes.iter().filter(|x| x.midia == c.midia && x.ini >= c.ini && x.id != c.id)
            .min_by(|a, b| a.ini.total_cmp(&b.ini)).map(|x| x.id);
        match prox {
            Some(p) => self.corte_acao("corte_juntar", json!({"ids": [c.id, p]})),
            None => self.aviso = Some("não há corte depois deste".into()),
        }
    }

    pub fn corte_nota_salvar(&mut self) {
        let Some(id) = self.x.c.sel else { return };
        let nota = self.x.c.nota.text.trim().to_string();
        self.corte_acao("corte_editar", json!({"id": id, "nota": nota}));
    }

    /// Tag para o corte escolhido, ou para todos os cortes listados em "Selecionados".
    pub fn corte_tag(&mut self, tag: &str, remover: bool) {
        let ids: Vec<i64> = if self.x.c.ver_selecionados {
            self.x.c.dos_selecionados.iter().map(|c| c.id).collect()
        } else {
            self.x.c.sel.into_iter().collect()
        };
        if ids.is_empty() || tag.trim().is_empty() { return; }
        self.corte_acao("corte_tags", json!({"ids": ids, "tags": [tag.trim()], "remover": remover}));
    }

    pub fn corte_tag_digitada(&mut self) {
        let t = self.x.c.nova_tag.text.trim().to_string();
        self.x.c.nova_tag = TextEdit::new("");
        self.corte_tag(&t, false);
    }

    /// Aceitar ou descartar: o escolhido, ou (sem escolha) todos os propostos da lista à vista.
    pub fn corte_aceitar(&mut self, todos: bool) {
        let lista = if self.x.c.ver_selecionados { &self.x.c.dos_selecionados } else { &self.x.c.cortes };
        let ids: Vec<i64> = if todos { lista.iter().filter(|c| c.proposto).map(|c| c.id).collect() }
            else { self.x.c.sel.into_iter().collect() };
        if ids.is_empty() { self.aviso = Some("nenhum corte proposto aqui".into()); return; }
        self.corte_acao("corte_aceitar", json!({"ids": ids}));
    }

    pub fn corte_descartar(&mut self) {
        let Some(id) = self.x.c.sel.take() else { return };
        self.corte_acao("corte_descartar", json!({"ids": [id]}));
    }

    pub fn corte_copiar_tags(&mut self) {
        let Some(id) = self.x.c.sel else { return };
        self.corte_acao("corte_copiar_tags", json!({"ids": [id]}));
    }

    /// Propõe cortes pela fala (na hora) ou por cena (tarefa: decodifica o vídeo) para o vídeo
    /// aberto ou, em "Selecionados", para os vídeos selecionados na lista.
    pub fn cortes_propor(&mut self, origem: &str) {
        let ids: Vec<i64> = if self.x.c.ver_selecionados { self.x.b.cortes.selecao.iter().copied().collect() }
            else { self.x.c.video.as_ref().map(|m| m.id).into_iter().collect() };
        if ids.is_empty() { self.aviso = Some("abra ou selecione um vídeo".into()); return; }
        self.x.c.propondo = true;
        self.pedir(Pedido::CortePropor(origem.into()), "corte_propor", json!({"ids": ids, "origem": origem}));
    }

    /// O tempo do cursor: o do player, se este vídeo estiver tocando; senão o do quadro da tira.
    pub fn cortes_tempo(&self) -> f32 {
        let v = self.x.c.video.as_ref().map(|m| m.caminho.as_str()).unwrap_or("");
        if self.x.player.video.as_deref() == Some(v) { self.x.player.tempo().unwrap_or(0.0) } else { self.x.c.tempo() }
    }

    /// Espaço / ▶: toca no próprio painel o corte escolhido (ou do cursor em diante); de novo, pausa
    /// onde está e leva a tira até ali.
    pub fn cortes_tocar(&mut self) {
        let Some(v) = self.x.c.video.as_ref().map(|m| m.caminho.clone()) else { return };
        if self.x.player.video.as_deref() == Some(v.as_str()) {
            let t = self.cortes_tempo();
            self.x.player.parar();
            self.cortes_ir(t);
            return;
        }
        let (ini, fim) = match self.x.c.sel.and_then(|id| self.x.c.corte(id)) {
            Some(c) => (c.ini, Some(c.fim)),
            None => (self.x.c.tempo(), None),
        };
        let (w, h) = self.x.c.tira.as_ref().map_or((16, 9), |(t, _)| (t.largura, t.altura));
        let arq = self.base.join(&v);
        if let Err(e) = self.x.player.tocar(&arq, &v, ini, fim, w, h) {
            self.aviso = Some(format!("{e}; instale com: sudo pacman -S ffmpeg mpv"));
        }
        self.versao += 1;
    }

    /// O player chegou ao fim do trecho: a tira vai até onde ele parou.
    pub fn player_terminou(&mut self, t: f32) {
        if self.preview.is_some() { self.ir_ao_tempo_preview(t); }
        else if self.aba == crate::app::Aba::Cortes { self.cortes_ir(t); }
        self.versao += 1;
    }

    fn corte_acao(&mut self, metodo: &str, dados: Value) {
        self.pedir(Pedido::CorteAcao, metodo, dados);
    }

    /// Depois de qualquer mudança: recarrega o vídeo aberto, os selecionados e as contagens.
    fn cortes_depois(&mut self) {
        if let Some(id) = self.x.c.video.as_ref().map(|m| m.id) { self.cortes_recarregar(id); }
        if self.x.c.ver_selecionados { self.cortes_dos_selecionados(); }
        self.cortes_contar();
        self.tags_recarregar();
    }

    pub(crate) fn resposta_cortes(&mut self, p: Pedido, r: Value) {
        match p {
            Pedido::CortesDe(m) => {
                if self.x.c.video.as_ref().map(|v| v.id) == Some(m) {
                    self.x.c.cortes = serde_json::from_value(r["cortes"].clone()).unwrap_or_default();
                    if let Some(id) = self.x.c.sel && !self.x.c.cortes.iter().any(|c| c.id == id) { self.x.c.sel = None; }
                }
                if let Some(c) = r["contagem"].get(m.to_string()) {
                    self.x.c.contagem.insert(m, serde_json::from_value(c.clone()).unwrap_or_default());
                } else {
                    self.x.c.contagem.remove(&m);
                }
            }
            Pedido::CortesSelecionados => {
                self.x.c.dos_selecionados = serde_json::from_value(r["cortes"].clone()).unwrap_or_default();
            }
            Pedido::CortesContagem => {
                if let Some(o) = r["contagem"].as_object() {
                    for (k, v) in o {
                        if let Ok(id) = k.parse::<i64>() {
                            self.x.c.contagem.insert(id, serde_json::from_value(v.clone()).unwrap_or_default());
                        }
                    }
                }
            }
            Pedido::CorteTira(v) => {
                if self.x.c.video.as_ref().map(|m| &m.caminho) == Some(&v) && let Ok(t) = serde_json::from_value::<Tira>(r) {
                    let im = self.imagens.file(&t.arquivo, (t.largura * t.n as u32) as f32 / 2.0, self.escala);
                    self.x.c.tira = Some((t, im));
                }
            }
            Pedido::CorteQuadros(v) => {
                if self.x.c.video.as_ref().map(|m| &m.caminho) == Some(&v) {
                    self.x.c.caminhos = serde_json::from_value(r).unwrap_or_default();
                    let q = self.x.c.quadro;
                    self.cortes_quadro(q);
                }
            }
            Pedido::CortePropor(origem) => {
                self.x.c.propondo = false;
                if let Some(cmd) = r["cmd"].as_array() {
                    let cmd: Vec<String> = cmd.iter().filter_map(|x| x.as_str().map(String::from)).collect();
                    self.rodar_processo("cortes-cena", cmd);
                    self.aviso = Some("procurando as mudanças de cena (veja as tarefas)".into());
                } else {
                    let n = r["propostos"].as_u64().unwrap_or(0);
                    let sem = r["sem_transcricao"].as_u64().unwrap_or(0);
                    self.aviso = Some(match (n, sem) {
                        (0, s) if s > 0 => "sem transcrição: transcreva o vídeo antes (Acervo → transcrever)".to_string(),
                        (n, 0) => format!("{n} corte(s) de {origem} proposto(s)"),
                        (n, s) => format!("{n} corte(s) proposto(s); {s} vídeo(s) sem transcrição"),
                    });
                    self.cortes_depois();
                }
            }
            Pedido::CorteAcao => self.cortes_depois(),
            _ => {}
        }
        self.versao += 1;
    }
}

impl App {
    pub fn cortes_recarregar_aberto(&mut self) {
        if let Some(id) = self.x.c.video.as_ref().map(|m| m.id) { self.cortes_recarregar(id); }
    }
}
