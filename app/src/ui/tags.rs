//! Dentro da aba Importar: o gerenciador de tags da marca, a janela "Importar tags de arquivos"
//! (Lightroom e metadados, com prévia e conflitos) e a janela "Agrupar em projeto".

use super::comum::{self, botao, etiqueta, pastilha, seg, seg_item};
use super::tema::{self, Tema};
use crate::app::App;
use crate::organizar::{CategoriaGer, TagGer};
use rune::{Color, Cursor, El, ScrollSpec, col, el, row, spacer, text};
use rune_kit::TextInput;

fn cor_origem(origem: &str, t: Tema) -> Color {
    match origem {
        "marca" => t.acento,
        "pasta" => t.dim,
        "lightroom" => t.rep,
        "metadados" => t.alerta,
        _ => t.txt,
    }
}

/// O gerenciador: categorias com contagens e tags; criar, renomear, mover, fundir e remover.
pub fn gerenciador(s: &App, t: Tema, k: &rune_kit::Kit) -> El {
    let o = &s.x.o;
    let mut lista = col().key("ger-lista").grow(1.0).min_w(0.0).h_full().gap(10.0).px(22.0).py(16.0)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(text("Categorias e tags da marca. A contagem soma mídias e cortes. Renomear muda só o nome visível; \
                     o slug fica, e as marcações continuam. As categorias marcadas \"do Lightroom\" vieram da importação \
                     e ainda não estão no tags.toml da marca.")
            .font(tema::PEQ.lh(16.0)).color(t.dim).wrap());
    if o.painel.categorias.is_empty() && o.painel.sem_categoria.is_empty() {
        lista = lista.child(text("carregando as tags…").font(tema::PEQ).color(t.dim));
    }
    for c in &o.painel.categorias {
        lista = lista.child(categoria(s, c, t, k));
    }
    if !o.painel.sem_categoria.is_empty() {
        let mut caixa = col().key("ger-sem-cat").w_full().gap(8.0).padding(12.0).radius(8.0).fill(t.card).border(1.0, t.linha)
            .child(row().w_full().gap(8.0).center_y()
                .child(text("Sem categoria").font(tema::NORMAL_B).color(t.txt))
                .child(text(format!("{} tag(s)", o.painel.sem_categoria.len())).font(tema::PEQ).color(t.dim)));
        caixa = caixa.child(tags_da(s, &o.painel.sem_categoria, t, k));
        lista = lista.child(caixa);
    }
    row().key("gerenciador").w_full().grow(1.0).min_h(0.0)
        .child(lista)
        .child(lado(s, t, k))
}

fn categoria(s: &App, c: &CategoriaGer, t: Tema, k: &rune_kit::Kit) -> El {
    let o = &s.x.o;
    let (cs1, cn1, cs2) = (c.slug.clone(), c.nome.clone(), c.slug.clone());
    let aberta = o.cat_aberta.as_deref() == Some(c.slug.as_str());
    let mut topo = row().w_full().gap(8.0).center_y()
        .child(text(c.nome.clone()).font(tema::NORMAL_B).color(t.txt))
        .child(text(format!("{} entrada(s) · {} tag(s)", c.n, c.tags.len())).font(tema::PEQ).color(t.dim))
        .child(if c.da_marca { el() } else { etiqueta("do Lightroom", t.rep) })
        .child(if c.multipla { el() } else { etiqueta("uma por mídia", t.dim) })
        .child(spacer());
    if c.da_marca {
        topo = topo.child(botao(format!("cat-ed-{}", c.slug), if aberta { "fechar" } else { "editar" }, t.dim, aberta, t,
                                move |s: &mut App| s.categoria_abrir(&cs1, &cn1)));
    }
    let mut caixa = col().key(format!("cat-{}", c.slug)).w_full().gap(8.0).padding(12.0).radius(8.0).fill(t.card)
        .border(1.0, if aberta { t.acento } else { t.linha }).child(topo);
    if aberta {
        caixa = caixa.child(row().w_full().gap(8.0).center_y()
            .child(TextInput::new(&o.cat_nome).placeholder("nome da categoria")
                .on_edit(|s: &mut App| &mut s.x.o.cat_nome)
                .on_submit(|s: &mut App, _| s.categoria_renomear())
                .build(k).key(format!("cat-nome-{}", c.slug)).grow(1.0).min_w(0.0))
            .child(botao(format!("cat-ren-{}", c.slug), "renomear", t.acento, false, t, |s: &mut App| s.categoria_renomear()))
            .child(botao(format!("cat-rm-{}", c.slug), "tirar a categoria", t.erro, false, t,
                         move |s: &mut App| s.categoria_remover(&cs2))))
            .child(text("Tirar a categoria deixa as tags dela sem categoria; nenhuma marcação se perde.")
                .font(tema::PEQ).color(t.fraco));
    }
    if c.tags.is_empty() {
        caixa = caixa.child(text("nenhuma tag ainda").font(tema::PEQ).color(t.fraco));
    } else {
        caixa = caixa.child(tags_da(s, &c.tags, t, k));
    }
    caixa
}

