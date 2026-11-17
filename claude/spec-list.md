# Spec list - Painel NG3 fase import voz roteiros

Esta spec lista as telas e fluxos da fase descrita em `docs/08-plano-fase-import-voz-roteiros.md`, como base para o design e o handoff. Atualizada em 2026-11-13 com o mockup navegável.

Fonte visual: `claude/mockup-painel-ng3.html` (protótipo HTML navegável, tema escuro, dados fictícios, sem funcionalidade). Abrir no navegador; as abas da lateral levam às telas abaixo.

O mockup ainda não tem as telas de Lightroom e metadados ("Importar tags de arquivos", opção de ler `.xmp` no "Importar pasta", origem `lightroom`/`metadados` nas pastilhas e no gerenciador). As partes de IA de imagem que estavam nele foram retiradas em 2026-11-13.

## Objetivo e escopo

A fase deixa o painel pronto para importar mídia com tags, marcar cortes, editar roteiros como documento de trabalho e sincronizar com o FilmCraft. As quatro branches cobertas são `fase/tags-import`, `fase/cortes`, `fase/roteiros-abertos` e `fase/filmcraft-sync`.

Fica fora desta spec a separação de voz em si (`fase/separacao-voz`): ela não tem tela nova e entra aqui só como dependência do preview de cortes.

Regras que valem para todas as telas: cortes e tags são anotações em `dados/`, nunca arquivos novos; nada do acervo é apagado, sobrescrito ou criado; um `.fcproj` só é gravado com cópia prévia e validação por `filmcraft-cli inspect`; arquivos `.xmp`, catálogos `.lrcat` e metadados das imagens são só lidos, nunca alterados.

## Removido do roadmap (2026-11-13)

Saiu a IA que reconhece imagens e vídeos. Isso inclui a classificação de vídeo em tags (V-JEPA), a sugestão de tags por IA com aceite em um clique (`ia_sugerida` e `ia_aceita`), a criação de tag nova por IA quando o classificador acha uma descrição melhor, a lista de recomendações da IA no gerenciador de tags e o limiar de novidade para promover tag livre.

Continuam no escopo as IAs que trabalham com texto e áudio: transcrição, ideia de roteiro e "reescrever parte". A busca de b-roll segue por tag, legenda e descrição. Os cortes automáticos por fala e por cena continuam como propostas ajustáveis.

Tags passam a vir de quatro origens: subpasta no import, arquivos do Lightroom, metadados da mídia e marcação manual.

## Telas

As telas novas e as alteradas, com a branch que as entrega e o arquivo de UI onde nascem. Todas existem no mockup.

