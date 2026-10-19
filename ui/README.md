# ui — a interface da suíte GPON

SvelteKit 3 + Svelte 5 (runes) + **Bits UI 2** (headless) + Vite 8 + TypeScript 6.
O build é **estático** e vai embutido no binário do Tauri: em produção **não existe
servidor Node** — o WebView carrega arquivos do disco.

## Por que a configuração parece estranha

- **Não existe `svelte.config.js`.** No SvelteKit 3 a configuração do kit mora dentro do
  plugin, em `vite.config.ts`.
- `adapter-static` com `fallback: 'index.html'`: é um SPA servido de arquivo.
- Porta **1420 fixa** (`strictPort: true`): é a `devUrl` que o `tauri.conf.json` aponta.
- `ssr = false` e `prerender = false` em `src/routes/+layout.ts`: desliga renderização no
  servidor, que aqui não existe.

## Mapa dos arquivos

- `src/routes/+page.svelte` — a tela única: árvore `Fonte > OLT > PON > ONU`, o
  *Find by* (SN / ALIAS / cliente) e a tela da PON com a análise de possível massiva.
- `src/routes/+layout.svelte`, `+layout.ts` — casca, tema e a guarda de SPA.
- `src/lib/ipc.ts` — **a única porta para o Rust**: os `invoke` dos comandos Tauri.
- `src/lib/demo.ts` — modo demonstração com dados falsos. Só entra **fora** do Tauri e
  avisa na tela; dentro do app todo dado vem do Rust por `CanonicalCmd`.
- `src/lib/theme.svelte.ts`, `tokens.css` — tema claro/escuro persistido e os tokens.
- `src/lib/index.ts`, `src/lib/assets/` — reexportações e ativos.
- `scripts/no-animation.mjs` — o lint anti-animação (abaixo).

## A regra que derruba o build: nada de animação

`npm run build` roda `npm run lint:no-anim` **antes** do `vite build`. Se aparecer
`transition`, `animation`, `@keyframes`, `animate-*` ou equivalente numa classe/estilo, o
build **falha** e nada é empacotado. Isso é decisão de produto, não preferência de estilo.

`build:raw` existe para depurar a build **sem** o lint — nunca use para entregar.

## Rodar

```sh
npm install          # a partir de ui/
npm run dev          # só a UI no navegador, http://127.0.0.1:1420 (modo demonstração)
npm run check        # svelte-check: tipos e avisos
npm run build        # lint anti-animação + build estático em ui/build
npm run preview      # serve a build de produção
```

A janela real do app (com o Rust) abre-se **da raiz do repo**, não daqui:

```sh
node ui/node_modules/@tauri-apps/cli/tauri.js dev     # Vite + janela, hot reload
node ui/node_modules/@tauri-apps/cli/tauri.js build   # release, frontend embutido
```

Um `cargo run` **puro** abre a janela em modo dev apontando para a porta 1420: sem o Vite
rodando a janela mostra `Could not connect to 127.0.0.1: Connection refused`. Não é bug do
app — é o binário de desenvolvimento sem o servidor de desenvolvimento.

## Recriar ou estender o projeto (`sv`)

Este projeto nasceu do template mínimo do `sv`:

```sh
npx sv@1.1.1 create --template minimal --types ts --install npm ui
```

Para adicionar recurso, `npx sv add` (ex.: `npx sv add tailwindcss`). O que entrar novo
precisa continuar passando pelo lint anti-animação e **não pode** depender de servidor
Node no runtime.

## Idioma

Comentários, nomes de tela, textos de interface e documentação são em **português do
Brasil**. Este README era o boilerplate em inglês gerado pelo `sv create` e foi traduzido
ao entrar no projeto — ver a seção "Idioma da documentação" no README da raiz.
