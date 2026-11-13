//! A janela, no layout do mockup (`claude/mockup-painel-ng3.html`): lateral com a marca e as abas,
//! cabeçalho com título e busca, filtros, conteúdo da aba, rodapé (seleção e tarefas), preview.

mod biblioteca;
mod comum;
mod cortes;
mod grade;
mod importar;
mod lista;
mod plugins;
mod preview;
mod roteiros;
pub mod tema;
pub use grade::GAP;

use crate::app::{Aba, App, POR_PAGINA, Tamanho};
use crate::filtro::{self, Ordem, Status};
use comum::{botao, botao_tx, pastilha, seg, seg_item};
use rune::{Cursor, Cx, El, ScrollSpec, col, el, row, spacer, text};
use rune_kit::TextInput;
use tema::Tema;

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let t = tema::tema();
    let k = tema::kit();
    let corpo = match (&s.erro_ponte, s.carregando) {
        (Some(e), _) if s.posts.is_empty() => aviso_central(&format!(
            "Não consegui falar com o motor Python.\n\n{e}\n\nO painel procura o acervo em {}.\n\
             Rode de dentro da pasta do acervo ou passe --acervo <pasta>.", s.base.display()), t),
        (_, true) => aviso_central("carregando o acervo…", t),
        _ => match s.aba {
            Aba::Posts => grade::grade(s, t)
                .on_layout(|s: &mut App, r| {
                    s.largura = r[2] - 32.0;
                    let c = grade::colunas(s.largura, s.tamanho);
                    if c != s.colunas { s.colunas = c; s.versao += 1; }
                }),
            Aba::Recomendados => lista::recomendados(s),
            Aba::Roteiros => roteiros::aba(s, t, &k),
            Aba::Importar => importar::aba(s, t, &k),
            Aba::Acervo => biblioteca::aba_acervo(s, t, &k),
            Aba::Cortes => cortes::aba(s, t, &k, cx.layout_size("tira-cortes").map(|v| v[0])),
            Aba::Plugins => plugins::aba(s, t, cx),
            Aba::Config => col().key("aba-config").w_full().grow(1.0).min_h(0.0).padding(24.0)
                .scroll(ScrollSpec { stick_bottom: false }).fill(t.bg)
                .child(row().w_full().child(spacer()).child(conteudo_chaves(s, t, &k).w(640.0)).child(spacer())),
        },
    };
    let mut principal = col().key("principal").grow(1.0).min_w(0.0).h_full().fill(t.bg)
        .child(cabecalho(s, t, &k));
    if !s.aba.sem_lista() {
        principal = principal.child(filtros(s, t));
    }
    principal = principal.child(corpo).child(rodape(s, t));
    if s.tarefas_abertas {
        principal = principal.child(tarefas(s, t));
    }
    let raiz = row().key("raiz").w_full().h_full().fill(t.bg)
        .child(lateral(s, t))
        .child(principal);
    let mut camadas = vec![];
    let tira_w = cx.layout_size("tira").map(|v| v[0]);
    if let Some(p) = preview::preview(s, t, tira_w) {
        camadas.push(p);
    }
    if s.x.b.janela_importar {
        camadas.push(importar::janela(s, t, &k));
    }
    if s.chave_aberta {
        camadas.push(painel_chave(s, t, &k));
    }
    if s.x.clientes_aberto {
        camadas.push(menu_clientes(s, t, &k));
    }
    if let Some(a) = &s.aviso {
        camadas.push(toast(a, t));
    }
    rune_kit::root(raiz, camadas)
}

fn aviso_central(msg: &str, t: Tema) -> El {
    col().w_full().grow(1.0).center().padding(40.0)
        .child(text(msg.to_string()).font(tema::NORMAL.lh(20.0)).color(t.dim).max_w(640.0))
}

/// Nome e subtítulo de cada aba, no cabeçalho.
fn titulo(a: Aba) -> (&'static str, &'static str) {
    match a {
        Aba::Importar => ("Importar", "Fila de revisão e tags"),
        Aba::Posts => ("Posts", "Acervo publicado: pilar, data, curtidas e score"),
        Aba::Recomendados => ("Repost", "Ranking de reaproveitamento dos posts"),
        Aba::Acervo => ("Acervo", "Tudo junto: posts, cortes e pastas apontadas"),
        Aba::Cortes => ("Cortes", "Preview e cortes: fala, cena e manual, como anotação"),
        Aba::Roteiros => ("Roteiros", "Roteiros a partir das falas transcritas"),
        Aba::Plugins => ("Plugins", "Plugins Luau, com os parâmetros de cada um"),
        Aba::Config => ("Chaves e config", "IA das sugestões, chaves de API e marca"),
    }
}