| Tela | Tipo | Branch | Função principal | Arquivo de UI |
| --- | --- | --- | --- | --- |
| Fila de revisão do import | alterada | fase/tags-import | Grade com seleção múltipla, miniatura, duração e status; filtros por importação, passo e subpasta; alternância Vídeos e Imagens | importar.rs, grade.rs |
| Importar pasta | nova (janela) | fase/tags-import | Árvore da pasta; cada subpasta vira tag; opção de ler `.xmp` e metadados | importar.rs |
| Painel de tags do import | nova | fase/tags-import | Pastilhas por categoria com a origem de cada tag (pasta, Lightroom, metadados, manual); campo livre com autocompletar; aplicar a N selecionados | importar.rs |
| Importar tags de arquivos | nova (janela) | fase/tags-import | Escolher fontes (`.xmp`, `.lrcat`, IPTC e EXIF), o que importar e ver a prévia do mapeamento antes de gravar | importar.rs |
| Imagens | alterada | fase/tags-import | Grade própria dentro do Importar; tag só no arquivo inteiro, sem cortes | importar.rs, grade.rs |
| Gerenciador de tags da marca | nova | fase/tags-import | Criar, renomear, remover ou fundir tags; criar em lote; contagem por categoria; coluna de origem; lista de tags sem categoria; fica dentro da aba Importar | importar.rs |
| Biblioteca de vídeos | nova | fase/cortes | Lista de todos os vídeos com busca, abrir um vídeo, seleção múltipla e filtro por qualquer tipo de tag (vídeo, corte ou ambos; categorias; subpastas; livres; Lightroom e metadados; tipo de corte) | preview.rs |
| Preview com cortes | alterada | fase/cortes | Tira de quadros com entrada e saída, cortes propostos ajustáveis, tags e nota por corte; tags do vídeo inteiro separadas das tags do corte | preview.rs |
| Cortes dos selecionados | nova | fase/cortes | Cortes de vários vídeos juntos, agrupados por vídeo, com aceitar propostos, aplicar tag e usar no roteiro | preview.rs |
| Roteiro aberto | alterada | fase/roteiros-abertos | Todo texto editável; lista de roteiros com status; partes livres (fala, b-roll, motion, texto na tela, pausa, parte livre); texto novo; nota de direção; fixar parte; reescrever só uma parte | roteiros.rs |
| Vídeo anexado no roteiro | nova | fase/roteiros-abertos | Anexar vídeo que já tem fala e texto: fala transcrita editável, texto que aparece no vídeo digitado, manter áudio original, legendar a fala | roteiros.rs |
| Versões do roteiro | nova | fase/roteiros-abertos | Lista de versões v1 a vn, comparar e voltar a uma versão | roteiros.rs |
| Aprovar e gerar projeto | nova (janela) | fase/roteiros-abertos | Modelo e formato (16:9, 9:16, 1:1); faixas a gerar (b-roll, motion, SFX, legenda, texto na tela); partes incluídas; checklist "Pronto para aprovar?" | roteiros.rs |
| Projetos e sincronia | alterada | fase/filmcraft-sync | Mostrar edição externa no card do projeto, listar diferenças e aplicar nos dois sentidos com confirmação; ao salvar com cortes ou posições alterados, refazer as legendas sobre as novas posições | lista.rs |
| Editor do projeto | nova | fase/filmcraft-sync | Parecido com o preview de cortes: roteiro à esquerda, player e timeline de seis faixas no centro, recomendações do momento à direita (b-roll, motion, SFX) | nova (projeto.rs) |
| Abrir no FilmCraft | ação | fase/filmcraft-sync | Abrir o projeto no tempo da parte, se o CLI aceitar `--seek`; senão só abrir o projeto | roteiros.rs |

A branch `fase/separacao-voz` não aparece: não tem tela nova. O Editor do projeto usa a busca de b-roll, o motion e o SFX descritos mais abaixo; essas partes dependem das três branches propostas no fim da seção de edição.

## Fluxos

Do import até o FilmCraft, cada vídeo passa por revisão, tags e cortes antes de o roteiro virar projeto. Depois da aprovação, a mudança volta pela sincronia, com confirmação de cada lado.

Fluxo resumido (8 passos, 1 ida e volta):

1. Importar pasta: só aponta, não copia; subpastas viram tags
2. Fila de revisão: fica fora do índice
3. Tags: pasta, Lightroom e metadados, e marcação manual
4. Cortes: fala, cena, manual; tags próprias por corte
5. Roteiro aberto: partes editáveis, com vídeo anexado e texto novo
6. Aprovar: gera o projeto com as opções escolhidas
7. FilmCraft: editar o `.fcproj`
8. Sincronia: confirma cada lado

Depois de aprovado, cada mudança passa pela sincronia, nunca por regerar o projeto inteiro. A fila de revisão pode ser filtrada por importação, por passo (Importado, Fila de revisão, Com tags, Com cortes, No roteiro) e por subpasta.

## Estados e dados

A mídia passa por estados em ordem fixa. A separação de voz é opcional, por isso aparece entre parênteses.

1. `novo`: arquivo apontado, ainda fora do índice
2. `indexado`: entrou no índice com tipo, duração e hash rápido
3. `(separado)`: voz isolada, só quando há música de fundo
4. `transcrito`: palavras com tempo gravadas
5. `cortado`: cortes automáticos por fala e cena propostos
6. `revisado`: tags conferidas; libera roteiro e b-roll

Imagens não têm transcrição nem cortes: vão de `novo` a `indexado` e a `revisado`.

As tabelas novas ficam em `dados/marcas/<marca>/midia.sqlite`:

