//! A grade de mídia do mockup (aba Acervo e fila de revisão do Importar): cards com caixa de
//! seleção, filtros em controles segmentados e o painel de tags à direita. Três modos: todos
//! numa grade, por pasta (navegar em pastas e subpastas) e por tags (as tags viram cards).

use super::comum::{self, botao, etiqueta, pastilha, seg, seg_item};
use super::tema::{self, Tema};
use crate::app::App;
use crate::dados::{Midia, TagConta};
use crate::midias::{Modo, POR_PAGINA, Vista};
use rune::{Color, Cursor, El, ScrollSpec, col, el, row, spacer, text};
use rune_kit::TextInput;

const CARD_W: f32 = 200.0;
const THUMB_H: f32 = 112.0;

/// Aba Acervo: tudo junto (posts, cortes e pastas), com busca e filtro por fonte.
pub fn aba_acervo(s: &App, t: Tema, k: &rune_kit::Kit) -> El {
    let v = Vista::Acervo;
    let vm = s.vista(v);
    let busca = TextInput::new(&vm.busca)
        .placeholder("buscar no caminho, na pasta e nas tags")
        .on_edit(|s: &mut App| &mut s.x.b.acervo.busca)
        .on_change(|s: &mut App, _| s.midia_busca(Vista::Acervo))
        .build(k).key("acervo-busca").w(300.0);

    let barra = row().w_full().gap(10.0).center_y().flex_wrap()
        .child(busca)
        .child(seg_modo(s, v, t))
        .child(seg_tipo(s, v, t))
        .child(seg_transcricao(s, v, t))
        .child(spacer())
        .child(seg_passo(s, v, t, &[("", "Todos"), ("fila", "Na fila"), ("sem_tags", "Sem tag"), ("com_sugestoes", "Sugestões"), ("revisado", "Revisados")]))
        .child(botoes_selecao(s, v, t));
    corpo(s, v, t, k, col().w_full().gap(10.0).child(barra).child(navegador_pastas(s, v, t)).child(filtro_tags(s, v, t)))
}