/// A lateral do mockup: logo, seletor de marca, abas com contagem e, no pé, o motor e a IA.
fn lateral(s: &App, t: Tema) -> El {
    let logo = el().size(28.0, 28.0).radius(7.0).fill(t.acento)
        .linear_gradient(std::f32::consts::FRAC_PI_4 * 3.0, [(0.0, t.acento), (1.0, t.rep)])
        .center()
        .child(text("NG3").font(rune::tok::TextStyle::new(11.0, 700.0)).color(rune::Color::WHITE));
    let marca = row().key("cliente").w_full().h(32.0).px(10.0).gap(6.0).center_y().radius(8.0)
        .fill(if s.x.clientes_aberto { t.acento.alpha(0.12) } else { t.card2 })
        .border(1.0, if s.x.clientes_aberto { t.acento } else { t.linha })
        .hover(t.linha_forte.alpha(0.3)).cursor(Cursor::Pointer)
        .child(text("Marca").font(tema::PEQ).color(t.dim))
        .child(spacer())
        .child(text(format!("{} ▾", s.nome_cliente())).font(tema::PEQ_M).color(t.txt).truncate().min_w(0.0))
        .on_press(|s: &mut App| s.x.clientes_aberto = !s.x.clientes_aberto);

    let n_fila = s.x.b.fila.resumo.passos.get("fila").copied().unwrap_or(0);
    let nav = col().key("nav").w_full().gap(2.0)
        .child(item_nav(s, t, Some(Aba::Importar), "Importar", n_fila))
        .child(item_nav(s, t, None, "Tags da marca", 0))
        .child(item_nav(s, t, Some(Aba::Posts), "Posts", s.posts.len()))
        .child(item_nav(s, t, Some(Aba::Recomendados), "Repost", 0))
        .child(item_nav(s, t, Some(Aba::Acervo), "Acervo", s.x.b.acervo.resumo.passos.get("todos").copied().unwrap_or(0)))
        .child(item_nav(s, t, Some(Aba::Cortes), "Cortes", 0))
        .child(item_nav(s, t, Some(Aba::Roteiros), "Roteiros", s.x.roteiros.len()))
        .child(item_nav(s, t, None, "Projetos", 0))
        .child(text("OUTRAS ABAS").font(tema::MINI).color(t.fraco).pt(14.0).pb(6.0).px(10.0))
        .child(item_nav(s, t, Some(Aba::Plugins), "Plugins", s.plugins.quantos()))
        .child(item_nav(s, t, Some(Aba::Config), "Chaves e config", 0));

    let led = if s.ponte.is_some() { t.ok } else { t.alerta };
    let estado = if s.ponte.is_some() { "motor Python ligado" } else { "sem motor Python" };
    let pe = col().key("lateral-pe").w_full().gap(8.0).pt(10.0).border_sides([1.0, 0.0, 0.0, 0.0], t.linha)
        .child(row().gap(6.0).px(4.0).center_y()
            .child(el().size(7.0, 7.0).radius(4.0).fill(led))
            .child(text(estado).font(tema::PEQ).color(t.dim)))
        .child(botao_claude(s, t));

    col().key("lateral").w(210.0).h_full().px(10.0).py(14.0).fill(t.card).border_sides([0.0, 1.0, 0.0, 0.0], t.linha)
        .child(row().w_full().gap(9.0).px(8.0).pt(4.0).pb(16.0).center_y()
            .child(logo)
            .child(col().gap(1.0)
                .child(text("Painel NG3").font(tema::NORMAL_B).color(t.txt))
                .child(text("Automação de vídeo").font(tema::PEQ).color(t.dim))))
        .child(col().w_full().px(6.0).pb(14.0).child(marca))
        .child(nav)
        .child(spacer())
        .child(pe)
}

/// Um item da lateral. `aba: None` é tela do plano que ainda não existe (aparece apagada).
fn item_nav(s: &App, t: Tema, aba: Option<Aba>, rotulo: &str, n: usize) -> El {
    let on = aba == Some(s.aba);
    let mut r = row().key(format!("nav-{rotulo}")).w_full().h(32.0).px(10.0).gap(6.0).center_y().radius(6.0)
        .fill(if on { t.acento.alpha(0.14) } else { rune::Color::TRANSPARENT })
        .child(text(rotulo.to_string()).font(if on { tema::NORMAL_M } else { tema::NORMAL })
            .color(if on { t.acento } else if aba.is_none() { t.fraco } else { t.dim }))
        .child(spacer());
    match aba {
        Some(a) => {
            if n > 0 {
                r = r.child(row().h(18.0).px(7.0).center_y().radius(9.0).fill(t.card2)
                    .child(text(n.to_string()).font(tema::PEQ).color(t.dim)));
            }
            r.hover(t.card2).cursor(Cursor::Pointer).on_press(move |s: &mut App| s.trocar_aba(a))
        }
        None => r.child(text("em breve").font(tema::PEQ).color(t.fraco.alpha(0.6))),
    }
}

