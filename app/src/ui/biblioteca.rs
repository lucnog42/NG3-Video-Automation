//! A grade de mídia do mockup (aba Acervo e fila de revisão do Importar): cards com caixa de
//! seleção, filtros em controles segmentados e o painel de tags à direita.

use super::comum::{self, botao, etiqueta, pastilha, seg, seg_item};
use super::tema::{self, Tema};
use crate::app::App;
use crate::dados::{Midia, TagConta};
use crate::midias::{POR_PAGINA, Vista};
use rune::{Color, Cursor, El, ScrollSpec, col, el, row, spacer, text};
use rune_kit::TextInput;

const CARD_W: f32 = 200.0;
const THUMB_H: f32 = 112.0;

/// Aba Acervo: tudo junto (posts, cortes e pastas), com busca e filtro por fonte.
pub fn aba_acervo(s: &App, t: Tema, k: &rune_kit::Kit) -> El {
    let v = Vista::Acervo;
    let vm = s.vista(v);
    let r = &vm.resumo;
    let busca = TextInput::new(&vm.busca)
        .placeholder("buscar no caminho, na pasta e nas tags")
        .on_edit(|s: &mut App| &mut s.x.b.acervo.busca)
        .on_change(|s: &mut App, _| s.midia_busca(Vista::Acervo))
        .build(k).key("acervo-busca").w(300.0);

    let mut fontes = vec![seg_item("af-todas", "Todas", vm.filtros.fonte.is_empty(), t,
                                   |s: &mut App| s.midia_filtro(Vista::Acervo, |f| f.fonte.clear()))];
    for (id, n) in &r.fontes {
        let nome = s.x.fontes.iter().find(|f| &f.id == id).map(|f| f.nome.clone()).unwrap_or_else(|| id.clone());
        let id2 = id.clone();
        fontes.push(seg_item(format!("af-{id}"), format!("{nome} {n}"), vm.filtros.fonte == *id, t,
                             move |s: &mut App| { let i = id2.clone(); s.midia_filtro(Vista::Acervo, move |f| f.fonte = i) }));
    }
    let barra = row().w_full().gap(10.0).center_y().flex_wrap()
        .child(busca)
        .child(seg_tipo(s, v, t))
        .child(seg("seg-fontes", fontes, t))
        .child(spacer())
        .child(seg_passo(s, v, t, &[("", "Todos"), ("fila", "Na fila"), ("sem_tags", "Sem tag"), ("revisado", "Revisados")]))
        .child(botoes_selecao(s, v, t));
    corpo(s, v, t, k, col().w_full().gap(10.0).child(barra).child(filtro_tags(s, v, t)))
}

/// Aba Importar, vista "fila de revisão": só as pastas apontadas.
pub fn fila(s: &App, t: Tema, k: &rune_kit::Kit) -> El {
    let v = Vista::Fila;
    let vm = s.vista(v);
    let r = &vm.resumo;
    let barra = row().w_full().gap(10.0).center_y().flex_wrap()
        .child(seg_tipo(s, v, t))
        .child(spacer())
        .child(seg_passo(s, v, t, &[("fila", "Fila de revisão"), ("sem_tags", "Sem tag"), ("revisado", "Revisados"), ("", "Todos")]))
        .child(botoes_selecao(s, v, t));

    let mut imps = row().w_full().gap(6.0).center_y().flex_wrap().child(comum::rotulo("importação", t).pr(4.0))
        .child(pastilha("imp-todas".into(), format!("Todas · {}", r.passos.get("todos").copied().unwrap_or(0)),
                        vm.filtros.importacao.is_none(), t, |s: &mut App| s.midia_filtro(Vista::Fila, |f| f.importacao = None)));
    for i in r.importacoes.iter().take(8) {
        let nome = s.x.fontes.iter().find(|f| f.id == i.fonte).map(|f| f.nome.clone()).unwrap_or_else(|| i.fonte.clone());
        let id = i.id;
        imps = imps.child(pastilha(format!("imp-{id}"), format!("{nome} · {} · {}", i.n, i.quando),
                                   vm.filtros.importacao == Some(id), t,
                                   move |s: &mut App| s.midia_filtro(Vista::Fila, move |f| {
                                       f.importacao = if f.importacao == Some(id) { None } else { Some(id) } })));
    }
    let mut subs = row().w_full().gap(6.0).center_y().flex_wrap().child(comum::rotulo("subpastas", t).pr(4.0));
    for sp in r.subpastas.iter().take(14) {
        let nome = sp.nome.clone();
        subs = subs.child(pastilha(format!("sub-{}", sp.nome), format!("/{} {}", sp.nome, sp.n), vm.filtros.subpasta == sp.nome, t,
                                   move |s: &mut App| { let n = nome.clone(); s.midia_filtro(Vista::Fila, move |f| {
                                       f.subpasta = if f.subpasta == n { String::new() } else { n } }) }));
    }
    if r.subpastas.len() > 14 {
        subs = subs.child(text(format!("+{} subpastas", r.subpastas.len() - 14)).font(tema::PEQ).color(t.fraco));
    }
    subs = subs.child(text("o nome da subpasta vira tag automaticamente").font(tema::PEQ).color(t.fraco).pl(6.0));
    corpo(s, v, t, k, col().w_full().gap(10.0).child(barra).child(imps).child(subs))
}