| Tabela | Chave principal | Para quê |
| --- | --- | --- |
| `importacao` | id, caminho, data | Cada "importar pasta"; base do filtro por importação |
| `midia` | id, fonte_id, caminho_rel | Um arquivo do acervo (vídeo ou imagem), com `status_import` e `importacao_id` |
| `tag` | id, slug, categoria, origem | Tag da marca ou livre; `origem` é `marca`, `livre`, `pasta`, `lightroom` ou `metadados`; o slug não muda ao renomear |
| `midia_tag` | midia_id, tag_id, por | Tag da mídia inteira. `por`: `humano`, `pasta`, `lightroom` ou `metadados` |
| `corte` | id, midia_id, ini, fim, origem | Anotação de `fala`, `cena` ou `manual`; nunca um arquivo novo |
| `corte_tag` | corte_id, tag_id, por | Tag do corte, independente das tags da mídia |
| `midia_texto` | midia_id, campo, texto, origem | Título e legenda lidos dos metadados, usados na busca de b-roll |
| `grupo` | id, nome, criado_em, projeto_id | Mídias agrupadas (de uma ou várias pastas) que viram projeto, com roteiro próprio |
| `grupo_midia` | grupo_id, midia_id | Mídias do grupo; uma mídia pode estar em vários grupos |

Corte manual nunca é sobrescrito por reprocessamento. Cada salvamento de roteiro grava `roteiros/<id>/v<n>.json`, e aprovar um roteiro não o trava. Tag da mídia e tag do corte são níveis separados: mudar uma não muda a outra.

## Importação de tags do Lightroom e dos metadados

A janela "Importar tags de arquivos" lê as fontes abaixo, mostra a prévia do mapeamento e só grava depois de confirmar. Pode ser aberta no Importar e no gerenciador de tags.

| Fonte | O que lê | Vira |
| --- | --- | --- |
| Sidecar `.xmp` do Lightroom | Keywords e keywords hierárquicas (por exemplo `Local\|Camp Nou`) | Tag; a hierarquia vira categoria e tag, com origem `lightroom` |
| Catálogo `.lrcat` | Keywords e coleções (cópia em `/tmp`, leitura apenas) | Tag com origem `lightroom` |
| IPTC, XMP e EXIF embutidos | Keywords, cidade, país, GPS | Tag de Local com origem `metadados` |
| IPTC, XMP e EXIF embutidos | Título e legenda | Texto de busca em `midia_texto`, não tag |
| Estrelas e rótulo de cor | Avaliação e cor | Tag só se a opção estiver marcada (desligada por padrão) |

Tag que já existe na marca é reaproveitada pelo slug; tag nova aparece marcada como "nova" na prévia e entra sem categoria quando o arquivo não traz hierarquia. Vale para imagens e, quando o arquivo tiver metadados, para vídeos.

## Critérios de aceite por branch

Cada branch só fecha com os testes passando antes e depois: `python3 -m unittest discover -s testes`.

**fase/tags-import**

- [ ] Import novo cai na fila de revisão, não direto no índice
- [ ] Fila filtra por importação, por passo e por subpasta
- [ ] Nome da subpasta vira tag com origem `pasta`
- [ ] Arquivo movido é reconhecido pelo hash rápido, sem reprocessar
- [ ] Imagens e vídeos ficam separados na mesma aba; imagem tem tag só no arquivo
- [ ] Aplicar tag a N selecionados de uma vez
- [ ] Importar tags de `.xmp`, `.lrcat` e metadados mostra a prévia do mapeamento antes de gravar
- [ ] Tag importada guarda a origem (`lightroom` ou `metadados`) e aparece com ela no painel e no gerenciador
- [ ] Arquivos `.xmp`, `.lrcat` e originais nunca são alterados
- [ ] Hierarquia `Categoria|Tag` vira categoria e tag
- [ ] Conflito de slug entre origens oferece agrupar ou manter separadas, com agrupar como padrão
- [ ] Grupos convivem com o "projeto de uma pasta"
- [ ] Mídias de pastas diferentes podem ser agrupadas; o grupo aparece na lista de projetos com o próprio campo de roteiro, que usa só o conteúdo do grupo
- [ ] Tag renomeada mantém o slug e as marcações
- [ ] Remover tag pede destino: apagar as marcações ou fundir

**fase/cortes**

- [ ] Corte automático nasce proposto; ajustar a borda não altera o arquivo do acervo
- [ ] Corte manual não é sobrescrito quando o processamento roda de novo
- [ ] `I` e `O` marcam entrada e saída no preview
- [ ] Tag do corte é independente da tag do vídeo; é possível copiar a tag do vídeo para o corte
- [ ] Biblioteca lista todos os vídeos, abre um específico e permite selecionar vários
- [ ] Filtro da biblioteca aceita todos os tipos de tag, em nível de vídeo, de corte ou ambos
- [ ] Cortes dos selecionados mostra os cortes de vários vídeos e aplica ações em lote