/// Barra de cima da área principal: título e subtítulo da aba; busca nas abas com lista.
fn cabecalho(s: &App, t: Tema, k: &rune_kit::Kit) -> El {
    let (nome, sub) = titulo(s.aba);
    let mut acoes = row().gap(8.0).center_y();
    if !s.aba.sem_lista() {
        acoes = acoes
            .child(text(format!("{} na tela", s.visiveis.len())).font(tema::PEQ).color(t.dim))
            .child(TextInput::new(&s.busca)
                .placeholder("buscar na pasta, na legenda e no que é falado")
                .on_edit(|s: &mut App| &mut s.busca)
                .on_change(|s: &mut App, tx| s.tecla_busca(tx))
                .build(k)
                .key("busca").w(340.0));
    }
    row().key("cabecalho").w_full().h(54.0).px(22.0).gap(10.0).center_y()
        .fill(t.card).border_sides([0.0, 0.0, 1.0, 0.0], t.linha)
        .child(text(nome).font(tema::TITULO).color(t.txt))
        .child(text(sub).font(tema::PEQ).color(t.dim).truncate().min_w(0.0))
        .child(spacer())
        .child(acoes)
}

/// Filtros das abas com lista (posts, repost, cortes), em duas linhas como no mockup:
/// controles de exibição em cima (status, tipo, ordem, tamanho, por página), ano e pilar embaixo.
fn filtros(s: &App, t: Tema) -> El {
    let mut controles = row().w_full().gap(10.0).center_y().flex_wrap();
    if s.aba == Aba::Posts {
        let mut st = vec![];
        for x in Status::TODOS {
            st.push(seg_item(format!("st-{}", x.rotulo()), x.rotulo(), s.filtro.status == x, t, move |s: &mut App| s.status(x)));
        }
        controles = controles.child(seg("seg-status", st, t));
        let mut tp = vec![];
        for tipo in ["fcproj", "pcraft"] {
            tp.push(seg_item(format!("tipo-{tipo}"), tipo, s.filtro.tipo == tipo, t, move |s: &mut App| {
                s.filtro.tipo = if s.filtro.tipo == tipo { String::new() } else { tipo.into() };
                s.refiltrar();
            }));
        }
        controles = controles.child(seg("seg-tipo", tp, t));
    }
    controles = controles.child(pastilha("sem-projeto".into(), "só sem projeto".into(), s.filtro.so_sem_projeto, t, |s: &mut App| {
        s.filtro.so_sem_projeto = !s.filtro.so_sem_projeto;
        s.refiltrar();
    }));
    controles = controles.child(spacer());
    if s.aba != Aba::Recomendados {
        let mut ord = vec![];
        for o in Ordem::TODAS {
            ord.push(seg_item(format!("ord-{}", o.rotulo()), o.rotulo(), s.filtro.ordem == o, t, move |s: &mut App| s.ordenar(o)));
        }
        let mut tam = vec![];
        for tm in Tamanho::TODOS {
            tam.push(seg_item(format!("tam-{}", tm.rotulo()), tm.rotulo(), s.tamanho == tm, t, move |s: &mut App| s.tamanho(tm)));
        }
        let mut pp = vec![];
        for n in POR_PAGINA {
            let rot = if n == 0 { "todos".to_string() } else { n.to_string() };
            pp.push(seg_item(format!("pp-{n}"), rot, s.por_pagina == n, t, move |s: &mut App| s.por_pagina(n)));
        }
        controles = controles
            .child(row().gap(6.0).center_y().child(comum::rotulo("ordem", t)).child(seg("seg-ordem", ord, t)))
            .child(row().gap(6.0).center_y().child(comum::rotulo("cards", t)).child(seg("seg-tam", tam, t)))
            .child(row().gap(6.0).center_y().child(comum::rotulo("por página", t)).child(seg("seg-pp", pp, t)));
    }
    let tem_filtro = !s.filtro.anos.is_empty() || !s.filtro.pilares.is_empty() || !s.filtro.tipo.is_empty()
        || s.filtro.so_sem_projeto || s.filtro.status != Status::Todos;
    if tem_filtro {
        controles = controles.child(botao("limpar", "limpar filtros", t.dim, false, t, |s: &mut App| s.limpar_filtros()));
    }

    let mut grupos = row().w_full().gap(6.0).center_y().flex_wrap();
    {
        let mut anos: Vec<&str> = s.posts.iter().map(|p| p.ano.as_str()).collect();
        anos.sort_unstable();
        anos.dedup();
        grupos = grupos.child(comum::rotulo("ano", t).pr(2.0));
        for a in anos.into_iter().rev() {
            let on = s.filtro.anos.contains(a);
            let a2 = a.to_string();
            grupos = grupos.child(pastilha(format!("ano-{a}"), a.to_string(), on, t, move |s: &mut App| s.alternar_ano(&a2)));
        }
        grupos = grupos.child(el().w(1.0).h(18.0).mx(6.0).fill(t.linha_forte));
    }
    let cont = filtro::contagem(s.posts.iter().map(|p| p.pilar.as_str()));
    grupos = grupos.child(comum::rotulo("pilar", t).pr(2.0));
    for (p, n) in cont {
        let on = s.filtro.pilares.contains(&p);
        let p2 = p.clone();
        grupos = grupos.child(pastilha(format!("pilar-{p}"), format!("{p} {n}"), on, t, move |s: &mut App| s.alternar_pilar(&p2)));
    }

    col().key("filtros").w_full().gap(10.0).px(22.0).py(10.0).fill(t.bg).border_sides([0.0, 0.0, 1.0, 0.0], t.linha)
        .child(controles)
        .child(grupos)
}