/// Filtros em cima, grade e paginador no meio, painel de tags à direita.
fn corpo(s: &App, v: Vista, t: Tema, k: &rune_kit::Kit, filtros: El) -> El {
    let vm = s.vista(v);
    let mut grade = row().w_full().gap(12.0).flex_wrap();
    for m in &vm.itens {
        grade = grade.child(card(s, v, m, t));
    }
    let mut meio = col().key(format!("grade-midia-{v:?}")).grow(1.0).min_w(0.0).h_full().px(22.0).py(16.0).gap(14.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(filtros);
    if vm.itens.is_empty() {
        let msg = if vm.carregando || s.x.b.sincronizando { "carregando o catálogo…" }
            else if v == Vista::Fila && vm.filtros.passo == "fila" { "Nada na fila de revisão. Use \"+ Importar pasta\" para apontar uma pasta nova." }
            else { "Nenhuma mídia com esses filtros." };
        meio = meio.child(text(msg).font(tema::NORMAL).color(t.dim).pt(30.0));
    } else {
        meio = meio.child(grade).child(paginador(s, v, t));
    }
    row().key(format!("vista-{v:?}")).w_full().grow(1.0).min_h(0.0)
        .child(meio)
        .child(painel_tags(s, v, t, k))
}

fn seg_tipo(s: &App, v: Vista, t: Tema) -> El {
    let vm = s.vista(v);
    let n = |k: &str| vm.resumo.tipos.get(k).copied().unwrap_or(0);
    let item = |id: &'static str, rot: String| seg_item(format!("tp-{v:?}-{id}"), rot, vm.filtros.tipo == id, t,
                                                        move |s: &mut App| s.midia_filtro(v, |f| f.tipo = id.into()));
    seg(format!("seg-tipo-{v:?}"), vec![
        item("video", format!("Vídeos {}", n("video"))),
        item("imagem", format!("Imagens {}", n("imagem"))),
        item("", "Todos".into()),
    ], t)
}

fn seg_passo(s: &App, v: Vista, t: Tema, passos: &[(&'static str, &'static str)]) -> El {
    let vm = s.vista(v);
    let mut itens = vec![];
    for &(id, rot) in passos {
        let n = vm.resumo.passos.get(if id.is_empty() { "todos" } else { id }).copied().unwrap_or(0);
        itens.push(seg_item(format!("ps-{v:?}-{id}"), format!("{rot} {n}"), vm.filtros.passo == id, t,
                            move |s: &mut App| s.midia_filtro(v, |f| f.passo = id.into())));
    }
    seg(format!("seg-passo-{v:?}"), itens, t)
}

fn botoes_selecao(s: &App, v: Vista, t: Tema) -> El {
    let vm = s.vista(v);
    let tudo = vm.total > 0 && vm.selecao.len() >= vm.total;
    row().gap(8.0).center_y()
        .child(botao(format!("sel-tudo-{v:?}"), if tudo { "tirar seleção" } else { "selecionar tudo" }, t.txt, false, t,
                     move |s: &mut App| s.midia_selecionar_tudo(v)))
        .child(botao(format!("sinc-{v:?}"), if s.x.b.sincronizando { "procurando…" } else { "procurar novos" }, t.dim,
                     s.x.b.sincronizando, t, |s: &mut App| { if !s.x.b.sincronizando { s.midia_sincronizar() } }))
}

