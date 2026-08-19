# PoolVasta

Assistente pessoal de champion select para **League of Legends**.

A ideia: centralizar em um app desktop o que hoje eu fico caçando manualmente na internet
antes de cada partida — contra-picks, build atualizada do meta e uma linha de jogo pro
campeão que eu escolher, com base no pool de campeões que eu mais jogo.

> Projeto pessoal / de estudo. Não afiliado à Riot Games.

## O que ele faz (planejado)

- [x] Detecta o cliente do League rodando e conecta via **LCU API** (API local do próprio
      cliente — o mesmo mecanismo usado por Porofessor, Blitz, U.GG, OP.GG etc.)
- [x] Lê a sessão de champion select em tempo real (picks aliados e inimigos)
- [x] Monta o pool de campeões automaticamente a partir de maestria + últimas 20 partidas
      (nível/pontos de maestria, nota da temporada, win rate recente, flag de "enferrujado"
      quando maestria é alta mas faz tempo que não joga o campeão)
- [ ] Sugere campeão do meu pool com base na composição aliada x inimiga
- [ ] Mostra a build mais atualizada (itens/runas) do campeão selecionado
- [ ] Mostra uma sugestão de tática/plano de jogo pra aquela partida específica

### Sobre o pool ponderado

A LCU API não expõe a nota (S/A/B/C/D) de partidas passadas nem uma "média" real — isso só
existe na tela de fim de jogo e não fica salvo em lugar nenhum acessível. O que dá pra
consultar depois é `highestGrade` (melhor nota da temporada com aquele campeão) e o
histórico das últimas 20 partidas gerais (não só daquele campeão) com vitória/derrota. O
score do pool combina win rate recente (40%), maestria normalizada por pontos (35%) e a
melhor nota da temporada (25%); champions com mastery alta mas sem jogo há 45+ dias entram
como "pode estar enferrujado" em vez de sumirem da lista.

## Por que isso não mexe com o Vanguard

O app conversa **apenas** com a LCU API (`127.0.0.1`, porta e token lidos do lockfile do
cliente) — não injeta código, não lê/escreve memória do processo do jogo e não automatiza
nenhuma ação. É a mesma abordagem usada por todo companion app estabelecido do ecossistema
de LoL. A janela do PoolVasta roda separada, fora do processo do jogo.

## Stack

- [Tauri 2](https://tauri.app/) (Rust) — shell do app desktop
- React + TypeScript + Vite — frontend
- Fontes de dados estáticas: [Data Dragon](https://developer.riotgames.com/docs/lol) / Community Dragon

## Rodando localmente

Pré-requisitos: [Rust](https://www.rust-lang.org/tools/install), Node.js 18+, e as
[dependências do Tauri](https://tauri.app/start/prerequisites/) pro seu SO.

```bash
npm install
npm run tauri dev
```

## Roadmap

1. ~~Conexão com a LCU (status do cliente, dados do summoner)~~
2. ~~Leitura da sessão de champion select em tempo real~~
3. ~~Pool de campeões calculado a partir de maestria + histórico de partidas~~
4. Motor de sugestão por composição (regras simples: contra-pick, balanço de dano, engage/peel)
5. Exibição de build/runas atualizadas do campeão selecionado
6. Sugestão de tática por partida
7. Polimento de UI e empacotamento