fn rodape(s: &App, t: Tema) -> El {
    let n = s.selecao.len();
    let rodando = s.tarefas.iter().filter(|t| t.fim.is_none()).count();
    let mut r = row().key("rodape").w_full().h(52.0).px(16.0).gap(10.0).center_y()
        .fill(t.card).border_sides([1.0, 0.0, 0.0, 0.0], t.linha);
    // Acervo e Importar têm seleção e ações próprias (painel de tags): aqui só as tarefas
    let de_posts = !matches!(s.aba, Aba::Acervo | Aba::Cortes | Aba::Importar);
    if !de_posts {
        let rodando = s.tarefas.iter().filter(|t| t.fim.is_none()).count();
        let rot = rotulo_tarefas(s);
        return r.h(40.0).child(spacer()).child(botao("ver-tarefas", rot, if rodando > 0 { t.alerta } else { t.dim },
                                                      s.tarefas_abertas, t, |s: &mut App| s.tarefas_abertas = !s.tarefas_abertas));
    }
    r = r.child(text(s.rotulo_selecao()).font(tema::NORMAL_M).color(if n > 0 { t.acento } else { t.dim }));
    if n > 0 {
        {
            r = r.child(text(format!("~{:.0} MB de projeto", s.mb_projeto())).font(tema::PEQ).color(t.dim));
        }
        r = r.child(botao("limpar-sel", "limpar seleção", t.dim, false, t, |s: &mut App| { s.selecao.clear(); s.versao += 1; }));
    }
    if !s.aba.sem_lista() && !s.visiveis.is_empty() {
        r = r.child(paginador(s, t)).child(spacer());
    } else {
        r = r.child(spacer());
    }
    let rot_tarefas = rotulo_tarefas(s);
    r = r.child(botao("ver-tarefas", rot_tarefas, if rodando > 0 { t.alerta } else { t.dim }, s.tarefas_abertas, t,
                      |s: &mut App| s.tarefas_abertas = !s.tarefas_abertas));
    let pode = n > 0 && s.ponte.is_some();
    r.child(row().key("gerar").h(30.0).px(14.0).center_y().radius(6.0)
        .fill(if pode { t.acento } else { t.card2 }).border(1.0, if pode { t.acento } else { t.linha_forte }).cursor(if pode { Cursor::Pointer } else { Cursor::NotAllowed })
        .child(text(format!("gerar projeto ({n})")).font(tema::NORMAL_M).color(if pode { tema::tinta_acento() } else { t.dim }))
        .on_press(|s: &mut App| s.gerar_selecao()))
}

pub fn colunas(largura: f32, tm: Tamanho) -> usize { grade::colunas(largura, tm) }

/// ‹ 1 2 … 7 8 9 … 26 › · por página
fn paginador(s: &App, t: Tema) -> El {
    let n = s.n_paginas();
    let atual = s.pagina;
    let mut r = row().key("paginador").gap(4.0).center_y();
    if n > 1 {
        r = r.child(botao_tx("pag-ant", "‹", t.txt, false, t, move |s: &mut App, tx| s.ir_pagina(atual.saturating_sub(1), tx)));
        let mut mostrar: Vec<usize> = vec![0, n - 1];
        mostrar.extend(atual.saturating_sub(2)..(atual + 3).min(n));
        mostrar.sort_unstable();
        mostrar.dedup();
        let mut ant = None;
        for p in mostrar {
            if let Some(a) = ant && p > a + 1 {
                r = r.child(text("…").font(tema::PEQ).color(t.dim));
            }
            r = r.child(botao_tx(format!("pag-{p}"), (p + 1).to_string(), if p == atual { t.acento } else { t.txt }, p == atual, t,
                                 move |s: &mut App, tx| s.ir_pagina(p, tx)));
            ant = Some(p);
        }
        r = r.child(botao_tx("pag-prox", "›", t.txt, false, t, move |s: &mut App, tx| s.ir_pagina(atual + 1, tx)));
    }
    let faixa = filtro::pagina(s.visiveis.len(), s.pagina, s.por_pagina);
    r.child(text(format!("{}–{} de {}", faixa.start + 1, faixa.end, s.visiveis.len())).font(tema::PEQ).color(t.dim).px(6.0))
}

