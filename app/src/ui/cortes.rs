//! Aba Cortes (mockup "Preview e cortes"): vídeos à esquerda; no centro o quadro, a tira e a
//! linha do tempo com os cortes (fala, cena, manual; claro = proposto); à direita os cortes do
//! vídeo aberto ou dos vídeos selecionados. Cortes são anotações: o arquivo não muda.

use super::comum::{self, botao, etiqueta, pastilha, seg, seg_item};
use super::tema::{self, Tema};
use crate::app::App;
use crate::cortes::Corte;
use crate::dados::Midia;
use crate::midias::{POR_PAGINA, Vista};
use rune::{Color, Cursor, El, ScrollSpec, SurfaceFit, col, el, row, spacer, text};
use rune_kit::TextInput;

const LISTA_W: f32 = 300.0;
const LADO_W: f32 = 370.0;

fn cor_origem(o: &str, t: Tema) -> Color {
    match o { "fala" => t.acento, "cena" => t.ok, _ => t.alerta }
}

/// mm:ss.d
fn tempo(s: f32) -> String {
    let s = s.max(0.0);
    format!("{:02}:{:04.1}", (s / 60.0) as u32, s % 60.0)
}

pub fn aba(s: &App, t: Tema, k: &rune_kit::Kit, largura_tira: Option<f32>) -> El {
    row().key("aba-cortes").w_full().grow(1.0).min_h(0.0)
        .child(lista(s, t, k))
        .child(centro(s, t, largura_tira))
        .child(lado(s, t, k))
}

// ------------------------------------------------------------ lista de vídeos
fn lista(s: &App, t: Tema, k: &rune_kit::Kit) -> El {
    let v = Vista::Cortes;
    let vm = s.vista(v);
    let n_sel = vm.selecao.len();
    let busca = TextInput::new(&vm.busca).placeholder("buscar vídeo…")
        .on_edit(|s: &mut App| &mut s.x.b.cortes.busca)
        .on_change(|s: &mut App, _| s.midia_busca(Vista::Cortes))
        .build(k).key("cortes-busca").w_full();
    let f = &vm.filtros;
    let tr = |id: &'static str, rot: String| seg_item(format!("ctr-{id}"), rot, f.transcricao == id, t,
                                                      move |s: &mut App| s.midia_filtro(Vista::Cortes, |f| f.transcricao = id.into()));
    let n = |x: &str| vm.resumo.transcricao.get(x).copied().unwrap_or(0);
    let topo = col().w_full().gap(10.0).padding(14.0).border_sides([0.0, 0.0, 1.0, 0.0], t.linha)
        .child(row().w_full().gap(8.0).center_y()
            .child(text("Vídeos").font(tema::NORMAL_B).color(t.txt))
            .child(etiqueta(vm.total.to_string(), t.dim))
            .child(spacer())
            .child(botao("cortes-sel-tudo", if vm.total > 0 && n_sel >= vm.total { "tirar seleção" } else { "selecionar todos" },
                         t.txt, false, t, |s: &mut App| s.midia_selecionar_tudo(Vista::Cortes))))
        .child(busca)
        .child(seg("seg-cortes-tr", vec![tr("sim", format!("Transcritos {}", n("sim"))), tr("nao", format!("Sem {}", n("nao"))),
                                         tr("sem_fala", format!("Sem fala {}", n("sem_fala"))), tr("", "Todos".into())], t));

    let aberto = s.x.c.video.as_ref().map(|m| m.id);
    let mut itens = col().key("cortes-vlist").w_full().grow(1.0).min_h(0.0).gap(2.0).padding(8.0)
        .scroll(ScrollSpec { stick_bottom: false });
    if vm.itens.is_empty() {
        itens = itens.child(text(if vm.carregando { "carregando…" } else { "nenhum vídeo com esse filtro" })
            .font(tema::PEQ).color(t.dim).padding(8.0));
    }
    for m in &vm.itens {
        itens = itens.child(linha_video(s, m, aberto == Some(m.id), vm.selecao.contains(&m.id), t));
    }
    if vm.n_paginas() > 1 {
        let p = vm.pagina;
        itens = itens.child(row().w_full().gap(6.0).center_y().pt(8.0)
            .child(botao("cpg-ant", "‹", t.txt, false, t, move |s: &mut App| s.midia_pagina(Vista::Cortes, p.saturating_sub(1))))
            .child(text(format!("{}–{} de {}", p * POR_PAGINA + 1, p * POR_PAGINA + vm.itens.len(), vm.total))
                .font(tema::PEQ).color(t.dim))
            .child(botao("cpg-prox", "›", t.txt, false, t, move |s: &mut App| s.midia_pagina(Vista::Cortes, p + 1))));
    }
    let n_cortes: usize = vm.selecao.iter().filter_map(|id| s.x.c.contagem.get(id)).map(|c| c.cortes).sum();
    let barra = col().w_full().gap(8.0).padding(12.0).border_sides([1.0, 0.0, 0.0, 0.0], t.linha)
        .child(row().w_full().gap(6.0).center_y()
            .child(text(format!("{n_sel} selecionado{}", if n_sel == 1 { "" } else { "s" })).font(tema::PEQ_M).color(t.txt))
            .child(text(format!("· {n_cortes} cortes")).font(tema::PEQ).color(t.dim)))
        .child(row().w_full().gap(6.0)
            .child(botao("cortes-ver-sel", "cortes dos selecionados", t.acento, s.x.c.ver_selecionados, t,
                         |s: &mut App| s.cortes_dos_selecionados()))
            .child(botao("cortes-limpar-sel", "limpar", t.dim, false, t, |s: &mut App| s.midia_limpar_selecao(Vista::Cortes))));
    col().key("cortes-lista").w(LISTA_W).h_full().fill(t.card).border_sides([0.0, 1.0, 0.0, 0.0], t.linha)
        .child(topo).child(itens).child(barra)
}

