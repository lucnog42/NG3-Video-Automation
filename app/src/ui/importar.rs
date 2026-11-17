//! Aba "importar": a fila de revisão das pastas apontadas (com o painel de tags) e a lista das
//! pastas de qualquer disco apontadas como fontes do cliente, sem cópia.

use super::comum::{self, botao, etiqueta, pastilha, seg, seg_item};
use super::tema::{self, Tema};
use crate::app::App;
use rune::{Cursor, El, ScrollSpec, col, el, row, spacer, text};
use rune_kit::TextInput;
use serde_json::json;

/// A aba: barra com "+ Importar pasta" e a troca entre a fila de revisão e as pastas apontadas.
pub fn aba(s: &App, t: Tema, k: &rune_kit::Kit) -> El {
    let b = &s.x.b;
    let barra = row().key("importar-barra").w_full().px(22.0).py(10.0).gap(10.0).center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.linha)
        .child(row().key("importar-pasta").h(28.0).px(12.0).center_y().radius(6.0).fill(t.acento).cursor(Cursor::Pointer)
            .child(text("+ Importar pasta").font(tema::PEQ_M).color(tema::tinta_acento()))
            .on_press(|s: &mut App| s.x.b.janela_importar = true))
        .child(seg("seg-importar", vec![
            seg_item("imp-fila", "Fila de revisão", !b.ver_pastas && !s.x.o.ver_tags, t,
                     |s: &mut App| { s.x.b.ver_pastas = false; s.ver_gerenciador(false); }),
            seg_item("imp-tags", "Tags da marca", s.x.o.ver_tags, t, |s: &mut App| s.ver_gerenciador(true)),
            seg_item("imp-pastas", format!("Pastas apontadas {}", s.x.fontes.len()), b.ver_pastas, t,
                     |s: &mut App| { s.x.o.ver_tags = false; s.x.b.ver_pastas = true; }),
        ], t))
        .child(spacer())
        .child(text("nada é copiado: as pastas são só apontadas").font(tema::PEQ).color(t.fraco));
    let corpo = if b.ver_pastas { pastas(s, t) }
        else if s.x.o.ver_tags { super::tags::gerenciador(s, t, k) }
        else { super::biblioteca::fila(s, t, k) };
    col().key("importar").w_full().grow(1.0).min_h(0.0)
        .on_drop_files(|s: &mut App, arquivos, _| { s.soltar_pasta(arquivos); s.x.b.janela_importar = true; })
        .child(barra)
        .child(corpo)
}

/// Janela "Importar pasta": caminho, nome e tipo. Os arquivos novos caem na fila de revisão.
pub fn janela(s: &App, t: Tema, k: &rune_kit::Kit) -> El {
    let x = &s.x;
    let mut tipos = row().w_full().gap(6.0).flex_wrap().center_y();
    for (id, _) in &x.tipos {
        let id2 = id.clone();
        tipos = tipos.child(pastilha(format!("tipo-fonte-{id}"), id.clone(), x.novo_tipo == *id, t,
                                     move |s: &mut App| s.x.novo_tipo = id2.clone()));
    }
    let desc = x.tipos.iter().find(|(id, _)| *id == x.novo_tipo).map(|(_, d)| d.clone()).unwrap_or_default();
    let painel = col().key("janela-importar").w(560.0).gap(12.0).padding(20.0).radius(12.0).fill(t.card).border(1.0, t.linha_forte)
        .on_press(|_: &mut App| {})
        .child(row().w_full().center_y()
            .child(text("Importar pasta").font(tema::TITULO).color(t.txt))
            .child(spacer())
            .child(botao("imp-fechar", "fechar", t.dim, false, t, |s: &mut App| s.x.b.janela_importar = false)))
        .child(text("Cole o caminho ou arraste a pasta para a aba Importar. Nada é copiado nem escrito nela: cada subpasta \
                     vira tag com origem \"pasta\" e os arquivos novos entram na fila de revisão.")
            .font(tema::PEQ.lh(16.0)).color(t.dim).wrap())
        .child(comum::rotulo("pasta", t))
        .child(TextInput::new(&x.nova_raiz).placeholder("/run/media/…/Viagem Nov 2026")
            .on_edit(|s: &mut App| &mut s.x.nova_raiz).build(k).key("fonte-raiz").w_full())
        .child(comum::rotulo("nome", t))
        .child(TextInput::new(&x.novo_nome).placeholder("nome (opcional: usa o da pasta)")
            .on_edit(|s: &mut App| &mut s.x.novo_nome).build(k).key("fonte-nome").w_full())
        .child(comum::rotulo("tipo", t))
        .child(tipos)
        .child(text(desc).font(tema::PEQ).color(t.dim))
        .child(row().w_full().gap(8.0).pt(6.0)
            .child(spacer())
            .child(botao("imp-cancelar", "cancelar", t.dim, false, t, |s: &mut App| s.x.b.janela_importar = false))
            .child(row().key("fonte-add").h(28.0).px(14.0).center_y().radius(6.0)
                .fill(if x.adicionando { t.card2 } else { t.acento }).cursor(Cursor::Pointer)
                .child(text(if x.adicionando { "apontando e indexando…" } else { "importar" }).font(tema::PEQ_M)
                    .color(if x.adicionando { t.dim } else { tema::tinta_acento() }))
                .on_press(|s: &mut App| { if !s.x.adicionando { s.fonte_adicionar() } })));
    col().key("camada-importar").w_full().h_full().absolute(0.0, 0.0).fill(rune::Color::BLACK.alpha(0.45))
        .on_press(|s: &mut App| s.x.b.janela_importar = false)
        .child(spacer())
        .child(row().w_full().child(spacer()).child(painel).child(spacer()))
        .child(spacer())
}