/// No fim da grade: selecionar a página inteira e ir para a próxima.
pub fn fim_de_pagina(s: &App, t: Tema) -> El {
    let n = s.n_paginas();
    let atual = s.pagina;
    let mut r = row().key("fim-pagina").w_full().gap(10.0).py(18.0).center();
    r = r.child(botao("sel-pagina", "selecionar / tirar a página toda", t.acento, false, t, |s: &mut App| s.selecionar_pagina()));
    if atual + 1 < n {
        r = r.child(botao_tx("prox-pagina", format!("próxima página ({} de {}) ›", atual + 2, n), t.txt, false, t,
                             move |s: &mut App, tx| s.ir_pagina(atual + 1, tx)));
    }
    r
}

fn tarefas(s: &App, t: Tema) -> El {
    let mut c = col().key("tarefas").w_full().h(220.0).gap(10.0).px(16.0).py(10.0).fill(t.card).border_sides([1.0, 0.0, 0.0, 0.0], t.linha)
        .scroll(ScrollSpec { stick_bottom: true });
    if s.tarefas.is_empty() {
        c = c.child(text("nenhuma tarefa ainda: gerar projeto e os plugins aparecem aqui, com a saída linha a linha")
            .font(tema::PEQ).color(t.dim));
    }
    for tf in s.tarefas.iter().rev().take(6) {
        let (cor, st) = match tf.fim {
            None => (t.alerta, "rodando"),
            Some(true) => (t.ok, "ok"),
            Some(false) => (t.erro, "falhou"),
        };
        let ultimas: Vec<&str> = tf.linhas.iter().rev().take(12).map(String::as_str).collect::<Vec<_>>().into_iter().rev().collect();
        c = c.child(col().key(format!("t-{}", tf.id)).w_full().gap(4.0)
            .child(row().w_full().gap(8.0).center_y()
                .child(comum::etiqueta(st, cor))
                .child(text(tf.dono.clone()).font(tema::PEQ_M).color(t.txt))
                .child(text(tf.comando.clone()).font(tema::MONO).color(t.dim).truncate().grow(1.0).min_w(0.0))
                .child(if tf.fim.is_none() {
                    let id = tf.id.clone();
                    botao(format!("cancel-{}", tf.id), "cancelar", t.erro, false, t, move |s: &mut App| s.cancelar_tarefa(&id))
                } else { el() }))
            .child(barra_tarefa(tf, t))
            .child(text(ultimas.join("\n")).font(tema::MONO.lh(16.0)).color(t.dim).selectable(true).wrap_anywhere()));
    }
    c
}

/// "tarefas · 1 rodando · 42%" (a média das que dizem quanto falta).
fn rotulo_tarefas(s: &App) -> String {
    let rodando: Vec<_> = s.tarefas.iter().filter(|t| t.fim.is_none()).collect();
    if rodando.is_empty() { return format!("tarefas {}", s.tarefas.len()); }
    let fr: Vec<f32> = rodando.iter().filter_map(|t| t.progresso).map(|(a, b)| a as f32 / b as f32).collect();
    if fr.is_empty() { format!("tarefas · {} rodando", rodando.len()) }
    else { format!("tarefas · {} rodando · {:.0}%", rodando.len(), 100.0 * fr.iter().sum::<f32>() / fr.len() as f32) }
}

/// A barra de uma tarefa que diz quanto já fez ("[12/340]" nas linhas), com o tempo que falta.
fn barra_tarefa(tf: &crate::app::Tarefa, t: Tema) -> El {
    let Some((a, b)) = tf.progresso else { return el() };
    let cor = match tf.fim { None => t.acento, Some(true) => t.ok, Some(false) => t.erro };
    let falta = if tf.fim.is_none() && !tf.eta.is_empty() { format!(" · faltam ~{}", tf.eta.trim_start_matches("~")) } else { String::new() };
    row().w_full().gap(10.0).center_y()
        .child(col().grow(1.0).min_w(0.0).child(comum::barra(a as f32 / b as f32, cor, t)))
        .child(text(format!("{a} de {b}{falta}")).font(tema::PEQ).color(t.dim))
}

/// Barra de um processo lento do motor que não é tarefa (ex.: catalogar as fontes).
pub fn barra_progresso(s: &App, chave: &str, t: Tema) -> El {
    let Some((rot, a, b)) = s.x.progressos.get(chave) else { return el() };
    let frac = if *b > 0 { *a as f32 / *b as f32 } else { 0.0 };
    row().w_full().gap(10.0).center_y()
        .child(text(rot.clone()).font(tema::PEQ).color(t.dim).truncate().max_w(320.0))
        .child(col().grow(1.0).min_w(0.0).child(comum::barra(frac, t.acento, t)))
        .child(text(if *b > 0 { format!("{a} de {b}") } else { "…".into() }).font(tema::PEQ).color(t.dim))
}

fn toast(msg: &str, t: Tema) -> El {
    col().key("toast-camada").w_full().h_full().absolute(0.0, 0.0).inert()
        .child(spacer())
        .child(row().w_full().pb(70.0).center()
            .child(row().key("toast").px(14.0).py(9.0).radius(9.0).fill(t.card2).border(1.0, t.linha_forte)
                .child(text(msg.to_string()).font(tema::PEQ).color(t.txt).max_w(700.0).wrap_anywhere())))
}

