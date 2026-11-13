# 08 — Plano da fase: tags no import, cortes, voz sem música, roteiros abertos, FilmCraft

Telas, fluxos e critérios de aceite de cada branch: `claude/spec-list.md`.
Protótipo navegável das telas: `claude/mockup-painel-ng3.html` (abrir no navegador).

Decisões tomadas com o Lucas (2026-11-13):

- **Tags**: lista fixa por marca (`config/marcas/<marca>/tags.toml`) **+ tags livres**. Vêm de
  quatro origens: **subpasta** no import, arquivos do **Lightroom**, **metadados** da mídia e
  marcação **manual**. Nenhuma IA reconhece imagem ou vídeo para sugerir tag.
- **Cortes**: mistura de três origens — frases da fala, mudança de cena e marcação manual — e
  são **só anotações** (entrada/saída + tags), nunca arquivos novos.
- **Separação voz/música**: automática quando houver música de fundo; dá para forçar por fonte.
- **Roteiros**: menos engessados, com espaço real para input humano.
- **Projetos**: edição integrada ao FilmCraft (ida e volta), não só geração.

**Voltou ao escopo (2026-11-13, pedido do Lucas): IA que olha a imagem, só para sugerir.**
`src/acervo/reconhecer.py` (ambiente `sep`): lê os metadados de cada mídia (data, câmera, GPS,
rotação); GPS dentro do raio de um lugar de `config/marcas/<marca>/lugares.toml` vira tag direto
(por "metadados"); o SigLIP multilíngue compara quadros/fotos com as tags de lugar, momento e plano
(frases em `[ia.*]` do `tags.toml`) e grava **sugestões pendentes** — a pessoa aceita ou recusa no
painel, e recusada não volta. Sem "pessoa" (a IA não reconhece gente), sem evento e qualidade (não
se veem na imagem); com GPS, a IA não sugere outro lugar. Config: `config/padroes/reconhecimento.toml`.

**Removido do roadmap (2026-11-13, antes da volta acima):** a IA que reconhece imagens e vídeos — classificação com
V-JEPA 2 + SigLIP, sugestão de tag com aceite em um clique (`ia_sugerida`/`ia_aceita`), tag nova
proposta por IA, lista de recomendações da IA no gerenciador e o limiar de novidade. Continuam
as IAs de texto e áudio: transcrição, ideia de roteiro e "reescrever parte".

Ordem de trabalho (cada uma num branch, com commit antes):

| # | branch | depende de | entrega |
|---|---|---|---|
| 1 | `fase/separacao-voz` | — | transcrição melhor já no próximo import |
| 2 | `fase/tags-import` | — | modelo de dados de mídia/corte/tag, aba importar nova, tags de Lightroom e metadados, gerenciador de tags |
| 3 | `fase/cortes` | 1, 2 | cortes automáticos (fala + cena) e manuais, biblioteca de vídeos |
| 4 | `fase/roteiros-abertos` | 2, 3 | roteiro editável por humano, versões, aprovar com opções |
| 5 | `fase/filmcraft-sync` | 4 | ler o `.fcproj` de volta, sincronia com confirmação, editor do projeto |

Propostas depois da 4, **ainda sem aprovação**: `fase/broll-motion` (busca de b-roll e motion
no painel), `fase/sfx` (biblioteca e sugestão de SFX) e `fase/edicao` (recursos de edição).
Detalhes em `claude/spec-list.md`.

---

## 1. Separação de voz antes de transcrever

**Onde entra:** `src/transcricao/legendar.py::transcrever`, entre `audio_do_video()` e o
`faster-whisper`. Novo módulo `src/transcricao/separar.py`, rodando no `.venv-sep/`.

**Fluxo**

1. `audio_do_video()` → wav 16 kHz mono (já existe).
2. `tem_musica(wav)` — detecção barata, sem GPU: energia fora da faixa de voz + proporção de
   quadros "tonais" fora das regiões de voz do VAD (`voz.analisar` já dá as regiões). Se a
   fração musical passar de `separacao.limiar_musica`, separa. Cache do resultado por hash.
