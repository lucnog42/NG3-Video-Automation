//! A grade de cards (posts e cortes), virtualizada: só as linhas na tela existem.

use super::comum::{botao, etiqueta, miniatura};
use super::tema::{self, Tema};
use crate::app::{Alvo, App, pilar_cor};
use crate::dados::data_br;
use rune::{Cursor, El, GrowthSpec, ScrollSpec, col, el, memo, row, spacer, text};

pub const GAP: f32 = 12.0;

pub fn colunas(largura: f32, t: crate::app::Tamanho) -> usize {
    (((largura + GAP) / (t.card() + GAP)).floor() as usize).max(1)
}

pub fn grade(s: &App, t: Tema) -> El {
    let n_linhas = s.linhas().len();
    let alto = s.tamanho.altura() + 90.0;
    let largura = s.largura.round() as u32;
    let linhas = col().key("linhas").gap(GAP).growth(GrowthSpec::default())
        .virtual_rows(n_linhas, alto,
            move |s: &App, i| format!("l{}-{}-{}", s.aba.rotulo(), largura, i).into(),
            move |s: &App, i| linha(s, i, tema::tema()))
        .virtual_rows_version((s.versao, largura, s.aba.rotulo(), s.pagina, s.tamanho as usize))
        .on_visible_rows(|s: &mut App, r| s.querer_miniaturas(r));
    col().key("grade").w_full().grow(1.0).min_h(0.0).px(16.0).py(14.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(linhas)
        .child(super::fim_de_pagina(s, t))
        .child(el().h(40.0))
        .fill(t.bg)
}

/// Uma linha justificada: cada card com a largura do formato da sua mídia e a mesma
/// altura de imagem (vertical fica estreito, horizontal fica largo; nada vira quadrado).
fn linha(s: &App, i: usize, t: Tema) -> El {
    let Some(l) = s.linhas().get(i).cloned() else { return el() };
    let pag = s.na_pagina();
    let mut r = row().w_full().gap(GAP);
    for (k, w) in l.cards {
        let Some(&j) = pag.get(k) else { continue };
        r = r.child(card_post(s, j, w, l.altura, t));
    }
    r
}

fn card_post(s: &App, j: usize, w: f32, h: f32, t: Tema) -> El {
    let p = &s.posts[j];
    let (ni, rp) = s.estado(&p.nome);
    let sel = s.selecao.contains(&p.nome);
    let capa = p.capa().map(str::to_string);
    let pronta = miniatura(s, capa.as_deref()).is_some_and(|i| i.is_ready());
    // card estreito (vídeo vertical): botões curtos, para nada vazar
    let compacto = s.tamanho == crate::app::Tamanho::Compacto || w < 200.0;
    let versao = (ni, rp, sel, pronta, p.projeto, (w.round() as u32, h.round() as u32), compacto);
    let nome = p.nome.clone();
    memo(format!("card-{}", p.nome), versao, || {
        let thumb_txt = if p.n_video > 0 { "▶ vídeo" } else { "imagem" };
        let mut thumb = el().key("thumb").w_full().h(h).corners([11.0, 11.0, 0.0, 0.0]).fill(t.card2)
            .child(super::comum::imagem_r(miniatura(s, capa.as_deref()), thumb_txt, t, [11.0, 11.0, 0.0, 0.0]));
        // selos sobre a imagem
        let mut topo = row().w_full().gap(6.0).padding(7.0).absolute(0.0, 0.0);
        topo = topo.child(etiqueta(if p.projeto { if compacto { "✓" } else { "PROJETO ✓" } } else { p.tipo.as_str() },
                                   if p.projeto { t.acento } else { t.dim }));
        if p.n_video > 0 && !compacto { topo = topo.child(etiqueta(format!("{} vídeo", p.n_video), t.rep)); }
        topo = topo.child(spacer()).child(etiqueta(format!("{:.0}", p.score), t.alerta));
        thumb = thumb.child(topo);
        let n1 = nome.clone();
        let n2 = nome.clone();
        let n3 = nome.clone();
        let marcas = row().key("marcas").w_full().gap(5.0).center_y()
            .child(botao("ni", "NI", t.erro, ni, t, move |s: &mut App| { let v = !s.estado(&n1).0; s.marcar(&n1, Some(v), None) }))
            .child(botao("rp", "RP", t.rep, rp, t, move |s: &mut App| { let v = !s.estado(&n2).1; s.marcar(&n2, None, Some(v)) }))
            .child(if !p.arquivo_projeto.is_empty() && !compacto {
                let arq = p.arquivo_projeto.clone();
                botao("abrir", "abrir", t.alerta, false, t, move |s: &mut App| s.abrir_projeto(&arq))
            } else { el() })
            .child(if w >= 300.0 {
                let pasta = p.pasta.clone();
                botao("pasta", "pasta", t.dim, false, t, move |s: &mut App| s.abrir_pasta(&pasta))
            } else { el() })
            .child(spacer())
            .child(if w < 245.0 { el() } else { text(format!("♥ {}", p.curtidas)).font(tema::PEQ).color(t.dim) })
            .child(botao("sel", match (sel, compacto) { (true, true) => "✓", (false, true) => "＋", (true, _) => "✓ selecionado", _ => "＋ selecionar" }, t.acento, sel, t,
                         move |s: &mut App| s.alternar_selecao(&n3)));
        let mut info = col().w_full().gap(7.0).px(10.0).py(9.0)
            .child(text(p.nome.clone()).font(tema::PEQ.lh(16.0)).color(t.txt).max_lines(if compacto { 1 } else { 2 }));
        if !compacto {
            info = info.child(row().w_full().gap(6.0).center_y()
                .child(etiqueta(p.pilar.clone(), tema::cor_pilar(pilar_cor(&p.pilar))))
                .child(text(data_br(&p.data)).font(tema::PEQ).color(t.dim)));
        }
        let info = info.child(marcas);
        let abrir = nome.clone();
        col().key(format!("post-{}", p.nome)).w(w).min_w(0.0).radius(11.0)
            .fill(t.card).border(if sel { 2.0 } else { 1.0 }, if sel { t.acento } else if rp { t.rep.alpha(0.5) } else { t.linha })
            .hover(t.linha_forte.alpha(0.08))
            .cursor(Cursor::Pointer)
            .child(thumb).child(info)
            // NI esmaecido por um véu, não por opacity (ver comum::imagem_r)
            .child(if ni { super::comum::veu(t.bg.alpha(0.55), 11.0) } else { el() })
            .on_press(move |s: &mut App| s.abrir_preview(Alvo::Post(abrir.clone())))
    })
}