/// Aba Importar, vista "fila de revisão": só as pastas apontadas.
pub fn fila(s: &App, t: Tema, k: &rune_kit::Kit) -> El {
    let v = Vista::Fila;
    let vm = s.vista(v);
    let r = &vm.resumo;
    let busca = TextInput::new(&vm.busca)
        .placeholder("buscar no caminho, na pasta e nas tags")
        .on_edit(|s: &mut App| &mut s.x.b.fila.busca)
        .on_change(|s: &mut App, _| s.midia_busca(Vista::Fila))
        .build(k).key("fila-busca").w(260.0);
    let barra = row().w_full().gap(10.0).center_y().flex_wrap()
        .child(busca)
        .child(seg_modo(s, v, t))
        .child(seg_tipo(s, v, t))
        .child(seg_transcricao(s, v, t))
        .child(spacer())
        .child(seg_passo(s, v, t, &[("fila", "Fila de revisão"), ("sem_tags", "Sem tag"), ("com_sugestoes", "Sugestões"), ("revisado", "Revisados"), ("", "Todos")]))
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
    corpo(s, v, t, k, col().w_full().gap(10.0).child(barra).child(imps).child(navegador_pastas(s, v, t))
        .child(filtro_tags(s, v, t)))
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
        .child(filtros)
        .child(if s.x.b.sincronizando { super::barra_progresso(s, "biblioteca", t) } else { el() });
    let f = &vm.filtros;
    let cards = match f.modo {
        Modo::Pasta => Some(cards_pastas(s, v, t)),
        Modo::Tags if f.tags.is_empty() && f.passo != "sem_tags" => {
            return row().key(format!("vista-{v:?}")).w_full().grow(1.0).min_h(0.0)
                .child(meio.child(cards_tags(s, v, t)))
                .child(painel_tags(s, v, t, k));
        }
        _ => None,
    };
    let sem_fonte = f.modo == Modo::Pasta && f.fonte.is_empty();
    if let Some(c) = cards { meio = meio.child(c); }
    let so_pastas = f.modo == Modo::Pasta && vm.itens.is_empty() && !vm.resumo.subpastas.is_empty();
    if sem_fonte || so_pastas {
        // na raiz do modo pasta só aparecem as fontes; numa pasta só de subpastas, só os cards
    } else if vm.itens.is_empty() {
        let msg = if vm.carregando || s.x.b.sincronizando { "carregando o catálogo…" }
            else if f.modo == Modo::Pasta { "Nenhum arquivo direto nesta pasta: abra uma subpasta acima." }
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

fn seg_modo(s: &App, v: Vista, t: Tema) -> El {
    let modo = s.vista(v).filtros.modo;
    let item = |id: &'static str, rot: &'static str, m: Modo| seg_item(format!("md-{v:?}-{id}"), rot, modo == m, t,
                                                                       move |s: &mut App| s.midia_modo(v, m));
    seg(format!("seg-modo-{v:?}"), vec![item("todos", "Todos", Modo::Todos), item("pasta", "Por pasta", Modo::Pasta),
                                         item("tags", "Por tags", Modo::Tags)], t)
}

fn nome_fonte(s: &App, id: &str) -> String {
    s.x.fontes.iter().find(|f| f.id == id).map(|f| f.nome.clone()).unwrap_or_else(|| id.to_string())
}

/// "PASTA  Todas › Fonte › Sub › Sub2": cada pedaço volta àquela pasta. No modo Todos, as
/// subpastas da pasta aberta aparecem como pastilhas (o filtro inclui as subpastas); no modo
/// Por pasta, viram cards na grade.
fn navegador_pastas(s: &App, v: Vista, t: Tema) -> El {
    let vm = s.vista(v);
    let f = &vm.filtros;
    if f.modo == Modo::Tags { return el(); }
    let crumb = |chave: String, rot: String, on: bool, fonte: String, pasta: String| -> El {
        row().key(chave).h(24.0).px(6.0).center_y().radius(5.0).hover(t.card2).cursor(Cursor::Pointer)
            .child(text(rot).font(if on { tema::PEQ_M } else { tema::PEQ }).color(if on { t.txt } else { t.acento }))
            .on_press(move |s: &mut App| s.midia_abrir_pasta(v, &fonte, &pasta))
    };
    let seta = || text("›").font(tema::PEQ).color(t.fraco);
    let mut r = row().w_full().gap(4.0).center_y().flex_wrap().child(comum::rotulo("pasta", t).pr(6.0))
        .child(crumb(format!("cr-{v:?}-raiz"), if v == Vista::Fila { "Todas as pastas".into() } else { "Todas as fontes".into() },
                     f.fonte.is_empty(), String::new(), String::new()));
    if !f.fonte.is_empty() {
        r = r.child(seta()).child(crumb(format!("cr-{v:?}-fonte"), nome_fonte(s, &f.fonte), f.subpasta.is_empty(),
                                        f.fonte.clone(), String::new()));
        let mut acc = String::new();
        let partes: Vec<&str> = f.subpasta.split('/').filter(|p| !p.is_empty()).collect();
        for (i, p) in partes.iter().enumerate() {
            if !acc.is_empty() { acc.push('/'); }
            acc.push_str(p);
            r = r.child(seta()).child(crumb(format!("cr-{v:?}-{i}"), p.to_string(), i + 1 == partes.len(), f.fonte.clone(), acc.clone()));
        }
    }
    if f.modo == Modo::Todos {
        r = r.child(el().w(1.0).h(18.0).mx(8.0).fill(t.linha_forte));
        if f.fonte.is_empty() {
            for (id, n) in &vm.resumo.fontes {
                let id2 = id.clone();
                r = r.child(pastilha(format!("pf-{v:?}-{id}"), format!("{} {n}", nome_fonte(s, id)), false, t,
                                     move |s: &mut App| s.midia_abrir_pasta(v, &id2, "")));
            }
        } else {
            for sp in vm.resumo.subpastas.iter().take(16) {
                let alvo = if f.subpasta.is_empty() { sp.nome.clone() } else { format!("{}/{}", f.subpasta, sp.nome) };
                let fonte = f.fonte.clone();
                r = r.child(pastilha(format!("ps-{v:?}-{}", sp.nome), format!("/{} {}", sp.nome, sp.n), false, t,
                                     move |s: &mut App| s.midia_abrir_pasta(v, &fonte, &alvo)));
            }
            if vm.resumo.subpastas.len() > 16 {
                r = r.child(text(format!("+{} (use Por pasta)", vm.resumo.subpastas.len() - 16)).font(tema::PEQ).color(t.fraco));
            }
        }
    }
    r
}

/// Um card de pasta ou de tag: nome grande e quantas mídias tem dentro.
fn card_grupo(chave: String, icone: &str, nome: String, n: usize, t: Tema, f: impl Fn(&mut App) + 'static) -> El {
    row().key(chave).w(CARD_W).h(64.0).px(12.0).gap(10.0).center_y().radius(8.0).fill(t.card).border(1.0, t.linha)
        .hover(t.card2).cursor(Cursor::Pointer)
        .child(el().size(30.0, 30.0).radius(6.0).center().fill(t.acento.alpha(0.14))
            .child(text(icone.to_string()).font(tema::NORMAL_B).color(t.acento)))
        .child(col().grow(1.0).min_w(0.0).gap(2.0)
            .child(text(nome).font(tema::PEQ_M).color(t.txt).truncate().w_full())
            .child(text(format!("{n} mídia{}", if n == 1 { "" } else { "s" })).font(tema::PEQ).color(t.dim)))
        .on_press(f)
}

/// Modo Por pasta: as fontes (na raiz) ou as subpastas da pasta aberta, como cards.
fn cards_pastas(s: &App, v: Vista, t: Tema) -> El {
    let vm = s.vista(v);
    let f = &vm.filtros;
    let mut g = row().w_full().gap(12.0).flex_wrap();
    if f.fonte.is_empty() {
        for (id, n) in &vm.resumo.fontes {
            let id2 = id.clone();
            g = g.child(card_grupo(format!("cf-{v:?}-{id}"), "▤", nome_fonte(s, id), *n, t,
                                   move |s: &mut App| s.midia_abrir_pasta(v, &id2, "")));
        }
    } else {
        for sp in &vm.resumo.subpastas {
            let alvo = if f.subpasta.is_empty() { sp.nome.clone() } else { format!("{}/{}", f.subpasta, sp.nome) };
            let fonte = f.fonte.clone();
            g = g.child(card_grupo(format!("cp-{v:?}-{}", sp.nome), "▸", sp.nome.clone(), sp.n, t,
                                   move |s: &mut App| s.midia_abrir_pasta(v, &fonte, &alvo)));
        }
    }
    g
}

/// Modo Por tags: cada tag da marca (por categoria), as livres e "sem tag" viram cards.
fn cards_tags(s: &App, v: Vista, t: Tema) -> El {
    let vm = s.vista(v);
    let b = &s.x.b;
    let abrir = |slug: String| move |s: &mut App| { let x = slug.clone(); s.midia_filtro(v, move |f| f.tags = vec![x]) };
    let sem = vm.resumo.passos.get("sem_tags").copied().unwrap_or(0);
    let mut c = col().w_full().gap(16.0)
        .child(row().w_full().gap(12.0).flex_wrap()
            .child(card_grupo(format!("ct-{v:?}-sem"), "∅", "Sem tag".into(), sem, t,
                              move |s: &mut App| s.midia_filtro(v, |f| f.passo = "sem_tags".into()))));
    let grupo = |titulo: &str, tags: &[TagConta], chave: &str| -> El {
        let mut g = row().w_full().gap(12.0).flex_wrap();
        for tg in tags.iter().filter(|x| x.n > 0) {
            g = g.child(card_grupo(format!("ct-{v:?}-{chave}-{}", tg.slug), "#", tg.nome.clone(), tg.n, t, abrir(tg.slug.clone())));
        }
        col().w_full().gap(8.0).child(comum::rotulo(titulo, t)).child(g)
    };
    let mut vazias = vec![];
    for cat in &b.tags.categorias {
        if cat.tags.iter().any(|x| x.n > 0) {
            c = c.child(grupo(&cat.nome, &cat.tags, &cat.slug));
        } else {
            vazias.push(cat.nome.clone());
        }
    }
    if b.tags.livres.iter().any(|x| x.n > 0) { c = c.child(grupo("livres", &b.tags.livres, "livres")); }
    if b.tags.pasta.iter().any(|x| x.n > 0) {
        let top: Vec<TagConta> = b.tags.pasta.iter().take(40).cloned().collect();
        c = c.child(grupo("das subpastas", &top, "pasta"));
    }
    if !vazias.is_empty() {
        c = c.child(text(format!("Ainda sem mídias: {}.", vazias.join(", "))).font(tema::PEQ).color(t.fraco));
    }
    c
}

/// Vídeos transcritos ou sem transcrição (escolher um deles mostra só vídeos).
fn seg_transcricao(s: &App, v: Vista, t: Tema) -> El {
    let vm = s.vista(v);
    let n = |k: &str| vm.resumo.transcricao.get(k).copied().unwrap_or(0);
    let item = |id: &'static str, rot: String| seg_item(format!("tr-{v:?}-{id}"), rot, vm.filtros.transcricao == id, t,
                                                        move |s: &mut App| s.midia_filtro(v, |f| f.transcricao = id.into()));
    seg(format!("seg-transc-{v:?}"), vec![
        item("sim", format!("Transcritos {}", n("sim"))),
        item("nao", format!("Sem transcrição {}", n("nao"))),
        item("", "Qualquer".into()),
    ], t)
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
    let dentro_sem_tag = vm.filtros.modo == Modo::Tags && vm.filtros.passo == "sem_tags";
    if vm.filtros.tags.is_empty() && !dentro_sem_tag { return el(); }
    let mut r = row().w_full().gap(6.0).center_y().flex_wrap();
    if vm.filtros.modo == Modo::Tags {
        r = r.child(botao(format!("volta-tags-{v:?}"), "‹ todas as tags", t.acento, false, t,
                          move |s: &mut App| s.midia_filtro(v, |f| { f.tags.clear(); if f.passo == "sem_tags" { f.passo.clear() } })));
        if dentro_sem_tag { r = r.child(text("Sem tag").font(tema::NORMAL_B).color(t.txt)); }
    }
    if !vm.filtros.tags.is_empty() { r = r.child(comum::rotulo("com as tags", t).pr(4.0)); }
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
    let capa = el().w_full().h(THUMB_H).radius(6.0).fill(t.card2)
        .child(comum::imagem_r(comum::miniatura(s, Some(&m.caminho)), tipo, t, [6.0; 4]))
        .child(marca.absolute(8.0, 8.0))
        .child(if m.tipo == "video" {
            row().absolute(8.0, THUMB_H - 26.0).child(if m.transcrito { etiqueta("transcrito", t.ok) }
                                                     else { etiqueta("sem transcrição", t.dim) })
        } else { el() })
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
    for sg in m.sugestoes.iter().take(2) {
        tags = tags.child(row().h(20.0).px(7.0).center_y().radius(10.0).fill(t.rep.alpha(0.12)).border(1.0, t.rep.alpha(0.6))
            .child(text(sg.nome.clone()).font(tema::PEQ).color(t.rep).truncate()));
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
            let mut lista = col().w_full().radius(6.0).fill(t.card2).border(1.0, t.linha_forte);
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

    if n > 0 {
        p = p.child(sugestoes_ia(s, v, t)).child(metadados(s, v, t)).child(renomear(s, v, t, k)).child(transcrever(s, v, t));
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

/// Renomear no catálogo (o arquivo no disco não muda): um selecionado recebe um nome; vários,
/// um padrão com {n}, {nome}, {pasta} e {data}, com a prévia dos primeiros.
fn renomear(s: &App, v: Vista, t: Tema, k: &rune_kit::Kit) -> El {
    let vm = s.vista(v);
    let b = &s.x.b;
    let n = vm.selecao.len();
    let mut c = col().w_full().gap(8.0).pt(4.0);
    if n == 1 {
        let m = vm.selecionados().next();
        c = c.child(comum::rotulo("nome no catálogo", t))
            .child(row().w_full().gap(6.0).center_y()
                .child(TextInput::new(&b.nome).placeholder("nome novo")
                    .on_edit(|s: &mut App| &mut s.x.b.nome)
                    .on_submit(move |s: &mut App, _| s.midia_renomear(v, false))
                    .build(k).key(format!("nome-{v:?}")).grow(1.0).min_w(0.0))
                .child(botao(format!("ren-{v:?}"), "renomear", t.acento, false, t, move |s: &mut App| s.midia_renomear(v, false))));
        if let Some(m) = m && m.renomeado {
            c = c.child(row().w_full().gap(6.0).center_y()
                .child(text(format!("arquivo: {}", m.original)).font(tema::PEQ).color(t.fraco).truncate().grow(1.0).min_w(0.0))
                .child(botao(format!("orig-{v:?}"), "nome original", t.dim, false, t, move |s: &mut App| s.midia_renomear(v, true))));
        }
    } else {
        let ids = s.ids_em_ordem(v);
        let largura = (ids.len().to_string().len()).max(2);
        let mut previa = col().w_full().gap(2.0);
        for (i, id) in ids.iter().take(3).enumerate() {
            if let Some(m) = vm.itens.iter().find(|m| m.id == *id) {
                previa = previa.child(row().w_full().gap(6.0)
                    .child(text(m.original.clone()).font(tema::PEQ).color(t.fraco).truncate().w(120.0))
                    .child(text("→").font(tema::PEQ).color(t.fraco))
                    .child(text(formatar(&b.padrao.text, m, i + 1, largura)).font(tema::PEQ_M).color(t.txt).truncate().grow(1.0).min_w(0.0)));
            }
        }
        if ids.len() > 3 { previa = previa.child(text(format!("… e mais {}", ids.len() - 3)).font(tema::PEQ).color(t.fraco)); }
        c = c.child(comum::rotulo(&format!("renomear {n} em lote"), t))
            .child(row().w_full().gap(6.0).center_y()
                .child(TextInput::new(&b.padrao).placeholder("{pasta}_{n}")
                    .on_edit(|s: &mut App| &mut s.x.b.padrao)
                    .on_submit(move |s: &mut App, _| s.midia_renomear(v, false))
                    .build(k).key(format!("padrao-{v:?}")).grow(1.0).min_w(0.0))
                .child(botao(format!("ren-lote-{v:?}"), "renomear", t.acento, false, t, move |s: &mut App| s.midia_renomear(v, false))))
            .child(text("{n} número · {nome} nome do arquivo · {pasta} pasta · {data} data").font(tema::PEQ).color(t.fraco).wrap())
            .child(previa)
            .child(botao(format!("orig-lote-{v:?}"), "voltar todos ao nome original", t.dim, false, t,
                         move |s: &mut App| s.midia_renomear(v, true)));
    }
    c.child(text("Muda só o nome no painel e na busca; o arquivo no disco continua igual.").font(tema::PEQ).color(t.fraco).wrap())
}

/// A mesma regra do `biblioteca._formatar` (prévia do lote): sem extensão no padrão, fica a do arquivo.
fn formatar(padrao: &str, m: &Midia, n: usize, largura: usize) -> String {
    let (base, ext) = match m.original.rfind('.') { Some(i) => (&m.original[..i], &m.original[i..]), None => (m.original.as_str(), "") };
    let pasta = m.subpasta.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    let nome = padrao.trim().replace("{n}", &format!("{n:0largura$}")).replace("{nome}", base)
        .replace("{pasta}", pasta).replace("{data}", &m.data).replace('/', "-");
    let nome = nome.split_whitespace().collect::<Vec<_>>().join(" ");
    let tem_ext = nome.rfind('.').is_some_and(|i| i > 0 && i < nome.len() - 1);
    if nome.is_empty() || tem_ext { nome } else { format!("{nome}{ext}") }
}

/// A tarefa de transcrever os vídeos selecionados: separar a voz da música (automático, sempre
/// ou nunca) e refazer os que já têm transcrição. Vai para a fila de tarefas.
fn transcrever(s: &App, v: Vista, t: Tema) -> El {
    let vm = s.vista(v);
    let b = &s.x.b;
    let carregados: Vec<&Midia> = vm.selecionados().collect();
    let videos = carregados.iter().filter(|m| m.tipo == "video").count();
    let ja = carregados.iter().filter(|m| m.tipo == "video" && m.transcrito).count();
    let fora_da_pagina = vm.selecao.len() > carregados.len();
    if videos == 0 && !fora_da_pagina {
        return col().w_full().gap(6.0).pt(4.0).child(comum::rotulo("transcrever", t))
            .child(text("Nenhum vídeo na seleção: imagens não são transcritas.").font(tema::PEQ).color(t.fraco).wrap());
    }
    let sep = |id: &'static str, rot: &'static str| seg_item(format!("sep-{v:?}-{id}"), rot, b.separar == id, t,
                                                             move |s: &mut App| s.x.b.separar = id.into());
    let resumo = if fora_da_pagina { "os vídeos selecionados (inclusive de outras páginas)".to_string() }
        else if ja > 0 && !b.refazer { format!("{} vídeo(s); {ja} já transcrito(s) serão pulados", videos) }
        else { format!("{videos} vídeo(s)") };
    let rotulo_botao = if fora_da_pagina { "transcrever os vídeos selecionados".to_string() }
        else { format!("transcrever {}", if b.refazer { videos } else { videos - ja }) };
    col().w_full().gap(8.0).pt(4.0)
        .child(comum::rotulo("transcrever (gera as legendas)", t))
        .child(row().w_full().gap(8.0).center_y()
            .child(text("separar a música").font(tema::PEQ).color(t.dim))
            .child(seg(format!("seg-sep-{v:?}"), vec![sep("auto", "se houver"), sep("sempre", "sempre"), sep("nunca", "nunca")], t)))
        .child(pastilha(format!("refazer-{v:?}"), "refazer os já transcritos".into(), b.refazer, t,
                        |s: &mut App| s.x.b.refazer = !s.x.b.refazer))
        .child(text(resumo).font(tema::PEQ).color(t.fraco).wrap())
        .child(row().w_full().child(spacer())
            .child(row().key(format!("transcrever-{v:?}")).h(26.0).px(12.0).center_y().radius(6.0).fill(t.acento)
                .cursor(Cursor::Pointer)
                .child(text(rotulo_botao).font(tema::PEQ_M).color(tema::tinta_acento()))
                .on_press(move |s: &mut App| s.midia_transcrever(v))))
        .child(text("\"Se houver\" mede a música nas pausas da fala e só separa quando ela aparece. A tarefa roda \
                     um vídeo por vez na GPU e aparece em tarefas; pode fechar o painel, que ela continua.").font(tema::PEQ).color(t.fraco).wrap())
}

/// As sugestões da IA de imagem nos selecionados (somadas), com aceitar/recusar por tag ou todas,
/// e o botão que roda o reconhecimento (metadados + GPS + IA) como tarefa.
fn sugestoes_ia(s: &App, v: Vista, t: Tema) -> El {
    let vm = s.vista(v);
    let mut conta: Vec<(String, String, usize)> = vec![];
    for m in vm.selecionados() {
        for sg in &m.sugestoes {
            match conta.iter_mut().find(|x| x.0 == sg.slug) {
                Some(x) => x.2 += 1,
                None => conta.push((sg.slug.clone(), sg.nome.clone(), 1)),
            }
        }
    }
    conta.sort_by(|a, b| b.2.cmp(&a.2));
    let mut c = col().w_full().gap(8.0).pt(4.0).child(comum::rotulo("sugestões da IA de imagem", t));
    if conta.is_empty() {
        c = c.child(text("Nenhuma sugestão pendente nos selecionados.").font(tema::PEQ).color(t.fraco));
    } else {
        let mut chips = row().w_full().gap(6.0).flex_wrap();
        for (slug, nome, n) in &conta {
            let (sl1, sl2) = (slug.clone(), slug.clone());
            chips = chips.child(row().key(format!("sg-{v:?}-{slug}")).h(26.0).gap(0.0).center_y().radius(13.0)
                .border(1.0, t.rep.alpha(0.6)).fill(t.rep.alpha(0.10))
                .child(row().key(format!("sga-{v:?}-{slug}")).h_full().pl(10.0).pr(6.0).center_y().cursor(Cursor::Pointer)
                    .hover(t.rep.alpha(0.18))
                    .child(text(format!("{nome} {n} ✓")).font(tema::PEQ).color(t.rep))
                    .on_press(move |s: &mut App| s.midia_sugestao(v, true, Some(&sl1))))
                .child(row().key(format!("sgr-{v:?}-{slug}")).h_full().pl(4.0).pr(9.0).center_y().cursor(Cursor::Pointer)
                    .hover(t.erro.alpha(0.18))
                    .child(text("×").font(tema::PEQ_M).color(t.erro))
                    .on_press(move |s: &mut App| s.midia_sugestao(v, false, Some(&sl2)))));
        }
        c = c.child(chips).child(row().w_full().gap(6.0)
            .child(botao(format!("sg-todas-{v:?}"), "aceitar todas", t.ok, false, t, move |s: &mut App| s.midia_sugestao(v, true, None)))
            .child(botao(format!("sg-nenhuma-{v:?}"), "recusar todas", t.erro, false, t, move |s: &mut App| s.midia_sugestao(v, false, None))));
    }
    c.child(row().w_full().gap(8.0).center_y()
            .child(text("Lê data, câmera e GPS (GPS perto de um lugar da marca vira tag) e a IA sugere lugar, momento e plano.")
                .font(tema::PEQ).color(t.fraco).wrap().grow(1.0).min_w(0.0))
            .child(botao(format!("reconhecer-{v:?}"), "reconhecer", t.rep, false, t, move |s: &mut App| s.midia_reconhecer(v))))
}

/// Os metadados de um selecionado (data, câmera, GPS, rotação), quando já foram lidos.
fn metadados(s: &App, v: Vista, t: Tema) -> El {
    let vm = s.vista(v);
    if vm.selecao.len() != 1 { return el(); }
    let Some(m) = vm.selecionados().next() else { return el() };
    if m.meta.is_empty() { return el(); }
    let g = |k: &str| m.meta.get(k).cloned().unwrap_or_default();
    let mut linhas = vec![];
    if !g("data").is_empty() { linhas.push(format!("data: {}", g("data"))); }
    if !g("camera").is_empty() { linhas.push(format!("câmera: {}", g("camera"))); }
    if !g("lat").is_empty() { linhas.push(format!("GPS: {}, {}", g("lat"), g("lon"))); }
    if !g("largura").is_empty() { linhas.push(format!("{} × {}{}", g("largura"), g("altura"),
        if g("rotacao").is_empty() { String::new() } else { format!(" · girado {}", g("rotacao")) })); }
    if !g("duracao").is_empty() { linhas.push(format!("duração: {:.1} s", g("duracao").parse::<f32>().unwrap_or(0.0))); }
    col().w_full().gap(4.0).pt(4.0).child(comum::rotulo("metadados do arquivo", t))
        .child(text(linhas.join("\n")).font(tema::PEQ.lh(17.0)).color(t.dim).selectable(true).wrap())
}
