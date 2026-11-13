//! Cores e tipografia do painel, as do mockup `claude/mockup-painel-ng3.html` (escuro).

use rune::Color;
use rune::tok::TextStyle;
use rune_kit::{Kit, PaletteSeed};

fn h(s: &str) -> Color { rune::color::hex(s).unwrap_or(Color::BLACK) }

#[derive(Clone, Copy)]
pub struct Tema {
    /// fundo da janela
    pub bg: Color,
    /// lateral, cabeçalho, painéis e cards (`--panel`)
    pub card: Color,
    /// campos, pastilhas e hover (`--panel2`)
    pub card2: Color,
    pub linha: Color,
    pub linha_forte: Color,
    pub txt: Color,
    /// texto secundário (`--muted`)
    pub dim: Color,
    /// rótulos em caixa alta e rodapés (`--faint`)
    pub fraco: Color,
    /// ação principal, seleção, aba ativa
    pub acento: Color,
    /// deu certo, ligado, revisado
    pub ok: Color,
    pub alerta: Color,
    pub erro: Color,
    /// segunda cor de destaque (RP, tipo de mídia, motion)
    pub rep: Color,
}

pub fn tema() -> Tema {
    Tema {
        bg: h("#0e1014"),
        card: h("#161920"),
        card2: h("#1c2029"),
        linha: h("#272c38"),
        linha_forte: h("#343a49"),
        txt: h("#e6e9f0"),
        dim: h("#8b93a7"),
        fraco: h("#5d6579"),
        acento: h("#6d8cff"),
        ok: h("#4cc38a"),
        alerta: h("#f0b24a"),
        erro: h("#ef6b6b"),
        rep: h("#b58cff"),
    }
}

pub fn kit() -> Kit {
    let t = tema();
    Kit::from_palette(PaletteSeed { bg: t.bg, fg: t.txt, accent: t.acento })
}

/// Cor de cada pilar (pastilhas e etiqueta do card).
pub fn cor_pilar(i: usize) -> Color {
    const C: [&str; 7] = ["#6d8cff", "#4cc38a", "#f0b24a", "#b58cff", "#ef8a6b", "#5fc2b8", "#8b93a7"];
    h(C[i % C.len()])
}

pub const TITULO: TextStyle = TextStyle::new(15.0, 600.0);
pub const NORMAL_B: TextStyle = TextStyle::new(13.0, 600.0);
pub const NORMAL: TextStyle = TextStyle::new(13.0, 400.0);
pub const NORMAL_M: TextStyle = TextStyle::new(13.0, 500.0);
pub const PEQ: TextStyle = TextStyle::new(11.5, 400.0);
pub const PEQ_M: TextStyle = TextStyle::new(11.5, 600.0);
pub const MINI: TextStyle = TextStyle::new(10.5, 600.0);
pub const NOTA: TextStyle = TextStyle::new(20.0, 600.0);
pub const MONO: TextStyle = TextStyle::new(11.5, 400.0).mono();

/// Texto sobre o azul de destaque (`#0b1020`).
pub fn tinta_acento() -> Color { h("#0b1020") }