/// A pasta do arquivo (nos posts, a do post: "01.mp4" sozinho não diz nada).
fn pasta_de(m: &Midia) -> String {
    let p = m.subpasta.trim_end_matches('/');
    let p = p.strip_suffix("/videos").unwrap_or(p);
    p.rsplit('/').next().unwrap_or(p).to_string()
}

fn linha_video(s: &App, m: &Midia, aberto: bool, sel: bool, t: Tema) -> El {
    let id = m.id;
    let conta = s.x.c.contagem.get(&id).copied().unwrap_or_default();
    let meta = if conta.cortes == 0 { "sem cortes".to_string() }
        else if conta.propostos > 0 { format!("{} cortes · {} propostos", conta.cortes, conta.propostos) }
        else { format!("{} cortes", conta.cortes) };
    let caixa = row().key(format!("cchk-{id}")).size(18.0, 18.0).radius(5.0).center()
        .fill(if sel { t.acento } else { Color::TRANSPARENT }).border(1.5, if sel { t.acento } else { t.linha_forte })
        .cursor(Cursor::Pointer)
        .child(if sel { text("✓").font(tema::MINI).color(tema::tinta_acento()) } else { el() })
        .on_press(move |s: &mut App| s.midia_alternar(Vista::Cortes, id));
    let m2 = m.clone();
    row().key(format!("cv-{id}")).w_full().h(62.0).px(8.0).gap(8.0).center_y().radius(6.0)
        .fill(if aberto { t.acento.alpha(0.14) } else { Color::TRANSPARENT })
        .hover(t.card2).cursor(Cursor::Pointer)
        .on_press(move |s: &mut App| s.cortes_abrir(m2.clone()))
        .child(caixa)
        .child(el().size(64.0, 36.0).child(comum::imagem_r(comum::miniatura(s, Some(&m.caminho)), "", t, [4.0; 4])))
        .child(col().grow(1.0).min_w(0.0).gap(2.0)
            .child(text(m.nome.clone()).font(if aberto { tema::PEQ_M } else { tema::PEQ }).color(if aberto { t.acento } else { t.txt })
                .truncate().w_full())
            .child(text(pasta_de(m)).font(tema::PEQ).color(t.fraco).truncate().w_full())
            .child(row().gap(6.0).center_y()
                .child(text(meta).font(tema::PEQ).color(if conta.propostos > 0 { t.alerta } else { t.fraco }))
                .child(if m.transcrito { text("· transcrito").font(tema::PEQ).color(t.ok) } else { el() })))
}

