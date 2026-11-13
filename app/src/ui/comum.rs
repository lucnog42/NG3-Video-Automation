//! Pedaços repetidos: pastilha, botão pequeno, etiqueta, miniatura.

use super::tema::{self, Tema};
use crate::app::App;
use rune::{Color, Cursor, El, Image, SurfaceFit, el, row, text};

/// Pastilha de filtro (liga/desliga).
pub fn pastilha(chave: String, rotulo: String, on: bool, t: Tema, f: impl Fn(&mut App) + 'static) -> El {
    row().key(chave).h(26.0).px(10.0).gap(6.0).center_y().radius(13.0)
        .fill(if on { t.acento.alpha(0.14) } else { t.card2 })
        .border(1.0, if on { t.acento } else { t.linha })
        .hover(t.linha_forte.alpha(0.25))
        .cursor(Cursor::Pointer)
        .child(text(rotulo).font(tema::PEQ).color(if on { t.acento } else { t.txt }))
        .on_press(f)
}

/// Botão compacto do painel (marcações, ações do card).
pub fn botao(chave: impl Into<String>, rotulo: impl Into<String>, cor: Color, on: bool, t: Tema,
             f: impl Fn(&mut App) + 'static) -> El {
    row().key(chave.into()).h(24.0).px(8.0).center_y().radius(6.0)
        .fill(if on { cor.alpha(0.22) } else { t.bg.alpha(0.85) })
        .border(1.0, if on { cor } else { cor.alpha(0.35) })
        .hover(cor.alpha(0.12))
        .cursor(Cursor::Pointer)
        .child(text(rotulo.into()).font(tema::PEQ_M).color(cor))
        .on_press(f)
}

/// `botao` cujo clique precisa do `Tx` (rolar, focar, abrir link).
pub fn botao_tx(chave: impl Into<String>, rotulo: impl Into<String>, cor: Color, on: bool, t: Tema,
                f: impl Fn(&mut App, &mut rune::Tx) + 'static) -> El {
    row().key(chave.into()).h(24.0).px(8.0).center_y().radius(6.0)
        .fill(if on { cor.alpha(0.22) } else { t.bg.alpha(0.85) })
        .border(1.0, if on { cor } else { cor.alpha(0.35) })
        .hover(cor.alpha(0.12))
        .cursor(Cursor::Pointer)
        .child(text(rotulo.into()).font(tema::PEQ_M).color(cor))
        .on_press_tx(f)
}

pub fn etiqueta(rotulo: impl Into<String>, cor: Color) -> El {
    row().h(18.0).px(6.0).center_y().radius(4.0)
        .fill(cor.mix(rune::Color::BLACK, 0.82).alpha(0.92))
        .child(text(rotulo.into()).font(tema::MINI).color(cor))
}

/// Uma imagem que preenche a caixa, ou um padrão listrado enquanto carrega.
pub fn imagem(im: Option<&Image>, rotulo: &str, t: Tema) -> El { imagem_r(im, rotulo, t, [0.0; 4]) }

/// `imagem` com os cantos arredondados na própria imagem (`[sup-esq, sup-dir, inf-dir, inf-esq]`).
///
/// Arredondar pela imagem, e não com `clip_rounded` no pai: o clip vira um grupo isolado (uma
/// textura à parte) e, numa lista que rola, o Rune cria a textura com 0 px quando o elemento sai
/// da tela ("Texture with 'isolated group' label is invalid"). Pelo mesmo motivo, esmaecer com
/// uma camada por cima (`veu`) em vez de `opacity`.
pub fn imagem_r(im: Option<&Image>, rotulo: &str, t: Tema, cantos: [f32; 4]) -> El {
    match im {
        Some(i) if i.is_ready() && i.error().is_none() => el().w_full().h_full().corners(cantos).pixels_fit(i.pixels(), SurfaceFit::Fill),
        _ => el().w_full().h_full().corners(cantos).center().fill(t.card2)
            .child(text(rotulo.to_string()).font(tema::PEQ_M).color(t.dim.alpha(0.6))),
    }
}