/// Tags ligadas no filtro (clicar numa tag do painel sem nada selecionado filtra a grade).
fn filtro_tags(s: &App, v: Vista, t: Tema) -> El {
    let vm = s.vista(v);
    if vm.filtros.tags.is_empty() { return el(); }
    let mut r = row().w_full().gap(6.0).center_y().flex_wrap().child(comum::rotulo("com as tags", t).pr(4.0));
    for slug in &vm.filtros.tags {
        let s2 = slug.clone();
        r = r.child(pastilha(format!("ft-{slug}"), format!("{} ×", nome_tag(s, slug)), true, t,
                             move |s: &mut App| { let x = s2.clone(); s.midia_filtro(v, move |f| f.tags.retain(|y| *y != x)) }));
    }
    r
}

fn nome_tag(s: &App, slug: &str) -> String {
    s.x.b.tags.todas().into_iter().find(|(sl, _, _)| *sl == slug).map(|(_, n, _)| n.to_string()).unwrap_or_else(|| slug.to_string())
}

fn card(s: &App, v: Vista, m: &Midia, t: Tema) -> El {
    let vm = s.vista(v);
    let sel = vm.selecao.contains(&m.id);
    let id = m.id;
    let marca = el().size(18.0, 18.0).radius(5.0).center()
        .fill(if sel { t.acento } else { Color::BLACK.alpha(0.35) })
        .border(1.5, if sel { t.acento } else { Color::WHITE.alpha(0.7) })
        .child(if sel { text("✓").font(tema::MINI).color(tema::tinta_acento()) } else { el() });
    let tipo = if m.tipo == "video" { "vídeo" } else { "foto" };
    let capa = el().w_full().h(THUMB_H).radius(6.0).clip_rounded(6.0).fill(t.card2)
        .child(comum::imagem(comum::miniatura(s, Some(&m.caminho)), tipo, t))
        .child(marca.absolute(8.0, 8.0))
        .child(row().absolute(CARD_W - 16.0 - 8.0 - 44.0, THUMB_H - 26.0).w(44.0).child(spacer())
            .child(etiqueta(tipo, if m.tipo == "video" { t.rep } else { t.dim })));

    let mut tags = row().w_full().gap(4.0).flex_wrap().min_h(20.0);
    let humanas: Vec<_> = m.tags.iter().filter(|x| x.origem != "pasta").collect();
    let pastas: Vec<_> = m.tags.iter().filter(|x| x.origem == "pasta").collect();
    for x in humanas.iter().chain(pastas.iter()).take(3) {
        tags = tags.child(chip_mini(&x.nome, x.origem == "pasta", t));
    }
    if m.tags.len() > 3 {
        tags = tags.child(text(format!("+{}", m.tags.len() - 3)).font(tema::PEQ).color(t.fraco));
    }
    let fonte = s.x.fontes.iter().find(|f| f.id == m.fonte).map(|f| f.nome.clone()).unwrap_or_else(|| m.fonte.clone());
    let (cor_st, st) = if m.na_fila() { (t.alerta, "na fila") } else { (t.ok, "revisado") };
    let caminho = m.caminho.clone();
    col().key(format!("m-{v:?}-{id}")).w(CARD_W).gap(6.0).padding(8.0).radius(8.0).fill(t.card)
        .border(if sel { 2.0 } else { 1.0 }, if sel { t.acento } else { t.linha })
        .hover(t.card2).cursor(Cursor::Pointer)
        .on_press(move |s: &mut App| s.midia_alternar(v, id))
        .child(capa)
        .child(text(m.nome.clone()).font(tema::PEQ_M).color(t.txt).truncate().w_full())
        .child(tags)
        .child(row().w_full().gap(6.0).center_y()
            .child(text(fonte).font(tema::PEQ).color(t.fraco).truncate().min_w(0.0).grow(1.0))
            .child(etiqueta(st, cor_st))
            .child(row().key(format!("ab-{v:?}-{id}")).h(18.0).px(5.0).center_y().radius(4.0).hover(t.linha_forte)
                .cursor(Cursor::Pointer).child(text("▶").font(tema::MINI).color(t.dim))
                .on_press(move |s: &mut App| s.midia_abrir(&caminho))))
}