**fase/roteiros-abertos**

- [ ] Roteiro começa da ideia da IA, de texto livre, de cortes escolhidos ou de um vídeo anexado
- [ ] Todo texto do roteiro é editável na própria tela
- [ ] É possível anexar um vídeo com fala e texto, editar a fala transcrita e digitar o texto que aparece no vídeo
- [ ] É possível criar partes de texto novas e de qualquer tipo
- [ ] Parte fixa não muda com "reescrever parte"
- [ ] Cada salvamento cria versão, e é possível voltar a uma versão anterior
- [ ] Aprovar abre as opções (modelo, formato, faixas, partes) antes de gerar o projeto
- [ ] Roteiro aprovado continua editável

**fase/filmcraft-sync**

- [ ] Edição externa do `.fcproj` aparece no card do projeto, por `mtime` e hash
- [ ] Nenhuma direção de sincronia grava sem confirmação
- [ ] Gravação do `.fcproj` tem cópia prévia e passa por `inspect`; se recusar, a cópia volta
- [ ] Projeto nunca é regerado inteiro se já foi editado à mão
- [ ] Salvar o projeto com cortes, posições ou ordem alterados refaz as legendas sobre as novas posições
- [ ] Editor do projeto mostra recomendações de b-roll, motion e SFX por momento do roteiro

## Busca de b-roll e motion

A busca já existe em parte, pela linha de comando. Falta a tela no painel; no mockup ela aparece dentro do Editor do projeto.

| Parte | Hoje | Falta |
| --- | --- | --- |
| Busca de b-roll | `achar/indice.py`, índice FTS5 em `broll.sqlite`, só pela CLI | Tela no painel com filtros e miniaturas |
| Candidatos por momento | `achar/broll.py candidatos`, até 2 do mesmo item, IA opcional | Trocar candidato e aprovar na tela |
| Faixa B-roll no projeto | `achar/broll.py inserir`, refeita a cada vez | Pré-visualizar antes de inserir |
| Motion pronto | Biblioteca e plugin `inserir-motion` | Tela de biblioteca com busca por tag |
| Motion novo | `criar/motion_hf.py`: briefing, HyperFrames, render em MOV com alfa | Tela de briefing, prévia e aprovação |

Requisitos da busca:

- Uma caixa de busca com filtros por tag, categoria, fonte (`broll`, `falas`, `posts`, `cortes`) e tipo (vídeo, foto, motion).
- Cada resultado mostra miniatura, duração, origem e o motivo do encontro: tag, legenda ou descrição. Título e legenda vindos dos metadados entram na busca.
- Fontes marcadas como `referencias` nunca aparecem como b-roll; a regra vem de `src/acervo/fontes.py`.
- A sugestão por momento usa a fala e o briefing; a pessoa troca, aprova ou descarta.

## Efeitos sonoros (SFX)

O modelo do FilmCraft já põe três SFX fixos: whoosh na abertura e no título, notification e riser. O SFX antes do ending fica em 2,2 s (`projetos/modelo.py`). Falta o SFX ligado a cada entrada e saída de b-roll e motion.

- Biblioteca de SFX em `assets/`, por marca e padrão, com tags (whoosh, impacto, riser, notificação, transição), duração e volume padrão.
- Ao aprovar b-roll ou motion, o painel sugere SFX pela tag do momento e pelo tipo de entrada; a pessoa confirma ou troca.
- Ajustes por SFX: volume, atraso em relação ao corte, fade in e fade out.
- O SFX fica em trilha própria no projeto; regravar o projeto não apaga SFX colocado à mão.
- O arquivo da biblioteca nunca é alterado; o SFX de cada momento é anotação em `dados/`.

## Edição de vídeo completa

Os itens abaixo cobrem uma edição completa. O que tem "(confirmar)" depende do que o `filmcraft-cli` ou o modelo `.fcproj` suporta. O Editor do projeto do mockup é o ponto de partida: as seis faixas (vídeo principal, B-roll, motion, legenda, texto na tela e SFX) já aparecem na timeline.

**Corte e montagem**