// ------------------------------------------------------------ centro
fn centro(s: &App, t: Tema, largura: Option<f32>) -> El {
    let c = &s.x.c;
    let base = col().key("cortes-centro").grow(1.0).min_w(0.0).h_full().px(22.0).py(16.0).gap(12.0)
        .scroll(ScrollSpec { stick_bottom: false });
    let Some(m) = c.video.as_ref() else {
        return base.center().child(text("Escolha um vídeo na lista à esquerda.\n\nI e O marcam entrada e saída de um corte \
            novo; ← → andam pela tira de quadros.").font(tema::NORMAL.lh(20.0)).color(t.dim).max_w(420.0));
    };
    let caminho = m.caminho.clone();
    let cab = row().w_full().gap(8.0).center_y()
        .child(text(m.nome.clone()).font(tema::NORMAL_B).color(t.txt))
        .child(text(pasta_de(m)).font(tema::PEQ).color(t.dim).truncate().min_w(0.0))
        .child(if m.transcrito { etiqueta("transcrito", t.ok) } else if m.sem_fala { etiqueta("sem fala", t.fraco) }
               else { etiqueta("sem transcrição", t.dim) })
        .child(etiqueta(if m.na_fila() { "na fila" } else { "revisado" }, if m.na_fila() { t.alerta } else { t.ok }))
        .child(spacer())
        .child(botao("cortes-abrir-arq", "abrir arquivo", t.dim, false, t, move |s: &mut App| s.midia_abrir(&caminho)));
    let mut tags_video = row().w_full().gap(6.0).center_y().flex_wrap().child(comum::rotulo("vídeo inteiro", t).pr(4.0));
    if m.tags.is_empty() { tags_video = tags_video.child(text("sem tags (aplique no Acervo)").font(tema::PEQ).color(t.fraco)); }
    for tg in &m.tags {
        tags_video = tags_video.child(etiqueta(if tg.origem == "pasta" { format!("/{}", tg.nome) } else { tg.nome.clone() }, t.dim));
    }

    // quadro grande: o escolhido, ou o vizinho pronto mais perto enquanto decodifica
    let q = c.quadro;
    let pronto = |k: usize| c.imagens.get(&k).filter(|i| i.is_ready());
    let grande = pronto(q).or_else(|| (1..8).find_map(|d| pronto(q.saturating_sub(d)).or_else(|| pronto(q + d))))
        .or_else(|| s.miniaturas.get(&m.caminho));
    let tocando = s.x.player.video.as_deref() == Some(m.caminho.as_str());
    let quadro = if s.x.player.mostrando(&m.caminho) {
        el().w_full().h(340.0).radius(9.0).fill(Color::BLACK)
            .child(el().w_full().h_full().radius(9.0).pixels_fit(&s.x.player.pixels, SurfaceFit::Fit))
    } else { match grande {
        Some(im) if im.is_ready() => el().w_full().h(340.0).radius(9.0).fill(Color::BLACK)
            .child(el().w_full().h_full().radius(9.0).pixels_fit(im.pixels(), SurfaceFit::Fit)),
        _ => el().w_full().h(340.0).radius(9.0).fill(t.card2).center().child(text("carregando quadros…").font(tema::PEQ).color(t.dim)),
    } };
    let n = c.tira.as_ref().map_or(0, |(tr, _)| tr.n);
    let marcando = c.entrada.is_some() && c.sel.is_none();
    let rot_i = if c.sel.is_some() { "I  entrada do corte aqui" } else { "I  entrada" };
    let rot_o = if c.sel.is_some() { "O  saída do corte aqui" } else if marcando { "O  saída (cria o corte)" } else { "O  saída" };
    let transporte = row().w_full().gap(6.0).center_y().flex_wrap()
        .child(botao("ct-ini", "⏮", t.txt, false, t, |s: &mut App| s.cortes_quadro(0)))
        .child(botao("ct-ant", "◀ quadro", t.txt, false, t, |s: &mut App| s.cortes_passo(-1)))
        .child(row().key("ct-tocar").h(24.0).px(10.0).center_y().radius(6.0).fill(t.acento).cursor(Cursor::Pointer)
            .child(text(if tocando { "❚❚ pausar" } else if c.sel.is_some() { "▶ tocar o corte" } else { "▶ tocar daqui" })
                .font(tema::PEQ_M).color(tema::tinta_acento()))
            .on_press(|s: &mut App| s.cortes_tocar()))
        .child(botao("ct-prox", "quadro ▶", t.txt, false, t, |s: &mut App| s.cortes_passo(1)))
        .child(botao("ct-fim", "⏭", t.txt, false, t, move |s: &mut App| s.cortes_quadro(n.saturating_sub(1))))
        .child(el().w(14.0))
        .child(botao("ct-i", rot_i, t.alerta, marcando, t, |s: &mut App| s.cortes_entrada()))
        .child(botao("ct-o", rot_o, t.alerta, false, t, |s: &mut App| s.cortes_saida()))
        .child(if marcando {
            botao("ct-cancela", "cancelar entrada", t.dim, false, t, |s: &mut App| { s.x.c.entrada = None; s.versao += 1; })
        } else { el() })
        .child(spacer())
        .child(text(format!("{} / {}", tempo(s.cortes_tempo()), tempo(c.duracao()))).font(tema::MONO).color(t.dim));

    let mut col_ = base.child(cab).child(tags_video).child(quadro).child(transporte);
    if let Some((tr, im)) = c.tira.as_ref() {
        let fita = comum::fita("tira-cortes", im, tr.n, s.cortes_quadro_no_ar(), 46.0, t, |s: &mut App, i| s.cortes_clicar_quadro(i));
        col_ = col_.child(comum::rotulo("tira de quadros", t)).child(fita);
        if let Some(w) = largura {
            col_ = col_.child(linha_do_tempo(s, w, t));
        }
    } else {
        col_ = col_.child(el().w_full().h(46.0).radius(4.0).fill(t.card2).key("tira-cortes"));
    }
    col_.child(row().w_full().gap(12.0).center_y().flex_wrap()
        .child(etiqueta("fala", t.acento)).child(etiqueta("cena", t.ok)).child(etiqueta("manual", t.alerta))
        .child(text("Uma faixa por origem. Claro = proposto; forte = aceito ou manual. Clique num corte para escolher. Cortes são anotações: \
                     o arquivo não muda.").font(tema::PEQ).color(t.fraco).wrap().grow(1.0).min_w(0.0)))
}