fn chip_mini(nome: &str, pasta: bool, t: Tema) -> El {
    row().h(20.0).px(7.0).center_y().radius(10.0)
        .fill(if pasta { Color::TRANSPARENT } else { t.card2 })
        .border(1.0, if pasta { t.linha } else { t.linha_forte })
        .child(text(if pasta { format!("/{nome}") } else { nome.to_string() }).font(tema::PEQ)
            .color(if pasta { t.dim } else { t.txt }).truncate())
}

fn paginador(s: &App, v: Vista, t: Tema) -> El {
    let vm = s.vista(v);
    let (p, n) = (vm.pagina, vm.n_paginas());
    let ini = p * POR_PAGINA + 1;
    let fim = (ini + vm.itens.len()).saturating_sub(1);
    let mut r = row().w_full().gap(8.0).center_y().py(6.0)
        .child(text(format!("{ini}–{fim} de {}", vm.total)).font(tema::PEQ).color(t.dim));
    if n > 1 {
        r = r.child(botao(format!("pg-ant-{v:?}"), "‹ anterior", t.txt, false, t, move |s: &mut App| s.midia_pagina(v, p.saturating_sub(1))))
            .child(text(format!("página {} de {n}", p + 1)).font(tema::PEQ).color(t.dim))
            .child(botao(format!("pg-prox-{v:?}"), "próxima ›", t.txt, false, t, move |s: &mut App| s.midia_pagina(v, p + 1)));
    }
    r
}