fn tags_da(s: &App, tags: &[TagGer], t: Tema, k: &rune_kit::Kit) -> El {
    let o = &s.x.o;
    let mut chips = row().w_full().gap(6.0).flex_wrap();
    let mut aberta_aqui = None;
    for tg in tags {
        let (sl, nm) = (tg.slug.clone(), tg.nome.clone());
        let on = o.tag_aberta.as_deref() == Some(tg.slug.as_str());
        if on { aberta_aqui = Some(tg); }
        chips = chips.child(row().key(format!("tg-{}", tg.slug)).h(26.0).px(10.0).gap(6.0).center_y().radius(13.0)
            .fill(if on { t.acento.alpha(0.14) } else { t.card2 }).border(1.0, if on { t.acento } else { t.linha })
            .cursor(Cursor::Pointer).hover(t.linha_forte.alpha(0.25))
            .child(el().size(7.0, 7.0).radius(4.0).fill(cor_origem(&tg.origem, t)))
            .child(text(tg.nome.clone()).font(tema::PEQ).color(t.txt))
            .child(text(tg.n.to_string()).font(tema::PEQ).color(t.dim))
            .on_press(move |s: &mut App| s.tag_abrir(&sl, &nm)));
    }
    let mut c = col().w_full().gap(8.0).child(chips);
    if let Some(tg) = aberta_aqui {
        c = c.child(editor_tag(s, tg, t, k));
    }
    c
}

fn editor_tag(s: &App, tg: &TagGer, t: Tema, k: &rune_kit::Kit) -> El {
    let o = &s.x.o;
    let mut mover = row().w_full().gap(6.0).flex_wrap().center_y().child(comum::rotulo("mover para", t).pr(4.0));
    for c in o.painel.categorias.iter().filter(|c| c.da_marca) {
        let cs = c.slug.clone();
        let atual = c.tags.iter().any(|x| x.slug == tg.slug);
        mover = mover.child(pastilha(format!("mv-{}-{}", tg.slug, c.slug), c.nome.clone(), atual, t,
                                     move |s: &mut App| s.tag_mover(&cs)));
    }
    mover = mover.child(pastilha(format!("mv-{}-nenhuma", tg.slug), "sem categoria".into(), false, t,
                                 |s: &mut App| s.tag_mover("")));
    col().key(format!("ed-{}", tg.slug)).w_full().gap(8.0).padding(10.0).radius(8.0).fill(t.card2).border(1.0, t.linha_forte)
        .child(row().w_full().gap(8.0).center_y()
            .child(text(format!("{} · slug {} · origem {} · {} mídia(s), {} corte(s)", tg.nome, tg.slug, tg.origem, tg.midias, tg.cortes))
                .font(tema::PEQ).color(t.dim).wrap().grow(1.0).min_w(0.0)))
        .child(row().w_full().gap(8.0).center_y()
            .child(TextInput::new(&o.tag_nome).placeholder("nome visível")
                .on_edit(|s: &mut App| &mut s.x.o.tag_nome)
                .on_submit(|s: &mut App, _| s.tag_renomear())
                .build(k).key(format!("tag-nome-{}", tg.slug)).grow(1.0).min_w(0.0))
            .child(botao(format!("tag-ren-{}", tg.slug), "renomear", t.acento, false, t, |s: &mut App| s.tag_renomear())))
        .child(mover)
        .child(row().w_full().gap(8.0).center_y()
            .child(TextInput::new(&o.fundir_em).placeholder("fundir em… (nome ou slug da outra tag)")
                .on_edit(|s: &mut App| &mut s.x.o.fundir_em)
                .build(k).key(format!("tag-fundir-{}", tg.slug)).grow(1.0).min_w(0.0))
            .child(botao(format!("tag-fun-{}", tg.slug), "fundir", t.alerta, false, t, |s: &mut App| s.tag_remover(true)))
            .child(botao(format!("tag-apg-{}", tg.slug), "apagar com as marcações", t.erro, false, t, |s: &mut App| s.tag_remover(false))))
}