/// No topo: qual IA faz as sugestões e se a chave dela funciona; um clique abre o painel.
fn botao_claude(s: &App, t: Tema) -> El {
    let i = &s.chave_info;
    let nome = if i.nome.is_empty() { "IA".to_string() } else { i.nome.clone() };
    let fim = if i.fim.is_empty() { String::new() } else { format!(" …{}", i.fim) };
    let (cor, rotulo) = match (s.claude, i.valida) {
        (false, _) => (t.alerta, format!("{nome}: sem chave")),
        (true, Some(false)) => (t.erro, format!("{nome}{fim}: erro")),
        (true, _) => (t.ok, format!("{nome} ✓{fim}")),
    };
    row().key("claude-chave").w_full().h(28.0).px(10.0).gap(6.0).center_y().radius(7.0)
        .border(1.0, if s.chave_aberta { cor } else { t.linha })
        .fill(if s.chave_aberta { cor.alpha(0.15) } else { rune::Color::TRANSPARENT })
        .hover(t.linha_forte.alpha(0.2)).cursor(Cursor::Pointer)
        .child(el().size(7.0, 7.0).radius(4.0).fill(cor))
        .child(text(rotulo).font(tema::PEQ_M).color(t.txt).truncate().min_w(0.0))
        .on_press(|s: &mut App| s.chave_aberta = !s.chave_aberta)
}

/// A IA das sugestões: provedor (Claude, DeepSeek, OpenRouter, Ollama, outra), modelo,
/// endereço e chave. Salvar grava em config.local.toml e testa.
/// O seletor de cliente (canto esquerdo do topo): trocar reinicia o motor com o acervo, as chaves
/// e o estado do outro cliente; "novo cliente" copia o molde.
fn menu_clientes(s: &App, t: Tema, k: &rune_kit::Kit) -> El {
    let x = &s.x;
    let mut lista = col().w_full().gap(4.0);
    for c in x.clientes.iter().filter(|c| !c.molde || c.slug == s.marca) {
        let on = c.slug == s.marca;
        let slug = c.slug.clone();
        lista = lista.child(row().key(format!("cli-{}", c.slug)).w_full().h(34.0).px(10.0).gap(8.0).center_y().radius(7.0)
            .fill(if on { t.acento.alpha(0.14) } else { rune::Color::TRANSPARENT })
            .hover(t.linha_forte.alpha(0.2)).cursor(Cursor::Pointer)
            .child(text(if on { "●" } else { "○" }).font(tema::PEQ).color(if on { t.acento } else { t.dim }))
            .child(text(c.nome.clone()).font(tema::NORMAL_M).color(t.txt))
            .child(spacer())
            .child(text(c.slug.clone()).font(tema::MONO).color(t.dim))
            .on_press(move |s: &mut App| { s.x.clientes_aberto = false; s.trocar_marca(&slug); }));
    }
    let painel = col().key("menu-clientes").w(420.0).gap(10.0).padding(14.0).radius(12.0).fill(t.card).border(1.0, t.linha_forte)
        .child(text("CLIENTES").font(tema::MINI).color(t.dim))
        .child(lista)
        .child(el().w_full().h(1.0).fill(t.linha))
        .child(text("NOVO CLIENTE").font(tema::MINI).color(t.dim))
        .child(TextInput::new(&x.novo_cliente).placeholder("nome do cliente").on_edit(|s: &mut App| &mut s.x.novo_cliente)
            .build(k).key("cli-nome").w_full())
        .child(TextInput::new(&x.novo_cliente_acervo).placeholder("pasta do acervo publicado (opcional: vazio cria uma nova)")
            .on_edit(|s: &mut App| &mut s.x.novo_cliente_acervo).build(k).key("cli-acervo").w_full())
        .child(row().w_full().gap(8.0).center_y()
            .child(text("Cada cliente tem o seu acervo, chaves de API, fontes, roteiros e motions.").font(tema::PEQ).color(t.dim)
                .wrap().grow(1.0).min_w(0.0))
            .child(botao("cli-criar", if x.criando_cliente { "criando…" } else { "criar e abrir" }, t.acento, x.criando_cliente, t,
                         |s: &mut App| { if !s.x.criando_cliente { s.criar_cliente() } })));
    col().key("camada-clientes").w_full().h_full().absolute(0.0, 0.0)
        .on_press(|s: &mut App| s.x.clientes_aberto = false)
        .child(row().w_full().pl(214.0).pt(76.0).child(painel.on_press(|_: &mut App| {})).child(spacer()))
}