/// O painel da direita: com seleção, aplica e tira tags e marca como revisado; sem seleção,
/// clicar numa tag filtra a grade.
fn painel_tags(s: &App, v: Vista, t: Tema, k: &rune_kit::Kit) -> El {
    let vm = s.vista(v);
    let b = &s.x.b;
    let n = vm.selecao.len();
    let carregados: Vec<&Midia> = vm.selecionados().collect();

    let mut p = col().key(format!("painel-tags-{v:?}")).w(330.0).h_full().padding(18.0).gap(12.0)
        .fill(t.card).border_sides([0.0, 0.0, 0.0, 1.0], t.linha)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(row().w_full().center_y()
            .child(text(if v == Vista::Fila { "Tags do import" } else { "Tags" }).font(tema::NORMAL_B).color(t.txt))
            .child(spacer())
            .child(if n > 0 { pastilha(format!("nsel-{v:?}"), format!("{n} selecionado{}", if n == 1 { "" } else { "s" }), true, t,
                                       move |s: &mut App| s.midia_limpar_selecao(v)) } else { el() }))
        .child(text(if n > 0 {
            "As tags valem para a mídia inteira (imagens só têm tag no arquivo). Clique numa tag para pôr em todos \
             os selecionados; clique de novo para tirar."
        } else {
            "Selecione mídias na grade para aplicar tags. Sem seleção, clicar numa tag filtra a grade."
        }).font(tema::PEQ.lh(16.0)).color(t.dim).wrap().w_full().padding(10.0).radius(6.0).fill(t.card2));

    if n > 0 {
        let digitado = b.nova_tag.text.trim().to_lowercase();
        p = p.child(comum::rotulo(&format!("aplicar a {n} selecionado{}", if n == 1 { "" } else { "s" }), t))
            .child(TextInput::new(&b.nova_tag).placeholder("tag livre ou da marca… (Enter aplica)")
                .on_edit(|s: &mut App| &mut s.x.b.nova_tag)
                .on_submit(move |s: &mut App, _| s.midia_tag_digitada(v))
                .build(k).key(format!("nova-tag-{v:?}")).w_full());
        if !digitado.is_empty() {
            let mut lista = col().w_full().radius(6.0).fill(t.card2).border(1.0, t.linha_forte).clip_rounded(6.0);
            let achadas: Vec<_> = b.tags.todas().into_iter()
                .filter(|(sl, nm, _)| sl.contains(&digitado) || nm.to_lowercase().contains(&digitado)).take(5).collect();
            for (sl, nm, origem) in &achadas {
                let sl2 = sl.to_string();
                lista = lista.child(row().key(format!("ac-{sl}")).w_full().h(30.0).px(10.0).center_y().hover(t.linha_forte)
                    .cursor(Cursor::Pointer)
                    .child(text(nm.to_string()).font(tema::PEQ).color(t.txt)).child(spacer())
                    .child(text(origem.clone()).font(tema::PEQ).color(t.fraco))
                    .on_press(move |s: &mut App| { s.midia_tag(v, &sl2, false); s.x.b.nova_tag = rune::TextEdit::new(""); }));
            }
            if !achadas.iter().any(|(_, nm, _)| nm.to_lowercase() == digitado) {
                lista = lista.child(row().key("ac-criar").w_full().h(30.0).px(10.0).center_y().hover(t.linha_forte)
                    .cursor(Cursor::Pointer)
                    .child(text(format!("+ criar tag livre \"{}\"", b.nova_tag.text.trim())).font(tema::PEQ_M).color(t.acento))
                    .child(spacer()).child(text("livre").font(tema::PEQ).color(t.fraco))
                    .on_press(move |s: &mut App| s.midia_tag_digitada(v)));
            }
            p = p.child(lista);
        }
    }

    // estado de uma tag no painel: com seleção, "todos os selecionados (da página) têm"; sem, "está no filtro"
    let ligada = |slug: &str| if n > 0 {
        !carregados.is_empty() && carregados.iter().all(|m| m.tem_tag(slug))
    } else {
        vm.filtros.tags.iter().any(|x| x == slug)
    };
    let chip = |tg: &TagConta, pasta: bool| -> El {
        let on = ligada(&tg.slug);
        let slug = tg.slug.clone();
        let rot = if pasta { format!("/{}", tg.nome) } else { tg.nome.clone() };
        let rot = if tg.n > 0 { format!("{rot} {}", tg.n) } else { rot };
        pastilha(format!("tg-{v:?}-{}", tg.slug), rot, on, t, move |s: &mut App| {
            if s.vista(v).selecao.is_empty() {
                let x = slug.clone();
                s.midia_filtro(v, move |f| { if f.tags.contains(&x) { f.tags.retain(|y| *y != x) } else { f.tags.push(x) } });
            } else {
                s.midia_tag(v, &slug, on);
            }
        })
    };

    for c in &b.tags.categorias {
        let mut chips = row().w_full().gap(6.0).flex_wrap();
        for tg in &c.tags { chips = chips.child(chip(tg, false)); }
        p = p.child(col().w_full().gap(8.0).child(comum::rotulo(&c.nome, t)).child(chips));
    }
    if !b.tags.pasta.is_empty() {
        let mut chips = row().w_full().gap(6.0).flex_wrap();
        for tg in b.tags.pasta.iter().take(30) { chips = chips.child(chip(tg, true)); }
        if b.tags.pasta.len() > 30 {
            chips = chips.child(text(format!("+{}", b.tags.pasta.len() - 30)).font(tema::PEQ).color(t.fraco));
        }
        p = p.child(col().w_full().gap(8.0).child(comum::rotulo("das subpastas (automáticas)", t)).child(chips));
    }
    if !b.tags.livres.is_empty() {
        let mut chips = row().w_full().gap(6.0).flex_wrap();
        for tg in &b.tags.livres { chips = chips.child(chip(tg, false)); }
        p = p.child(col().w_full().gap(8.0).child(comum::rotulo("livres (sem categoria)", t)).child(chips));
    }

    if n > 0 {
        let algum_na_fila = carregados.iter().any(|m| m.na_fila()) || carregados.len() < n;
        let algum_revisado = carregados.iter().any(|m| !m.na_fila());
        let mut acoes = row().w_full().gap(8.0).pt(6.0)
            .child(botao(format!("limpar-{v:?}"), "limpar seleção", t.dim, false, t, move |s: &mut App| s.midia_limpar_selecao(v)))
            .child(spacer());
        if algum_revisado && v == Vista::Acervo {
            acoes = acoes.child(botao(format!("volta-fila-{v:?}"), "voltar para a fila", t.alerta, false, t,
                                      move |s: &mut App| s.midia_revisar(v, false)));
        }
        if algum_na_fila {
            acoes = acoes.child(row().key(format!("revisar-{v:?}")).h(26.0).px(12.0).center_y().radius(6.0).fill(t.acento)
                .cursor(Cursor::Pointer)
                .child(text("marcar como revisado").font(tema::PEQ_M).color(tema::tinta_acento()))
                .on_press(move |s: &mut App| s.midia_revisar(v, true)));
        }
        p = p.child(acoes);
    }
    p
}