/// Coluna da direita: nova categoria, criar em lote e importar tags de arquivos.
fn lado(s: &App, t: Tema, k: &rune_kit::Kit) -> El {
    let o = &s.x.o;
    let mut cats = row().w_full().gap(6.0).flex_wrap()
        .child(pastilha("lote-sem".into(), "sem categoria".into(), o.lote_categoria.is_empty(), t,
                        |s: &mut App| s.x.o.lote_categoria.clear()));
    for c in o.painel.categorias.iter().filter(|c| c.da_marca) {
        let cs = c.slug.clone();
        cats = cats.child(pastilha(format!("lote-{}", c.slug), c.nome.clone(), o.lote_categoria == c.slug, t,
                                   move |s: &mut App| s.x.o.lote_categoria = cs.clone()));
    }
    col().key("ger-lado").w(330.0).h_full().padding(18.0).gap(10.0).fill(t.card).border_sides([0.0, 0.0, 0.0, 1.0], t.linha)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(comum::rotulo("nova categoria", t))
        .child(row().w_full().gap(8.0).center_y()
            .child(TextInput::new(&o.nova_categoria).placeholder("ex.: Equipe")
                .on_edit(|s: &mut App| &mut s.x.o.nova_categoria)
                .on_submit(|s: &mut App, _| s.categoria_criar())
                .build(k).key("ger-nova-cat").grow(1.0).min_w(0.0))
            .child(botao("ger-criar-cat", "criar", t.acento, false, t, |s: &mut App| s.categoria_criar())))
        .child(el().h(6.0))
        .child(comum::rotulo("criar tags em lote", t))
        .child(text("Uma por linha: \"Categoria: Nome\", \"slug = Nome\" ou só o nome. A categoria da linha vale mais que a escolhida abaixo.")
            .font(tema::PEQ.lh(16.0)).color(t.dim).wrap())
        .child(rune_kit::TextArea::new(&o.lote).width(None).auto_grow(4, 10)
            .placeholder("Lugar: Estádio\nMomento: Treino\nequipe-a = Equipe A")
            .on_edit(|s: &mut App| &mut s.x.o.lote)
            .build(k).key("ger-lote").w_full())
        .child(cats)
        .child(row().w_full().child(spacer())
            .child(botao("ger-criar-lote", "criar as tags", t.acento, false, t, |s: &mut App| s.tags_criar_lote())))
        .child(el().h(6.0))
        .child(comum::rotulo("Lightroom e metadados", t))
        .child(text("Para importar keywords do Lightroom (.xmp e catálogo .lrcat) e dos metadados das fotos e vídeos, \
                     selecione as mídias na fila de revisão ou no acervo e use \"importar tags de arquivos\" no painel lateral.")
            .font(tema::PEQ.lh(16.0)).color(t.dim).wrap())
        .child(row().w_full().gap(6.0).flex_wrap().pt(4.0)
            .child(legenda("marca", t)).child(legenda("pasta", t)).child(legenda("lightroom", t))
            .child(legenda("metadados", t)).child(legenda("livre", t)))
}

fn legenda(origem: &str, t: Tema) -> El {
    row().gap(5.0).center_y()
        .child(el().size(7.0, 7.0).radius(4.0).fill(cor_origem(origem, t)))
        .child(text(origem.to_string()).font(tema::PEQ).color(t.dim))
}