/// Camada que esmaece o que está embaixo, sem `opacity` (ver `imagem_r`); não pega cliques.
pub fn veu(cor: Color, raio: f32) -> El {
    el().absolute(0.0, 0.0).w_full().h_full().radius(raio).fill(cor).inert()
}

/// Uma tira de quadros (n quadros lado a lado numa imagem só) como um elemento só, com uma área
/// de clique por quadro por cima; os quadros fora de `on` ficam esmaecidos por um véu.
pub fn fita(chave: &str, tira: &Image, n: usize, on: usize, altura: f32, t: Tema,
            f: impl Fn(&mut App, usize) + Clone + 'static) -> El {
    let mut alvos = row().absolute(0.0, 0.0).w_full().h(altura);
    for i in 0..n {
        let g = f.clone();
        let mut cel = el().key(format!("{chave}-{i}")).grow(1.0).h_full().hover(Color::WHITE.alpha(0.15))
            .cursor(Cursor::Pointer).on_press(move |s: &mut App| g(s, i));
        cel = if i == on { cel.border(2.0, t.txt) } else { cel.fill(Color::BLACK.alpha(0.45)) };
        alvos = alvos.child(cel);
    }
    let base = if tira.is_ready() { el().w_full().h(altura).radius(4.0).pixels(tira.pixels()) }
               else { el().w_full().h(altura).radius(4.0).fill(t.card2) };
    el().key(chave.to_string()).w_full().h(altura).child(base).child(alvos)
}

pub fn barra(frac: f32, cor: Color, t: Tema) -> El {
    el().h(5.0).w_full().radius(3.0).fill(t.card2)
        .child(el().h(5.0).w_frac(frac.clamp(0.0, 1.0)).radius(3.0).fill(cor))
}

pub fn texto_bloco(titulo: &str, corpo: &str, t: Tema) -> El {
    rune::col().w_full().gap(6.0).padding(12.0).radius(9.0).fill(t.card2).border(1.0, t.linha)
        .child(text(titulo.to_uppercase()).font(tema::MINI).color(t.dim))
        .child(text(corpo.to_string()).font(tema::NORMAL.lh(19.0)).color(t.txt).selectable(true).wrap())
}

pub fn miniatura<'a>(s: &'a App, rel: Option<&str>) -> Option<&'a Image> {
    rel.and_then(|r| s.miniaturas.get(r))
}

/// Controle segmentado do mockup (`.seg`): opções lado a lado numa caixa só.
pub fn seg(chave: impl Into<String>, itens: Vec<El>, t: Tema) -> El {
    let mut r = row().key(chave.into()).h(26.0).center_y().radius(6.0).border(1.0, t.linha_forte);
    for (i, it) in itens.into_iter().enumerate() {
        if i > 0 {
            r = r.child(el().w(1.0).h_full().fill(t.linha_forte));
        }
        r = r.child(it);
    }
    r
}

/// Uma opção de `seg`.
pub fn seg_item(chave: impl Into<String>, rotulo: impl Into<String>, on: bool, t: Tema,
                f: impl Fn(&mut App) + 'static) -> El {
    row().key(chave.into()).h_full().px(10.0).center_y().radius(5.0)
        .fill(if on { t.acento.alpha(0.14) } else { Color::TRANSPARENT })
        .hover(t.card2).cursor(Cursor::Pointer)
        .child(text(rotulo.into()).font(if on { tema::PEQ_M } else { tema::PEQ }).color(if on { t.acento } else { t.dim }))
        .on_press(f)
}

/// Rótulo pequeno em caixa alta antes de um grupo de controles (`.lbl` do mockup).
pub fn rotulo(txt: &str, t: Tema) -> El {
    text(txt.to_uppercase()).font(tema::MINI).color(t.fraco)
}
