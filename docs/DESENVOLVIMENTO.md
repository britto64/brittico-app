# Desenvolvimento do Brittico App

O site do Brittico como programa, em Rust (egui), copiando o site tela por tela. Estão prontos o **Início**
(`brittico.xyz/`, logado como Kennzy), a **Call** (`brittico.xyz/call/`), a casa dos **Jogos** (`brittico.xyz/jogos/`),
o **LivePixel** (`brittico.xyz/livepixel/`), o **perfil** (`brittico.xyz/u/`, do jeito que o dono vê, com as cinco abas
de configuração) e as **Configurações do app**, com dados de mentira, nos temas claro e escuro e nas três paletas do
site. Nada fala com o servidor: o que no site leva a uma página que o app não tem (post, estatísticas, termos, os
outros jogos) abre no navegador.

## Rodar

```sh
cargo run --release
```

Abre no Início, como o site. Telas de início, para comparar com o site: `BRITTICO_TELA=inicio-teste` (o Início
com rascunho, votos e o "Responder" aberto), `BRITTICO_TELA=jogos` (a página dos jogos), `BRITTICO_TELA=perfil` e `perfil-baixo` (o perfil, em cima e rolado), `BRITTICO_TELA=geral` (o `#geral` da Call), `BRITTICO_TELA=sala`
(a porta do canal de voz "treino 1"), `BRITTICO_TELA=dentro` (dentro da call).

## Testar sem mexer no mouse

`BRITTICO_ROTEIRO` dá ao app eventos de mentira, um passo por quadro, separados por `|`:
`m x y` move · `c x y` clica · `r x y` clique direito · `p x y`/`u x y` aperta/solta · `k Tecla` (nome do egui::Key,
`shift+`/`alt+` na frente) · `t texto` digita · `f caminho` solta um arquivo · `s dy` rola · `w n` espera n quadros.
Junto com `BRITTICO_TELA` (as telas de início estão no `main.rs`, incluindo `livepixel-<janela>`, `perfil-aba1..5` e
`config0..4`) e `BRITTICO_TEMA=claro|escuro [agressivo|arredondado]`, e com a janela numa tela virtual do Hyprland,
dá para testar cada clique e fotografar o resultado. Do lado do site: `tools/site-mock.cjs` (`CLICAR="seletor; @x,y;
@@x,y"`, `ESQUEMA=light`, `HOST=localhost` para o modo de teste da Call), `tools/site-estilos.cjs` (posição e estilo
calculado de cada peça) e `tools/site-cliques.cjs` (tudo o que dá para clicar numa página, para achar função esquecida).

## O que dá para fazer

No Início:

- escrever um post (título, texto, o botão `>` do texto verde, contador) e **Publicar**: ele entra no topo do feed
  e na lista "Seus posts";
- votar para cima e para baixo (votar de novo tira o voto), **Responder** um post, apagar ou denunciar pelo "…";
- passar o mouse na grade de atividade para ver o dia.

Na Call:

- trocar de canal, abrir um canal de voz e clicar **Entrar na call** (ou dois cliques no canal);
- dentro da call: microfone, fone, câmera, mostrar a tela e sair;
- escrever no `#geral` e apertar Enter;
- a trilha da esquerda tem o Início; os Jogos, o LivePixel e o LoL de proximidade (a voz por proximidade, "novidades
  em breve"; não é o hub de LoL do site); os grupos da Call (cada grupo é um ícone da trilha, igual
  aos outros, e abre a Call nele; a coluna de grupos de dentro da Call saiu) e o "+"; e as configurações do app no pé.
  Na conversa, os emotes viram imagem, os links abrem no navegador, o nome abre o cartão da pessoa; o seletor de emotes,
  o menu do grupo (convidar, entrar com convite), o da conta, as categorias que recolhem, o quadro ampliado, o modo
  palco, a tela cheia, as estatísticas (I), os atalhos M, D, V, S, Esc e a Call Presidencial aberta.

No perfil (abre pelo nome no topo, pelo "Meu perfil" e pelo "ver perfil" do Início): o cartão, os números, a grade
de atividade, as publicações e as mensagens do chat; e as abas Editar Conta (cor do banner e bio com prévia ao vivo),
Preferências (tema escuro/claro/sistema, paleta e cor de destaque, que valem para o app inteiro, como o botão de sol do
topo), Segurança, Privacidade e Conexões (tornar principal, desvincular).