/// Janela "Importar tags de arquivos": fontes, prévia do mapeamento e conflitos de slug.
pub fn janela_importar_tags(s: &App, t: Tema, k: &rune_kit::Kit) -> El {
    let o = &s.x.o;
    let Some(v) = o.janela_tags else { return el() };
    let n = s.vista(v).selecao.len();
    let fontes = row().w_full().gap(6.0).flex_wrap()
        .child(pastilha("it-xmp".into(), "sidecar .xmp do Lightroom".into(), o.xmp, t, |s: &mut App| { s.x.o.xmp = !s.x.o.xmp; s.x.o.previa = None; }))
        .child(pastilha("it-emb".into(), "XMP/IPTC dentro do arquivo".into(), o.embutidos, t,
                        |s: &mut App| { s.x.o.embutidos = !s.x.o.embutidos; s.x.o.previa = None; }))
        .child(pastilha("it-est".into(), "estrelas e cor viram tag".into(), o.estrelas, t,
                        |s: &mut App| { s.x.o.estrelas = !s.x.o.estrelas; s.x.o.previa = None; }));
    let mut painel = col().key("janela-tags").w(680.0).max_h(720.0).gap(10.0).padding(20.0).radius(12.0).fill(t.card)
        .border(1.0, t.linha_forte).on_press(|_: &mut App| {})
        .child(row().w_full().center_y()
            .child(text("Importar tags de arquivos").font(tema::TITULO).color(t.txt))
            .child(spacer())
            .child(botao("it-fechar", "fechar", t.dim, false, t, |s: &mut App| s.x.o.janela_tags = None)))
        .child(text(format!("{n} mídia(s) selecionada(s). Os arquivos .xmp, o catálogo .lrcat e as mídias são só lidos, nunca \
            alterados. Hierarquia \"Categoria|Tag\": o primeiro nível vira a categoria e o último, a tag. Título e legenda \
            entram na busca, não viram tag."))
            .font(tema::PEQ.lh(16.0)).color(t.dim).wrap())
        .child(comum::rotulo("fontes", t))
        .child(fontes)
        .child(comum::rotulo("catálogo do Lightroom (opcional)", t))
        .child(TextInput::new(&o.lrcat).placeholder("~/Pictures/Lightroom/Catálogo.lrcat (lido numa cópia)")
            .on_edit(|s: &mut App| &mut s.x.o.lrcat)
            .build(k).key("it-lrcat").w_full())
        .child(row().w_full().gap(8.0)
            .child(spacer())
            .child(botao("it-previa", if o.lendo { "lendo os arquivos…" } else { "ver a prévia" }, t.acento, o.lendo, t,
                         |s: &mut App| { if !s.x.o.lendo { s.importar_tags_previa() } })));
    if let Some(p) = &o.previa {
        painel = painel.child(el().h(1.0).w_full().fill(t.linha))
            .child(text(format!("{} de {} mídia(s) têm dados · {} tag(s) · {} título(s)/legenda(s){}", p.com_dados, p.midias,
                                p.tags.len(), p.textos,
                                if p.categorias_novas.is_empty() { String::new() }
                                else { format!(" · categorias novas: {}", p.categorias_novas.join(", ")) }))
                .font(tema::PEQ_M).color(t.txt).wrap());
        let mut linhas = col().key("it-linhas").w_full().gap(4.0).max_h(380.0).scroll(ScrollSpec { stick_bottom: false });
        if p.tags.is_empty() {
            linhas = linhas.child(text("Nenhuma keyword, cidade, país ou coleção encontrada nas fontes escolhidas.")
                .font(tema::PEQ).color(t.dim));
        }
        for tg in &p.tags {
            let ignorada = o.ignorar.contains(&tg.slug);
            let (sl1, sl2, sl3) = (tg.slug.clone(), tg.slug.clone(), tg.slug.clone());
            let separar = o.escolhas.get(&tg.slug).map(String::as_str) == Some("separar");
            let mut l = row().key(format!("it-{}-{}", tg.slug, tg.origem)).w_full().h(30.0).gap(8.0).px(8.0).center_y()
                .radius(6.0).fill(if tg.estado == "conflito" { t.alerta.alpha(0.08) } else { t.card2 })
                .child(row().key(format!("it-on-{}", tg.slug)).size(18.0, 18.0).center().radius(4.0)
                    .border(1.0, if ignorada { t.linha_forte } else { t.acento })
                    .fill(if ignorada { Color::TRANSPARENT } else { t.acento.alpha(0.2) }).cursor(Cursor::Pointer)
                    .child(text(if ignorada { "" } else { "✓" }).font(tema::MINI).color(t.acento))
                    .on_press(move |s: &mut App| s.previa_alternar(&sl1)))
                .child(text(tg.nome.clone()).font(tema::PEQ_M).color(if ignorada { t.fraco } else { t.txt }))
                .child(text(if tg.categoria_nome.is_empty() { "sem categoria".to_string() } else { tg.categoria_nome.clone() })
                    .font(tema::PEQ).color(t.dim))
                .child(if tg.categoria_nova { etiqueta("categoria nova", t.rep) } else { el() })
                .child(etiqueta(tg.origem.clone(), cor_origem(&tg.origem, t)))
                .child(spacer())
                .child(text(format!("{} mídia(s)", tg.n)).font(tema::PEQ).color(t.dim));
            l = match tg.estado.as_str() {
                "nova" => l.child(etiqueta("nova", t.ok)),
                "existe" => l.child(etiqueta("já existe", t.dim)),
                _ => {
                    let de = tg.existente.as_ref().map(|e| e.origem.clone()).unwrap_or_default();
                    l.child(text(format!("já existe ({de})")).font(tema::PEQ).color(t.alerta))
                        .child(seg(format!("it-esc-{}", tg.slug), vec![
                            seg_item(format!("it-agr-{}", tg.slug), "agrupar", !separar, t, move |s: &mut App| s.previa_escolher(&sl2, false)),
                            seg_item(format!("it-sep-{}", tg.slug), "separar", separar, t, move |s: &mut App| s.previa_escolher(&sl3, true)),
                        ], t))
                }
            };
            linhas = linhas.child(l);
        }
        painel = painel.child(linhas)
            .child(text("Conflito: o mesmo nome já existe com outra origem (uma tag de pasta, por exemplo). Agrupar usa uma tag só; \
                         separar cria outra, com a origem no nome.").font(tema::PEQ.lh(16.0)).color(t.fraco).wrap())
            .child(row().w_full().gap(8.0)
                .child(spacer())
                .child(botao("it-cancelar", "cancelar", t.dim, false, t, |s: &mut App| s.x.o.janela_tags = None))
                .child(botao("it-gravar", if o.gravando { "gravando…" } else { "gravar as tags" }, t.ok, o.gravando, t,
                             |s: &mut App| { if !s.x.o.gravando { s.importar_tags_gravar() } })));
    }
    camada("camada-tags", painel, |s: &mut App| s.x.o.janela_tags = None)
}