- Marcar entrada e saída, dividir, juntar e apagar com fechamento da lacuna (ripple).
- Aparar pelas palavras da fala, usando os tempos da transcrição.
- Reordenar clipes; desfazer e refazer (undo/redo) em todas as ações.

**Camadas e transições**

- Faixas separadas para vídeo principal, B-roll, motion, legenda, texto na tela e SFX, com ordem de camadas.
- Transições de corte seco, fade e dissolve, com duração editável.
- Velocidade do clipe, lenta e rápida (confirmar).

**Áudio**

- Volume por clipe e por trilha, fade in e fade out, mudo.
- Música abaixa sob a fala (ducking), com nível definido na marca.
- Normalização (já existe no plugin `normalizar-audio`) e separação de voz da branch `fase/separacao-voz`.

**Imagem e enquadramento**

- Posição, zoom e enquadramento de cada clipe (confirmar; o modelo já lê o efeito `motion`).
- Opacidade e correção básica de brilho, contraste e saturação (confirmar).

**Texto e legenda**

- Editar texto, tempo e estilo da legenda, com no máximo 4 palavras por cartela (regra de `projeto-filmcraft.toml`).
- Texto na tela como parte do roteiro, com entrada e saída.

**Prévia e exportação**

- Tocar, pausar, buscar no tempo, tocar só uma parte e prévia de áudio.
- Exportar para `saidas/` e registrar o estado da exportação no projeto.

Proposta de três branches novas, depois de `fase/roteiros-abertos`: `fase/broll-motion` (busca e motion), `fase/sfx` (biblioteca e sugestão) e `fase/edicao` (recursos de edição). Aprovadas em 2026-11-17.

## Perguntas em aberto

Estas decisões dependem do Lucas ou de uma investigação antes do desenvolvimento:

1. Com quantas mídias importadas revisar as tags livres para virar categoria da marca? O plano sugere cerca de 50.
2. O corte por fala agrupa por pausa ou por outro critério sem IA de imagem? O plano começa por pausa.
3. O stem de música é descartado depois da transcrição? O padrão é descartar.
4. O `filmcraft-cli` aceita `--seek` para abrir no tempo da parte? Se não, a ação só abre o projeto.
5. Qual campo do clipe o FilmCraft preserva ao salvar, para guardar o marcador `parte_id`? Confirmar com `filmcraft-cli inspect`.
6. ~~Gerenciador de tags: aba nova ou dentro do Importar?~~ Decidido em 2026-11-17: dentro do Importar.
7. ~~Lightroom: `.xmp` ou também `.lrcat`?~~ Decidido em 2026-11-17: os dois.
8. ~~Separador de hierarquia?~~ Decidido em 2026-11-17: `Categoria|Tag`.
9. Estrelas e rótulos de cor viram tag? O mockup deixa desligado por padrão.
10. ~~Mesmo slug de pasta e de arquivo?~~ Decidido em 2026-11-17: a pessoa escolhe agrupar ou manter separadas (padrão: agrupar); além disso, mídias de pastas diferentes podem ser agrupadas num projeto com roteiro próprio (`docs/08`, seção 2).
11. ~~Qual biblioteca lê IPTC, XMP e EXIF?~~ Decidido em 2026-11-17: o `acervo/reconhecer.py`, se der conta de tudo; senão ExifTool. Falta saber quais formatos de vídeo trazem metadados úteis.
12. O texto que já aparece num vídeo anexado ao roteiro é digitado pela pessoa. Sem reconhecimento de imagem, está bom assim?
13. Um grupo pode ter mais de um roteiro ao mesmo tempo, ou cada grupo é sempre um projeto só?

## Fontes

- `claude/mockup-painel-ng3.html`, mockup navegável com as telas desta spec (2026-11-13)
- `docs/08-plano-fase-import-voz-roteiros.md`, plano da fase com decisões de 2026-11-13
- `CLAUDE.md`, regras do projeto: acervo somente leitura, `.fcproj` com cópia e validação, sem nome de marca no código
- `app/src/ui/*.rs`, abas atuais usadas para o inventário das telas (importar, preview, roteiros, grade, lista, plugins, tema)
- Código existente citado nas seções de busca, SFX e edição: `src/achar/broll.py`, `src/achar/indice.py`, `src/criar/motion_hf.py`, `src/projetos/modelo.py`, `plugins/inserir-motion`, `config/padroes/projeto-filmcraft.toml`