/// Os cortes do vídeo sobre a duração, uma faixa por origem (fala, cena, manual), com o cursor
/// (quadro atual) e a entrada marcada.
fn linha_do_tempo(s: &App, w: f32, t: Tema) -> El {
    const FAIXA: f32 = 16.0;
    const GAP: f32 = 3.0;
    let c = &s.x.c;
    let d = c.duracao().max(0.1);
    let x = |s: f32| (w * s / d).clamp(0.0, w);
    let alto = 3.0 * FAIXA + 4.0 * GAP;
    let mut l = el().key("linha-cortes").w_full().h(alto).radius(6.0).fill(t.card2);
    for ct in &c.cortes {
        let sel = c.sel == Some(ct.id);
        let cor = cor_origem(&ct.origem, t);
        let id = ct.id;
        let faixa = match ct.origem.as_str() { "fala" => 0.0, "cena" => 1.0, _ => 2.0 };
        let largura = (x(ct.fim) - x(ct.ini)).max(4.0);
        l = l.child(row().key(format!("lc-{id}")).absolute(x(ct.ini), GAP + faixa * (FAIXA + GAP)).w(largura).h(FAIXA)
            .radius(4.0).px(4.0).center_y()
            .fill(cor.alpha(if ct.proposto { 0.16 } else { 0.42 }))
            .border(if sel { 2.0 } else { 1.0 }, if sel { t.txt } else { cor.alpha(if ct.proposto { 0.55 } else { 1.0 }) })
            .cursor(Cursor::Pointer).on_press(move |s: &mut App| s.cortes_selecionar(id))
            .child(if largura > 34.0 { text(ct.origem.clone()).font(tema::MINI).color(cor).truncate() } else { text("") }));
    }
    if let Some(e) = c.entrada {
        l = l.child(el().absolute(x(e) - 1.0, 0.0).w(2.0).h(alto).fill(t.alerta));
    }
    l.child(el().absolute(x(s.cortes_tempo()) - 1.0, 0.0).w(2.0).h(alto).fill(t.txt).inert())
}