/// As pastas apontadas (e as fontes do acervo), com ligar, reindexar, contar, transcrever e tirar.
fn pastas(s: &App, t: Tema) -> El {
    let x = &s.x;
    let mut c = col().key("importar-pastas").w_full().grow(1.0).min_h(0.0).px(22.0).py(16.0).gap(12.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(text(format!("Material de {}: pastas em qualquer disco, apontadas (nunca copiadas). O índice, as transcrições e \
            as miniaturas ficam em dados/marcas/{}/; a pasta de origem não é tocada.", s.nome_cliente(), s.marca))
            .font(tema::PEQ.lh(16.0)).color(t.dim).wrap());

    // lista
    if x.fontes.is_empty() {
        c = c.child(text("carregando as fontes…").font(tema::PEQ).color(t.dim));
    }
    for f in &x.fontes {
        let conta = x.contagens.get(&f.id);
        let (i1, i2, i3, i4, i5) = (f.id.clone(), f.id.clone(), f.id.clone(), f.id.clone(), f.id.clone());
        let ativo = f.ativo;
        let mut linha = row().w_full().gap(8.0).center_y()
            .child(text(f.nome.clone()).font(tema::NORMAL_M).color(if f.ativo { t.txt } else { t.dim }))
            .child(etiqueta(f.tipo.clone(), if f.tipo == "referencias" { t.alerta } else { t.rep }))
            .child(if f.origem == "importada" { etiqueta("importada", t.acento) } else { etiqueta("do acervo", t.dim) })
            .child(if !f.existe { etiqueta("pasta não encontrada (disco desmontado?)", t.erro) } else { el() })
            .child(spacer())
            .child(text(format!("{} no índice", f.indexados)).font(tema::PEQ).color(t.dim));
        if let Some(n) = conta {
            linha = linha.child(text(format!("· {} vídeo(s), {} foto(s), {} transcrito(s)", n.videos, n.imagens, n.transcritos))
                .font(tema::PEQ).color(t.dim));
        }
        let mut acoes = row().w_full().gap(6.0).center_y()
            .child(text(if f.raiz.is_empty() { "(acervo publicado)".to_string() } else { f.raiz.clone() })
                .font(tema::MONO).color(t.dim).truncate().grow(1.0).min_w(0.0))
            .child(botao(format!("f-at-{}", f.id), if f.ativo { "ligada" } else { "desligada" }, t.acento, f.ativo, t,
                         move |s: &mut App| s.fonte_acao(&i1, "fonte_ativar", json!({"ativo": !ativo}))))
            .child(botao(format!("f-ix-{}", f.id), "reindexar", t.txt, false, t,
                         move |s: &mut App| s.fonte_acao(&i2, "fonte_indexar", json!({}))));
        if f.origem == "importada" {
            acoes = acoes.child(botao(format!("f-ct-{}", f.id), "contar", t.dim, false, t,
                                      move |s: &mut App| s.fonte_acao(&i3, "fonte_contar", json!({}))));
            if f.tipo == "falas" {
                acoes = acoes.child(botao(format!("f-tr-{}", f.id), "transcrever o que falta", t.alerta, false, t,
                                          move |s: &mut App| s.fonte_acao(&i4, "fonte_transcrever", json!({}))));
            }
            acoes = acoes.child(botao(format!("f-rm-{}", f.id), "tirar do sistema", t.erro, false, t,
                                      move |s: &mut App| s.fonte_acao(&i5, "fonte_remover", json!({}))));
        }
        c = c.child(col().key(format!("fonte-{}", f.id)).w_full().gap(6.0).padding(10.0).radius(8.0).fill(t.card)
            .border(1.0, t.linha).child(linha).child(acoes));
    }
    c.child(text("falas: entram nos roteiros e no b-roll (depois de transcritas) · broll e fotos: só cobertura · referencias: \
        estilo e montagem de outros, nunca usadas como material. \"Tirar do sistema\" só esquece a pasta; nenhum arquivo é apagado.")
        .font(tema::PEQ.lh(16.0)).color(t.dim.alpha(0.8)).wrap())
}
