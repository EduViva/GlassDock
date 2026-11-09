# GlassDock

Dock de vidro (glassmorphism) com widgets que substitui a barra de tarefas do Windows 10 e 11.
É um app nativo (Rust + Win32 + Direct2D), sem navegador embutido nem runtime, pensado para gastar o mínimo possível.

## Instalar e usar

1. Copie `GlassDock.exe` para uma pasta sua, por exemplo `C:\Users\<você>\Apps\GlassDock\`.
2. Dê dois cliques. Como o executável não tem assinatura digital, o SmartScreen pode avisar: clique em **Mais informações** e depois em **Executar assim mesmo**.
3. A barra de tarefas some e a dock aparece. Para ter a barra de volta, basta sair do GlassDock (botão direito num espaço vazio da dock › **Sair do GlassDock**). Ela volta exatamente como estava, mesmo se o app fechar de repente.

Não precisa de administrador, de instalação nem de conta. Os ajustes ficam em `%APPDATA%\GlassDock\config.json`.

> Se algo der errado e a barra não voltar: rode `GlassDock.exe --restore-taskbar`.

## O que tem

**Dock**
- Vidro com desfoque nativo do Windows, borda clara, brilho e sombra
- Ampliação dos ícones estilo Mac (ou só escala, ou nada), nome do app ao passar o mouse
- Ícones não quadrados ganham uma placa arredondada (dá para desligar)
- Apps fixados, apps abertos (separados), indicador de aberto (ponto ou traço)
- Animação de abertura: pulo, pulso ou tremor
- Clique: abre o app; se já está na frente, minimiza; com várias janelas, alterna entre elas
- Clique do meio: nova janela
- Botão direito: lista de janelas, nova janela, fixar/remover, mover janelas para outro workspace, mostrar no Explorador, fechar, encerrar e forçar encerramento
- Prévias ao vivo das janelas ao parar o mouse num app (com botão de fechar; clique do meio também fecha)
- Arraste para reordenar ou para fora para remover, e arraste o separador para mudar o tamanho dos ícones
- Solte arquivos na dock: um `.exe`/atalho é fixado, uma pasta vira pilha, e soltar na Lixeira apaga
- Botão Iniciar (abre o Iniciar do Windows; o botão direito abre o menu Win+X)
- Pastas na dock, Downloads e Lixeira (com Esvaziar Lixeira)
- Espaçadores pequeno e grande
- Posição embaixo, em cima, à esquerda ou à direita
- Ocultar automaticamente, esconder em tela cheia (jogos, vídeos) e reservar o espaço para janelas maximizadas
- Vários monitores: a dock segue o ponteiro, ou uma dock por monitor
- Ctrl+Alt+D leva o foco para a dock (setas, Enter, Esc)

**Status e bandeja, em linha** (sem o quadrado grande)
- Rede, volume (a roda do mouse muda o volume) e bateria; um clique abre as Configurações rápidas
- Ícones da bandeja dos outros apps (Discord, OneDrive…): os fixados ficam em linha e o resto vai para a setinha. O clique e o botão direito vão para o próprio app.
- Relógio com data no formato regional; o clique abre o calendário

**Workspaces (ambientes)**
- Cada workspace tem os seus apps fixados, widgets e janelas
- Ao trocar, as janelas dos outros saem da frente e voltam quando você volta
- Troca com Ctrl+Alt+1…9, a roda do mouse sobre a dock, as bolinhas ou o menu
- Se uma janela de outro workspace ganha o foco (por exemplo com Alt+Tab), a dock vai para o workspace dela

**Widgets** (todos funcionais; clique abre um painel; botão direito › Editar widget…)
- Coleção de apps, Favoritos, Arquivos (Downloads ou outra pasta, com contagem de novos)
- Relógio, Analógico, Relógio mundial (14 cidades)
- Timer de foco, Pomodoro, Contagem regressiva, Progresso do tempo
- Calendário em 4 estilos. Os compromissos vêm do endereço ICS da sua agenda (Google, Outlook, iCloud), com eventos recorrentes.
- Lista de tarefas, Post-it
- Tocando agora: Spotify, navegador, app Música ou qualquer player, com capa e controles
- Bateria (3 estilos), Atividade de CPU e memória (3 estilos), Rede (2 estilos), Layout do teclado
- Conta-gotas (6 formatos de cor, atalho opcional Ctrl+Alt+P), Capturas de tela (arraste direto para outros apps), Beber água
- Mascote: gato, cachorro, passarinho, dragão, dino, guerreira, guerreiro, Papai Noel e fantasminha. Ele anda por cima da dock conforme a CPU e você pode colocar comida.
- Uso do Claude (limite de 5 h, semanal e tokens), Uso do Codex, Uso do OpenCode
- Clima (Open-Meteo, sem chave): agora, condições e previsão de 5 dias
- Pilhas: vários widgets no mesmo lugar, trocando com a roda do mouse

## Consumo de recursos

- Parado, não redesenha nada: tudo é disparado por eventos do Windows (janelas, volume, rede, bateria, pastas). O desfoque é feito pelo próprio Windows.
- Quando um widget muda (relógio, atividade), só o retângulo dele é redesenhado e enviado ao Windows.
- CPU, memória e rede só são lidas enquanto algum widget visível precisa delas.
- Música, clima, agenda e uso de IA rodam em threads de fundo que dormem até ter algo para fazer.
- Executável de ~2 MB.

## Diferenças em relação ao PrimoDock

- Sem temas sazonais, licença, loja, pacotes de ícones nem IconStudio (só o tema de vidro, escuro, claro ou seguindo o Windows)
- Bandeja em linha em vez do quadrado
- Calendário: o Windows não deixa apps comuns lerem a agenda do sistema, então os compromissos vêm de um endereço ICS
- Bateria: não mostra a bateria de fones e mouses Bluetooth
- Uso do Claude: o botão **Configurar** liga a status line do Claude Code ao GlassDock (a sua status line anterior continua aparecendo)

## Atalhos

| Atalho | O que faz |
|---|---|
| Ctrl+Alt+1 … 9 | Troca de workspace (desligável em Ajustes › Geral; em teclados ABNT2 ocupa o AltGr+número) |
| Ctrl+Alt+D | Foco na dock |
| Ctrl+Alt+P | Conta-gotas (se ligado no widget) |
| Roda do mouse na dock | Troca de workspace |
| Clique do meio num app | Nova janela |

## Compilar

Num Linux (ou WSL), com Rust e o mingw-w64:

```bash
rustup target add x86_64-pc-windows-gnu
```

```bash
cargo build --release --target x86_64-pc-windows-gnu
```

No Windows, com Rust instalado: `cargo build --release`.

O modo `GlassDock.exe --render-test <pasta>` salva em PNG a dock, os tiles e os painéis de todos os widgets (útil para conferir o visual).