// ------------------------------------------------------------ lado direito
fn lado(s: &App, t: Tema, k: &rune_kit::Kit) -> El {
    let c = &s.x.c;
    let n_um = c.cortes.len();
    let n_sel = c.dos_selecionados.len();
    let mut p = col().key("cortes-lado").w(LADO_W).h_full().padding(16.0).gap(12.0).fill(t.card)
        .border_sides([0.0, 0.0, 0.0, 1.0], t.linha).scroll(ScrollSpec { stick_bottom: false })
        .child(seg("seg-cortes-lado", vec![
            seg_item("cl-um", format!("Este vídeo · {n_um}"), !c.ver_selecionados, t, |s: &mut App| { s.x.c.ver_selecionados = false; s.versao += 1; }),
            seg_item("cl-sel", format!("Selecionados · {n_sel}"), c.ver_selecionados, t, |s: &mut App| s.cortes_dos_selecionados()),
        ], t));
    let propor = row().w_full().gap(6.0).flex_wrap()
        .child(botao("propor-fala", if c.propondo { "propondo…" } else { "propor pela fala" }, t.acento, c.propondo, t,
                     |s: &mut App| s.cortes_propor("fala")))
        .child(botao("propor-cena", "propor por cena", t.ok, false, t, |s: &mut App| s.cortes_propor("cena")))
        .child(botao("aceitar-todos", "aceitar todos propostos", t.txt, false, t, |s: &mut App| s.corte_aceitar(true)));
    if c.ver_selecionados {
        p = p.child(selecionados(s, t, k, propor));
        return p;
    }
    if c.video.is_none() {
        return p.child(text("Abra um vídeo na lista para ver e marcar os cortes dele, ou selecione vários e veja \
            \"Selecionados\".").font(tema::PEQ.lh(16.0)).color(t.dim).wrap());
    }
    p = p.child(row().w_full().center_y().child(text("Cortes").font(tema::NORMAL_B).color(t.txt)))
        .child(propor)
        .child(text("Tags do corte são independentes das do vídeo (as do vídeo só valem se você copiar). Mexer na borda, \
            aceitar, dividir ou juntar protege o corte: reprocessar não mexe nele.").font(tema::PEQ.lh(16.0)).color(t.dim)
            .wrap().w_full().padding(10.0).radius(6.0).fill(t.card2));
    if c.cortes.is_empty() {
        p = p.child(text("Nenhum corte ainda. Proponha pela fala (precisa de transcrição) ou por cena, ou marque à mão \
            com I e O.").font(tema::PEQ.lh(16.0)).color(t.fraco).wrap());
    }
    for ct in &c.cortes {
        p = p.child(card_corte(s, ct, t, k));
    }
    p
}