No LivePixel: pintar com 6 pincéis (teclas 1 a 6), conta-gotas (Alt/Shift), apagador, as 16 cores e as recentes com
seletor, 24 pixels por minuto contados de verdade, arrastar e a roda para andar e aproximar, o cartão de quem pintou
(clique direito) com compartilhar e denunciar, placar de streamers e artistas (clicar voa até o território), chat
global e da guilda, grade e territórios, o menu do perfil (notificações, configurações do jogo, trocar streamer), a
roleta diária, a imagem de guia (soltar, escolher ou colar; pixelar, opacidade, mover, redimensionar, travar) e as
ferramentas de moderador (apagar de qualquer um, selecionar, mover, Delete).

Nas Configurações do app: voz e vídeo (aparelhos, medidor do microfone, ruído, eco, volume automático, apertar para
falar, sensibilidade), os atalhos, a aparência, o comportamento do app (as animações desligam a flutuação dos Jogos de
verdade) e o Sobre com a memória própria medida na hora.

Nos Jogos: passar o mouse numa capa põe o jogo na faixa de cima; o botão ou a capa do LivePixel levam ao atalho dele,
os outros jogos avisam que ainda não chegaram ao app.

O app é focado em Início, Call, Jogos e LivePixel. Loja, música, LoL, VODs, as ferramentas da moderação (painel,
overlays, editor de thumbnail) e as páginas do Kennzy (mídia kit, IRL) ficam só no site, por decisão do Britto
(2026-10-30).

## De onde vem cada coisa

- os dados do Início e dos Jogos: `assets/mock/home.json` e `jogos.json`. O site só tem modo de teste na Call, então
  `node tools/site-mock.cjs saida.png /` (ou `/jogos/`, `"/u/?id=1"`; `ROLAR=800` desce a página) abre a página do site de verdade (a pasta `public/` servida em
  localhost) com a API respondida por esses mesmos arquivos. É assim que as duas telas são comparadas; com `MEDIR="seletor, seletor"` ele
  imprime onde cada peça ficou;
- cores e medidas: o site no tema Preto escuro, medido no navegador numa janela de 1600×900 (`theme.rs` usa os nomes das variáveis CSS);
- ícones: os mesmos SVG do site, copiados por `python3 tools/extrair-icones.py` (rodar de novo quando o site mudar um ícone);
- as capas dos jogos: `node tools/extrair-jogos.cjs` abre o /jogos/ do site e grava cada capa em `assets/jogos/`, em
  camadas (a parada e cada grupo que flutua), com as cores que o navegador calculou;
- fontes: Open Sans 600/700/800, Patua One e Avara, dentro do executável.

Medido em 2026-10-30, nos Jogos (as capas flutuam a 30 quadros por segundo): ~31 MB e ~6% de CPU. No Início parado: ~25 MB de memória própria e 0% de CPU. Em 2026-10-27, dentro da call com gente falando: ~40 MB de memória própria e ~1% de CPU; parado em outra seção, 0%.

## LoL de proximidade (o Olheiro)

O "L" da trilha abre uma **janela própria**, como a lista de amigos da Steam (`src/olheiro/`). Ela não tem ligação com a Call nem com a conta: quem trabalha é o `olheiro-motor`, que mora aqui em `crates/` (lê o LoL e o minimapa, entra sozinho na sala de voz da partida pelo porteiro, grava e toca a voz e ajusta o volume de cada um pela distância). Com ela aberta, o X da janela grande só esconde o Brittico (no Linux, minimiza); fechar a do Olheiro com a grande escondida fecha o app.

- O servidor de voz (porteiro) é o campo "Servidor de voz" dos Ajustes da janela, ou `OLHEIRO_PORTEIRO`. Vazio: o mapa funciona, ninguém se ouve.
- A janela: você no topo, a barra de voz, o time (clicar em alguém abre o volume e o mutar) e o minimapa embaixo. Abrir já liga. Para olhar sem o LoL: `BRITTICO_OLHEIRO=abrir | ajustes | simulada1..5` (e `BRITTICO_OLHEIRO_ABERTO=chave` já com o volume de alguém aberto). Duas cópias com `simulada1` e `simulada2`, `OLHEIRO_DIR` diferentes e o mesmo porteiro caem na mesma sala; `OLHEIRO_SOM=nao` ou `tom` não mexe no som do computador. `BRITTICO_OLHEIRO_ID` troca a classe da janela (para a regra do Hyprland).
- Windows: `./build-windows.sh` gera `dist/brittico.exe` (alvo MSVC pelo cargo-xwin; a libwebrtc pede `crt-static`, já no `.cargo/config.toml`).