3. `separar(wav)` → `<cache>/sep/<hash>_voz.wav` com [`audio-separator`](https://github.com/nomadkaraoke/python-audio-separator)
   (modelos UVR/MDX/RoFormer). Modelo padrão: um RoFormer de vocal; alternativa leve
   `UVR-MDX-NET-Voc_FT` se faltar VRAM. Segmentos curtos para caber nos 4 GB; CPU como reserva.
4. O whisper e o `refinar_pela_voz` rodam na **voz isolada**; os tempos continuam valendo para
   o vídeo original (mesma duração, mesma taxa).
5. Grava no `words.json` de que áudio veio (`"fonte_audio": "separada" | "original"`) e o
   modelo, para poder re-transcrever só o que mudou.

**Config:** `config/padroes/separacao.toml` (já criado). Por fonte, em `fontes.json`:
`"separar": "auto" | "sempre" | "nunca"`.

**Ganho extra:** o mesmo stem de voz melhora o silencecut e o VAD; o stem de música pode virar
"trilha original" na montagem (opcional, depois).

**Testes:** `tem_musica` com sinais sintéticos (seno + ruído vs. só ruído modulado);
`separar` com mock do subprocess. Nada de modelo real nos testes.

Não tem tela nova; entra no painel só como dependência do preview de cortes.

## 2. Interface de tags no import

### Modelo de dados (sqlite em `dados/marcas/<marca>/midia.sqlite`)

```
importacao(id, caminho, data)                  -- cada "importar pasta"; filtro por importação
midia(id, fonte_id, importacao_id, caminho_rel, tipo, duracao, largura, altura,
      hash_rapido, status_import)              -- tipo: video | imagem
tag(id, slug, nome, categoria, origem)         -- origem: marca | livre | pasta | lightroom | metadados
midia_tag(midia_id, tag_id, por)               -- por: humano | pasta | lightroom | metadados
corte(id, midia_id, ini, fim, origem, proposto, titulo, nota, criado_em)
                                               -- origem: fala | cena | manual
corte_tag(corte_id, tag_id, por)
midia_texto(midia_id, campo, texto, origem)    -- título e legenda dos metadados (busca, não tag)
```

- `hash_rapido` (tamanho + primeiros/últimos MB) para reconhecer arquivo movido sem reprocessar.
- `status_import`: `novo → indexado → (separado) → transcrito → cortado → revisado`.
  Imagens vão de `novo` a `indexado` e a `revisado` (sem transcrição nem cortes).
- O `slug` da tag não muda ao renomear; só o `nome` visível.
- O índice FTS de `achar/indice.py` ganha colunas de tags, cortes e `midia_texto`
  (busca por "tag:camp-nou").

### Fluxo na aba importar

1. **Importar pasta** (janela): mostra a árvore da pasta; nada é copiado. Cada subpasta vira tag
   com origem `pasta`. Opção de já ler `.xmp` e metadados no mesmo passo.
2. Os arquivos novos caem numa **fila de revisão**, não direto no índice.
3. Fila em grade (reaproveita `ui/grade.rs`): miniatura, duração, status; seleção múltipla.
   Filtros por importação, por passo (Importado, Fila de revisão, Com tags, Com cortes,
   No roteiro) e por subpasta. Alternância **Vídeos / Imagens** na mesma aba; imagem só recebe
   tag no arquivo inteiro.
4. Painel lateral de tags: categorias da marca como pastilhas, cada tag mostrando a origem
   (pasta, Lightroom, metadados, manual), + campo de tag livre com autocompletar; aplicar a N
   selecionados de uma vez.
5. "Marcar como revisado" libera a mídia para roteiros e b-roll.

### Importar tags do Lightroom e dos metadados

Janela "Importar tags de arquivos", aberta no Importar e no gerenciador. Escolhe as fontes,
mostra a **prévia do mapeamento** e só grava depois de confirmar. Arquivos `.xmp`, catálogos
`.lrcat` e os metadados das mídias são **só lidos**, nunca alterados.

| Fonte | O que lê | Vira |
|---|---|---|
| Sidecar `.xmp` do Lightroom | keywords e keywords hierárquicas (`Local\|Camp Nou`) | tag; a hierarquia vira categoria e tag, origem `lightroom` |
| Catálogo `.lrcat` (sqlite, só leitura; desligado por padrão) | keywords e coleções | tag, origem `lightroom` |
| IPTC, XMP e EXIF embutidos | keywords, cidade, país, GPS | tag de Local, origem `metadados` |
| IPTC, XMP e EXIF embutidos | título e legenda | texto em `midia_texto`, não tag |
| Estrelas e rótulo de cor (desligado por padrão) | avaliação e cor | tag, se a opção estiver marcada |

Tag que já existe na marca é reaproveitada pelo slug; tag nova aparece como "nova" na prévia e
entra sem categoria quando o arquivo não traz hierarquia. Vale para imagens e, quando houver
metadados, para vídeos. Leitor dos metadados: a decidir (ver perguntas).

### Gerenciador de tags da marca

Um menu próprio ("tags da marca", `ui/tags.rs`), separado da fila de import:

- Lista de **categorias** com a contagem de entradas (mídias + cortes) em cada uma, e dentro
  dela as tags com a contagem e a **origem** de cada tag. Lista à parte das tags sem categoria.
- **Criar, renomear e remover** categoria ou tag; **criar em lote** (colar uma lista, uma por
  linha, `Categoria: tag` ou só o nome). Renomear muda só o nome visível; o `slug` fica, para
  não quebrar as marcações. Remover pede destino: apagar as marcações ou **fundir** em outra tag.
- Atalho para a janela "Importar tags de arquivos".
- Tudo grava em `config/marcas/<marca>/tags.toml` (escrita atômica, com cópia em `historico/`);
  as contagens vêm do `midia.sqlite`.

## 3. Cortes como anotação

Três geradores, todos escrevendo na tabela `corte` — nunca no disco do acervo:

| origem | como | para |
|---|---|---|
| `fala` | `plano/falas.frases_de()` agrupa frases em ideias por pausa > X s | falas |
| `cena` | PySceneDetect (`ContentDetector`) no proxy 360p, no `.venv-sep` ou no sistema | broll |
| `manual` | entrada/saída no preview (`I`/`O` na tira de quadros), com tags e nota | qualquer |

- Cortes automáticos nascem como **propostos**; no preview a pessoa ajusta borda, junta, divide,
  apaga. Um corte manual nunca é sobrescrito por reprocessamento.
- Tag do vídeo e tag do corte são **níveis separados**: mudar uma não muda a outra. Dá para
  copiar as tags do vídeo para o corte.
- **Biblioteca de vídeos** (`ui/preview.rs`): lista de todos os vídeos com busca, seleção
  múltipla e filtro por qualquer tipo de tag (do vídeo, do corte ou ambos; categorias, subpastas,
  livres, Lightroom e metadados, tipo de corte).
- **Cortes dos selecionados**: cortes de vários vídeos juntos, agrupados por vídeo, com aceitar
  propostos, aplicar tag e usar no roteiro.
- `projetos/gerar` e `roteiros` passam a aceitar `corte_id` onde hoje aceitam vídeo + tempo.

## 4. Roteiros menos engessados

Hoje: a IA escreve tudo, a pessoa só move/remove/troca candidato, e depois de aprovado trava.

Novo modelo — **o roteiro é um documento de trabalho conjunto**:

- **Começar de qualquer ponto:** da ideia da IA, de um texto livre do Lucas (briefing, tópicos),
  de cortes escolhidos à mão ("montar roteiro com estes") ou de um **vídeo anexado**.
- **Todo texto editável** na própria tela; lista de roteiros com status.
- **Partes livres:** adicionar parte em qualquer lugar; tipos `fala` (corte existente), `broll`,
  `motion`, `texto_na_tela`, `pausa` e `livre`, além de **texto novo** (a gravar). Duração editável.
- **Vídeo anexado:** vídeo que já tem fala e texto; a fala transcrita é editável, o texto que
  aparece no vídeo é digitado pela pessoa; opções de manter o áudio original e legendar a fala.
- **Nota de direção por parte** (humano) e **por roteiro**; a IA lê as notas ao reescrever.
- **"Reescrever só esta parte"** / **"sugerir alternativas"**: a IA propõe 3 opções para uma
  parte, mantendo o resto travado. Partes podem ser marcadas como **fixas** (a IA não mexe).
- **Trocar fala por outra parecida:** busca no banco de falas + cortes por tags e texto.
- **Aparar fala:** ajustar entrada/saída da fala em palavras (a transcrição dá os tempos).
- **Versões:** cada salvamento grava `roteiros/<id>/v<n>.json`; dá para comparar e voltar.
- **Aprovar abre opções** antes de gerar: modelo e formato (16:9, 9:16, 1:1), faixas a gerar
  (b-roll, motion, SFX, legenda, texto na tela), partes incluídas e checklist "Pronto para aprovar?".
- **Aprovado não trava:** mudanças depois vão para o projeto pela sincronia (seção 5), com
  aviso quando o projeto foi editado à mão no FilmCraft.

Mudanças em `plano/roteiros.py`: `editar()` ganha `inserir`, `tipo`, `duracao`, `aparar`,
`nota`, `fixar`, `reescrever_parte`, `anexar_video`; formato de arquivo com `versao`. Prompts
em `plano/prompts.py` passam a receber notas e partes fixas.

## 5. Edição integrada com o FilmCraft

Hoje a ida é boa (`gerar`, `fcproj.ganho/legendas`), mas não há volta.

1. **Mapa roteiro ↔ timeline:** ao aprovar, cada clipe gerado recebe no `.fcproj` um marcador
   com `parte_id` (campo de nome/nota do clipe que o FilmCraft preserva — confirmar com
   `filmcraft-cli inspect`). Guardar também em `roteiros/<id>/mapa.json`.
2. **Ler de volta:** `linha_tempo.ler()` já lê o projeto; novo `projetos/sincronia.py` compara
   o projeto atual com o mapa e lista: partes cortadas, reordenadas, aparadas, clipes novos.
3. **Duas direções, sempre com confirmação:**
   - FilmCraft → roteiro: "o projeto mudou; atualizar o roteiro?" (aplica ordem e tempos).
   - roteiro → FilmCraft: aplicar só a parte mudada (trocar clipe, aparar), via `fcproj.salvar`
     com cópia e `inspect`. Nunca regerar o projeto inteiro se ele já foi editado à mão.
4. **Abrir no FilmCraft** a partir do painel, no tempo da parte (se o CLI aceitar `--seek`;
   senão só abrir o projeto).
5. Detectar edição externa por `mtime` + hash do `.fcproj`; mostrar no card do projeto.
6. **Editor do projeto** (`ui/projeto.rs`): roteiro à esquerda, player e timeline de seis faixas
   (vídeo principal, b-roll, motion, legenda, texto na tela, SFX) no centro, recomendações do
   momento à direita (b-roll, motion, SFX).

**A investigar primeiro:** que campos do clipe o FilmCraft preserva ao salvar (para o marcador
`parte_id`) e o que o `filmcraft-cli` oferece além de `probe` e `inspect`.

---

## Perguntas em aberto

A lista completa (12 perguntas) está em `claude/spec-list.md`. As principais:

- Quantas tags livres viram categoria da marca? (revisar a cada ~50 mídias importadas)
- Cortes por fala: agrupar por pausa ou outro critério? (começar por pausa)
- O stem de música deve ser guardado ou descartado depois da transcrição? (padrão: descartar)
- Lightroom: só `.xmp` ou também `.lrcat`? Separador de hierarquia (`Categoria|Tag`)?
  Keyword com o mesmo slug de uma tag de pasta: funde ou fica separada por origem?
- Qual leitor de IPTC/XMP/EXIF (por exemplo `exiftool`) e quais formatos de vídeo trazem
  metadados úteis?