fn card_corte(s: &App, ct: &Corte, t: Tema, k: &rune_kit::Kit) -> El {
    let c = &s.x.c;
    let sel = c.sel == Some(ct.id);
    let cor = cor_origem(&ct.origem, t);
    let id = ct.id;
    let (st, cor_st) = match ct.status() { "proposto" => ("proposto", t.rep), "manual" => ("protegido", t.dim), _ => ("aceito", t.ok) };
    let mut tags = row().w_full().gap(4.0).flex_wrap();
    for tg in &ct.tags {
        let slug = tg.slug.clone();
        tags = tags.child(if sel {
            pastilha(format!("ctg-{id}-{}", tg.slug), format!("{} ×", tg.nome), false, t, move |s: &mut App| s.corte_tag(&slug, true))
        } else { etiqueta(tg.nome.clone(), t.txt) });
    }
    let mut card = col().key(format!("cc-{id}")).w_full().gap(6.0).padding(10.0).radius(8.0)
        .fill(t.card2).border(if sel { 2.0 } else { 1.0 }, if sel { t.acento } else { t.linha })
        .cursor(Cursor::Pointer).on_press(move |s: &mut App| s.cortes_selecionar(id))
        .child(row().w_full().gap(6.0).center_y()
            .child(etiqueta(ct.origem.clone(), cor))
            .child(text(format!("{} → {}", tempo(ct.ini), tempo(ct.fim))).font(tema::MONO).color(t.txt))
            .child(text(format!("{:.1} s", ct.fim - ct.ini)).font(tema::PEQ).color(t.fraco))
            .child(spacer())
            .child(etiqueta(st, cor_st)));
    if !ct.texto.is_empty() {
        let txt: String = if sel { ct.texto.clone() } else { ct.texto.chars().take(110).collect::<String>() + if ct.texto.chars().count() > 110 { "…" } else { "" } };
        card = card.child(text(txt).font(tema::PEQ.lh(16.0)).color(t.dim).wrap());
    }
    if !ct.tags.is_empty() { card = card.child(tags); }
    if !sel {
        if !ct.nota.is_empty() { card = card.child(text(format!("nota: {}", ct.nota)).font(tema::PEQ).color(t.fraco).truncate()); }
        return card;
    }
    // escolhido: bordas, ações, tags e nota
    let video_tags: Vec<String> = c.video.as_ref().map(|m| m.tags.iter().filter(|x| x.origem != "pasta").map(|x| x.nome.clone()).collect())
        .unwrap_or_default();
    card.child(row().w_full().gap(4.0).center_y().flex_wrap()
            .child(text("entrada").font(tema::PEQ).color(t.dim))
            .child(botao(format!("bi-m-{id}"), "−0,1", t.txt, false, t, |s: &mut App| s.cortes_borda("ini", -0.1)))
            .child(botao(format!("bi-p-{id}"), "+0,1", t.txt, false, t, |s: &mut App| s.cortes_borda("ini", 0.1)))
            .child(el().w(8.0))
            .child(text("saída").font(tema::PEQ).color(t.dim))
            .child(botao(format!("bf-m-{id}"), "−0,1", t.txt, false, t, |s: &mut App| s.cortes_borda("fim", -0.1)))
            .child(botao(format!("bf-p-{id}"), "+0,1", t.txt, false, t, |s: &mut App| s.cortes_borda("fim", 0.1))))
        .child(row().w_full().gap(6.0).flex_wrap()
            .child(botao(format!("cdiv-{id}"), "dividir no quadro", t.txt, false, t, |s: &mut App| s.corte_dividir_aqui()))
            .child(botao(format!("cjun-{id}"), "juntar com o próximo", t.txt, false, t, |s: &mut App| s.corte_juntar_proximo())))
        .child(TextInput::new(&c.nova_tag).placeholder("tag do corte… (Enter)")
            .on_edit(|s: &mut App| &mut s.x.c.nova_tag)
            .on_submit(|s: &mut App, _| s.corte_tag_digitada())
            .build(k).key(format!("ctag-{id}")).w_full())
        .child(if video_tags.is_empty() { el() } else {
            row().w_full().gap(4.0).center_y().flex_wrap()
                .child(text("vídeo:").font(tema::PEQ).color(t.fraco))
                .child(text(video_tags.join(", ")).font(tema::PEQ).color(t.fraco).truncate().min_w(0.0).grow(1.0))
                .child(botao(format!("ccop-{id}"), "copiar para o corte", t.acento, false, t, |s: &mut App| s.corte_copiar_tags()))
        })
        .child(row().w_full().gap(6.0).center_y()
            .child(TextInput::new(&c.nota).placeholder("nota do corte (Enter salva)")
                .on_edit(|s: &mut App| &mut s.x.c.nota)
                .on_submit(|s: &mut App, _| s.corte_nota_salvar())
                .build(k).key(format!("cnota-{id}")).grow(1.0).min_w(0.0))
            .child(botao(format!("cnsal-{id}"), "salvar", t.txt, false, t, |s: &mut App| s.corte_nota_salvar())))
        .child(row().w_full().gap(6.0).pt(2.0)
            .child(if ct.proposto { botao(format!("cace-{id}"), "aceitar", t.ok, false, t, |s: &mut App| s.corte_aceitar(false)) } else { el() })
            .child(spacer())
            .child(botao(format!("cdes-{id}"), if ct.origem == "manual" { "apagar" } else { "descartar" }, t.erro, false, t,
                         |s: &mut App| s.corte_descartar())))
}