fn painel_chave(s: &App, t: Tema, k: &rune_kit::Kit) -> El {
    let painel = conteudo_chaves(s, t, k).w(500.0);
    col().key("camada-chave").w_full().h_full().absolute(0.0, 0.0)
        .on_press(|s: &mut App| s.chave_aberta = false)
        .child(col().w_full().h_full().pl(214.0).pb(16.0).child(spacer())
            .child(painel.on_press(|_: &mut App| {})))
}

/// As chaves de API (sugestões e vídeo): no painel do botão do topo e na aba "chaves e config".
fn conteudo_chaves(s: &App, t: Tema, k: &rune_kit::Kit) -> El {
    let i = &s.chave_info;
    let sel = s.provedor_em_edicao().cloned().unwrap_or_default();
    let mudou = s.ia_provedor != i.provedor || s.ia_modelo.text.trim() != i.modelo || s.ia_base.text.trim() != i.base_url;

    let mut provs = row().w_full().gap(6.0).flex_wrap();
    for p in &i.provedores {
        let id = p.id.clone();
        let rot = if p.tem_chave || !p.chave { format!("{} ✓", p.nome) } else { p.nome.clone() };
        provs = provs.child(botao(format!("prov-{}", p.id), rot, t.acento, p.id == s.ia_provedor, t,
                                  move |s: &mut App| s.escolher_provedor(&id)));
    }

    let modelo = TextInput::new(&s.ia_modelo)
        .placeholder(match sel.id.as_str() {
            "openrouter" => "ex.: anthropic/claude-sonnet, google/gemini-…, deepseek/…",
            "ollama" => "um modelo baixado no Ollama (com visão, para ver as imagens)",
            _ => "nome do modelo",
        })
        .on_edit(|s: &mut App| &mut s.ia_modelo).build(k).key("ia-modelo").w_full();
    let base = TextInput::new(&s.ia_base)
        .placeholder("https://…/v1 (formato da OpenAI)")
        .on_edit(|s: &mut App| &mut s.ia_base).build(k).key("ia-base").w_full();
    let campo = TextInput::new(&s.chave)
        .placeholder(if sel.tem_chave { "chave salva — cole outra para trocar" } else { "cole a chave da API" })
        .secure()
        .on_edit(|s: &mut App| &mut s.chave)
        .on_submit(|s: &mut App, _| s.salvar_chave(false))
        .build(k)
        .key("chave-campo").w_full();

    let situacao = match (s.claude, i.origem.as_str(), i.valida) {
        (false, _, _) => format!("Em uso: {} — sem chave. As sugestões de motion precisam dela.", i.nome),
        (true, "ambiente", _) => format!("Em uso: {} · {} · chave …{} da variável de ambiente.", i.nome, i.modelo, i.fim),
        (true, _, Some(true)) if i.erro.is_empty() => format!("Em uso: {} · {} — funcionando.", i.nome, i.modelo),
        (true, _, Some(_)) => format!("Em uso: {} · {} — {}", i.nome, i.modelo, i.erro),
        (true, _, None) => format!("Em uso: {} · {} (ainda não testado).", i.nome, i.modelo),
    };
    let erro = i.valida == Some(false) || !i.erro.is_empty();
    let ocupado = i.testando;
    let rotulo = |txt: &str| text(txt.to_string()).font(tema::MINI).color(t.dim);

    let mut painel = col().key("painel-chave").gap(10.0).padding(16.0).radius(12.0)
        .fill(t.card).border(1.0, t.linha_forte)
        .child(row().w_full().center_y()
            .child(text("Chaves de API").font(tema::TITULO).color(t.txt))
            .child(spacer())
            .child(if s.aba == Aba::Config { el() } else {
                botao("chave-fechar", "fechar", t.dim, false, t, |s: &mut App| s.chave_aberta = false) }))
        .child(row().w_full().gap(6.0).center_y()
            .child(text("salvar para").font(tema::PEQ).color(t.dim))
            .child(pastilha("esc-marca".into(), format!("só {}", s.nome_cliente()), s.x.chave_escopo == "marca", t,
                            |s: &mut App| s.x.chave_escopo = "marca".into()))
            .child(pastilha("esc-geral".into(), "todos os clientes (geral)".into(), s.x.chave_escopo == "geral", t,
                            |s: &mut App| s.x.chave_escopo = "geral".into())))
        .child(text(format!("A IA e as chaves de {} valem só para ele; quem não tem as suas usa as gerais. Agora: IA {} · chave {}.",
                            s.nome_cliente(),
                            if i.escopo_ia == "marca" { "própria do cliente" } else { "geral" },
                            match i.origem.as_str() { "cliente" => "do cliente", "geral" => "geral", "ambiente" => "da variável de ambiente", _ => "nenhuma" }))
            .font(tema::PEQ.lh(16.0)).color(t.dim).wrap())
        .child(if i.escopo_ia == "marca" {
            botao("ia-geral", "este cliente volta a usar a IA geral", t.dim, false, t, |s: &mut App| s.usar_ia_geral()) } else { el() })
        .child(text("Sugestões de motion").font(tema::NORMAL_M).color(t.txt))
        .child(text(situacao).font(tema::PEQ.lh(16.0)).color(if erro { t.erro } else { t.dim }).wrap())
        .child(rotulo("PROVEDOR"))
        .child(provs)
        .child(rotulo("MODELO"))
        .child(modelo);
    if !i.modelos.is_empty() && sel.id == i.provedor && sel.id != "claude" {
        painel = painel.child(text(format!("disponíveis: {}", i.modelos.iter().take(12).cloned().collect::<Vec<_>>().join(", ")))
            .font(tema::PEQ).color(t.dim).wrap_anywhere());
    }
    if sel.id != "claude" {
        painel = painel.child(rotulo("ENDEREÇO DA API")).child(base);
    }
    if sel.chave {
        painel = painel.child(rotulo("CHAVE")).child(campo);
    }
    painel = painel
        .child(row().w_full().gap(8.0).center_y()
            .child(botao("chave-salvar", if ocupado { "conferindo…" } else if mudou { "usar esta IA e testar" } else { "salvar e testar" },
                         t.acento, ocupado, t, |s: &mut App| { if !s.chave_info.testando { s.salvar_chave(false) } }))
            .child(if !mudou && s.claude { botao("chave-testar", "testar", t.txt, false, t, |s: &mut App| s.testar_chave()) } else { el() })
            .child(spacer())
            .child(if sel.chave && sel.tem_chave {
                botao("chave-remover", "remover chave", t.erro, false, t, |s: &mut App| s.salvar_chave(true)) } else { el() }))
        .child(text(match sel.id.as_str() {
            "claude" => "Chave em platform.claude.com → API Keys, na organização ligada ao Team: os créditos mensais do plano pagam o uso.",
            "deepseek" => "Chave em platform.deepseek.com. O modelo deepseek-flash aceita imagens (as referências e os quadros do vídeo).",
            "openrouter" => "Uma chave do openrouter.ai dá acesso a vários modelos; escreva o nome como aparece lá.",
            "ollama" => "Roda no seu computador, sem chave e sem custo: instale o Ollama e baixe um modelo. Sem visão, as imagens ficam de fora.",
            _ => "Qualquer API no formato da OpenAI (/chat/completions). Se o modelo não aceitar imagens, o app pergunta só com o texto.",
        }).font(tema::PEQ.lh(16.0)).color(t.dim).wrap())
        .child(el().w_full().h(1.0).fill(t.linha))
        .child(row().w_full().center_y()
            .child(text("Vídeo com IA (OpenRouter)").font(tema::NORMAL_M).color(t.txt))
            .child(spacer())
            .child(if s.tem_chave_video() { comum::etiqueta(format!("chave …{} ✓", s.chave_info.fim_video), t.ok) }
                   else { comum::etiqueta("sem chave", t.alerta) }))
        .child(text("Chave própria do vídeo (openrouter.ai → Keys), separada da chave das sugestões: \
            pode ser outra conta ou outra chave do OpenRouter, com o seu limite de gasto.").font(tema::PEQ.lh(16.0)).color(t.dim).wrap())
        .child(TextInput::new(&s.chave_video)
            .placeholder(if s.tem_chave_video() { "chave salva — cole outra para trocar" } else { "sk-or-…" })
            .secure()
            .on_edit(|s: &mut App| &mut s.chave_video)
            .on_submit(|s: &mut App, _| s.video_salvar_chave(false))
            .build(k).key("chave-video").w_full())
        .child(row().w_full().gap(8.0).center_y()
            .child(botao("chave-video-salvar", "salvar chave do vídeo", t.acento, false, t, |s: &mut App| s.video_salvar_chave(false)))
            .child(spacer())
            .child(if s.tem_chave_video() {
                botao("chave-video-remover", "remover", t.erro, false, t, |s: &mut App| s.video_salvar_chave(true)) } else { el() }))
        .child(text("As chaves ficam só neste computador, em config/local.toml (fora do git, legível só por você); \
            o app nunca as mostra de volta.").font(tema::PEQ.lh(16.0)).color(t.dim.alpha(0.8)).wrap())
        .child(el().w_full().h(1.0).fill(t.linha))
        .child(text("Marca (cliente)").font(tema::NORMAL_M).color(t.txt))
        .child(text("Cada marca tem o seu acervo, pilares, vocabulário e identidade em config/marcas/<marca>/, e o \
            seu estado em dados/marcas/<marca>/. Trocar reinicia o motor.").font(tema::PEQ.lh(16.0)).color(t.dim).wrap())
        .child(marcas(s, t));
    painel
}

fn marcas(s: &App, t: Tema) -> El {
    let mut r = row().w_full().gap(8.0).center_y().wrap();
    for m in &s.marcas {
        let on = *m == s.marca;
        let m2 = m.clone();
        r = r.child(botao(format!("marca-{m}"), m.clone(), t.acento, on, t, move |s: &mut App| s.trocar_marca(&m2)));
    }
    r
}