/// Janela "Agrupar em projeto": o nome do grupo; as mídias vêm da seleção (ou da pasta aberta).
pub fn janela_grupo(s: &App, t: Tema, k: &rune_kit::Kit) -> El {
    let o = &s.x.o;
    let Some(v) = o.janela_grupo else { return el() };
    let vm = s.vista(v);
    let n = vm.selecao.len();
    let origem = if n > 0 { format!("{n} mídia(s) selecionada(s), de uma ou várias pastas") }
        else if !vm.filtros.subpasta.is_empty() { format!("a pasta aberta: {}", vm.filtros.subpasta) }
        else { "nada selecionado: selecione mídias ou abra uma pasta".to_string() };
    let painel = col().key("janela-grupo").w(520.0).gap(12.0).padding(20.0).radius(12.0).fill(t.card).border(1.0, t.linha_forte)
        .on_press(|_: &mut App| {})
        .child(row().w_full().center_y()
            .child(text("Agrupar em projeto").font(tema::TITULO).color(t.txt))
            .child(spacer())
            .child(botao("gr-fechar", "fechar", t.dim, false, t, |s: &mut App| s.x.o.janela_grupo = None)))
        .child(text(format!("Vai para o grupo: {origem}. O grupo aparece na aba Roteiros, com o campo de roteiro dele: a IA usa só \
            as falas e as imagens do grupo. Nada é copiado ou movido; a mesma mídia pode estar em vários grupos.")).font(tema::PEQ.lh(16.0))
            .color(t.dim).wrap())
        .child(comum::rotulo("nome do grupo", t))
        .child(TextInput::new(&o.grupo_nome).placeholder("ex.: Chegada no primeiro dia")
            .on_edit(|s: &mut App| &mut s.x.o.grupo_nome)
            .on_submit(|s: &mut App, _| s.grupo_criar())
            .build(k).key("gr-nome").w_full())
        .child(row().w_full().gap(8.0)
            .child(spacer())
            .child(botao("gr-cancelar", "cancelar", t.dim, false, t, |s: &mut App| s.x.o.janela_grupo = None))
            .child(botao("gr-criar", "criar o grupo", t.acento, false, t, |s: &mut App| s.grupo_criar())));
    camada("camada-grupo", painel, |s: &mut App| s.x.o.janela_grupo = None)
}

fn camada(chave: &str, painel: El, fechar: impl Fn(&mut App) + 'static) -> El {
    col().key(chave).w_full().h_full().absolute(0.0, 0.0).fill(Color::BLACK.alpha(0.45))
        .on_press(fechar)
        .child(spacer())
        .child(row().w_full().child(spacer()).child(painel).child(spacer()))
        .child(spacer())
}