/// "Cortes dos selecionados": os cortes de vários vídeos juntos, por vídeo, com ações em lote.
fn selecionados(s: &App, t: Tema, k: &rune_kit::Kit, propor: El) -> El {
    let c = &s.x.c;
    let vm = s.vista(Vista::Cortes);
    let n_videos = vm.selecao.len();
    let mut col_ = col().w_full().gap(12.0)
        .child(row().w_full().gap(8.0).center_y()
            .child(text("Cortes dos selecionados").font(tema::NORMAL_B).color(t.txt))
            .child(spacer())
            .child(etiqueta(format!("{n_videos} vídeos · {} cortes", c.dos_selecionados.len()), t.acento)));
    if n_videos == 0 {
        return col_.child(text("Marque vídeos na lista (a caixinha de cada um) para ver e tratar os cortes deles juntos.")
            .font(tema::PEQ.lh(16.0)).color(t.dim).wrap());
    }
    col_ = col_.child(propor)
        .child(TextInput::new(&c.nova_tag).placeholder("tag para todos os cortes abaixo… (Enter)")
            .on_edit(|s: &mut App| &mut s.x.c.nova_tag)
            .on_submit(|s: &mut App, _| s.corte_tag_digitada())
            .build(k).key("ctag-lote").w_full());
    let mut grupo: Option<i64> = None;
    for ct in &c.dos_selecionados {
        if grupo != Some(ct.midia) {
            grupo = Some(ct.midia);
            let m = vm.itens.iter().find(|m| m.id == ct.midia).cloned();
            let nome = m.as_ref().map(|m| m.nome.clone()).unwrap_or_else(|| format!("vídeo {}", ct.midia));
            let n = c.dos_selecionados.iter().filter(|x| x.midia == ct.midia).count();
            let mut cab = row().key(format!("cg-{}", ct.midia)).w_full().gap(8.0).center_y().pt(6.0)
                .child(text(nome).font(tema::PEQ_M).color(t.txt).truncate().grow(1.0).min_w(0.0))
                .child(etiqueta(n.to_string(), t.dim));
            if let Some(m) = m {
                cab = cab.child(botao(format!("cg-abrir-{}", ct.midia), "abrir", t.acento, false, t, move |s: &mut App| s.cortes_abrir(m.clone())));
            }
            col_ = col_.child(cab);
        }
        let cor = cor_origem(&ct.origem, t);
        let (st, cor_st) = match ct.status() { "proposto" => ("proposto", t.rep), "manual" => ("protegido", t.dim), _ => ("aceito", t.ok) };
        let mut linha = col().key(format!("cs-{}", ct.id)).w_full().gap(4.0).padding(8.0).radius(6.0).fill(t.card2).border(1.0, t.linha)
            .child(row().w_full().gap(6.0).center_y()
                .child(etiqueta(ct.origem.clone(), cor))
                .child(text(format!("{} → {}", tempo(ct.ini), tempo(ct.fim))).font(tema::MONO).color(t.txt))
                .child(spacer()).child(etiqueta(st, cor_st)));
        if !ct.tags.is_empty() {
            linha = linha.child(text(ct.tags.iter().map(|x| x.nome.as_str()).collect::<Vec<_>>().join(", "))
                .font(tema::PEQ).color(t.dim).truncate());
        }
        col_ = col_.child(linha);
    }
    col_
}
