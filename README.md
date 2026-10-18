# suite-gpon

Suíte unificada de gestão de rede GPON/FTTH — substitui 4 clientes legados (A = Nokia/ALU 5520 AMS,
B = ZTE NetNumen U31, C = **Huawei U2000**, D = FiberHome UNM2000) por **um** app desktop.

- **Stack:** Rust (edition 2021+) + Tauri 2.x · UI em SvelteKit + Svelte 5 + **Bits UI** (headless)
- **Alvo:** Windows primário, Linux secundário · binário < 20 MB · RAM ociosa < 150 MB
- **Restrições de ouro:** nenhum runtime dos legados · nenhuma credencial em arquivo (cofre `keyring`)
  · UI **sem animação** (o build quebra se aparecer) · testes **100% mock** (Fases 0–1 sem hardware)
- **Spec:** documento interno do projeto, **não versionado aqui**. O teste `crates/domain/tests/spec_sync.rs`
  a lê pelo ambiente (`GPON_SPEC`) e, quando ela não existe, avisa e passa.

## Estrutura

```
crates/
  domain/              modelo canônico — Rust puro, sem Tauri, sem I/O, sem dependência
    src/entities.rs      18 entidades, com a evidência de disco de cada uma
    src/ops.rs           as 33 operações canônicas + nível de evidência por módulo (E/P/?/—)
    src/phase1.rs        escopo da Fase 1: 12 leituras + 4 escritas, módulo piloto C
    tests/spec_sync.rs   reabre a spec e compara tabela por tabela com o código
  transport/           contrato de execução — Rust puro, sem dependência
    src/lib.rs           trait Transport (async, futuro boxeado, std-only)
    src/cmd.rs           os 16 comandos canônicos da Fase 1, com payloads tipados
    src/result.rs        resultados tipados (Olt, Onu, PonPort, Alarm, WriteAck...)
    src/error.rs         TransportError: NaoSuportado/NaoEncontrado/FalhaDoNe/Rede/RespostaInvalida
    src/testing.rs       InMemoryTransport — o "NE" das Fases 0/1, com os dois perfis de fixture
    src/analise.rs       agrupar_massiva() + parser RFC3339 próprio (agrupa quedas por janela)
    tests/list_olts_in_memory.rs
    tests/busca_ordem_volume.rs   busca por SN/ALIAS, ordem por queda, lista vazia, 1024 ONUs
  mock-ne-huawei/      o "NE" das Fases 0/1 — fixtures por família (MA5800 e MA5600T)
    fixtures/ma5800/     dados.txt + catalogo_mml.txt (dicionário MML desta família)
    fixtures/ma5600t/    dados.txt + catalogo_mml.txt
    src/lib.rs           carrega a fixture no InMemoryTransport e valida campo a campo
    src/bin/             CLI: --model ma5800|ma5600t [--catalogo]
    tests/por_perfil.rs  os 16 comandos rodando DUAS vezes (um teste por op × perfil)
cofre/               contrato do cofre de credenciais — sem dependência (Segredo não vaza em log)
src-tauri/             casca desktop (comandos Tauri 2 + capabilities mínimas)
  src/ne.rs            ponte UI↔NE: comandos sobre CanonicalCmd + DTOs (lógica em funções *_de)
  src/guarda.rs        trava das escritas: prévia → token de uso único → execução + auditoria
  src/cofre_sistema.rs cofre real via `keyring` (Credential Manager / Secret Service / Keychain)
ui/                    SvelteKit 3 + Svelte 5 + Bits UI; build estático em ui/build
  src/routes/+page.svelte   estrutura U2000: árvore Fonte>OLT>PON>ONU, Find by, tela da PON (massiva)
  src/lib/demo.ts      modo demonstração (só fora do Tauri, anunciado na tela)
```

## Rodar

```bash
cargo test --workspace -j 4            # 120 testes: 19 domínio + 28 transporte + 41 mock + 7 cofre + 25 casca
cargo test -p gpon-domain --offline    # caminho rápido (sem compilar o Tauri)
bash scripts/verificar.sh              # gate completo: testes + lint de UI + build + régua da spec
```

Parede de recursos: numa máquina com pouca RAM/poucos núcleos, **use `-j 4`**. Um build paralelo
(`-j 12`) já derrubou uma máquina de desenvolvimento; o build do Tauri é a parte pesada.

### Abrir a janela (as duas formas certas)

Um `cargo build`/`cargo run` **puro** produz binário em **modo dev**: o Tauri então NÃO embute o
frontend e aponta a janela para `devUrl` (http://127.0.0.1:1420). Se o Vite não estiver rodando, a
janela abre com `Could not connect to 127.0.0.1: Connection refused` — não é bug do app, é o binário
de desenvolvimento sem o servidor de desenvolvimento.

Desenvolvimento (Vite + janela, com hot reload) — da RAIZ do repo:

```bash
node ui/node_modules/@tauri-apps/cli/tauri.js dev
```

Binário autônomo (frontend embutido, não precisa de servidor):

```bash
node ui/node_modules/@tauri-apps/cli/tauri.js build --debug   # mais rápido, binário grande
node ui/node_modules/@tauri-apps/cli/tauri.js build           # release (mede a meta de tamanho)
./target/release/suite-gpon
```

Os ícones (`src-tauri/icons/`) foram gerados por `tauri icon` a partir de `icons/icon.png`
(PNG flat, sem gradiente — coerente com a decisão de UI).

Para apontar a spec de outro lugar:

```bash
GPON_SPEC=/caminho/para/SPEC.md cargo test -p gpon-domain
```

## Estado (Fase 0, ordem da §7)

| # | Passo | Estado |
|---|---|---|
| 1 | Esqueleto Tauri 2.x + SvelteKit/Svelte 5 + Bits UI, tema e tokens | **pronto** — UI builda, tema claro/escuro persistido, casca Rust compila |
| 2 | Core de domínio + trait `Transport` + manifesto de capacidades | **domínio e `Transport` prontos e testados**; falta o `capabilities.toml` |
| 3 | `mock-ne-huawei` com os 2 perfis (`ma5800` / `ma5600t`) | **pronto** — fixtures por família, catálogo MML por perfil, 32 testes (16 operações × 2 perfis) + busca por critério, ordem por hora de queda, volume de 1024 ONUs e fixture inválida |
| 4 | As 12 leituras da §7.1 ligadas na UI | **parcial** — 6 ligadas com dado real (fontes, OLTs, PONs, ONUs, busca, alarmes) + a análise de massiva; faltam detalhe de ONU/OLT e export de inventário |
| 5 | Escritas seguras (reset, descrição) | **pronto no núcleo e na tela** — cofre real, prévia obrigatória, token de uso único (120 s), auditoria JSONL |
| 6 | Badge "via {módulo}" (§3.4) + painel F1 (§5.1) | **parcial** — badge nas telas; F1 com o resumo do escopo |

### O mock, por linha de comando

```bash
cargo run -q -p mock-ne-huawei --                          # lista os perfis
cargo run -q -p mock-ne-huawei -- --model ma5800           # resumo + self-test do perfil
cargo run -q -p mock-ne-huawei -- --model ma5600t --catalogo   # + dicionário MML da família
cargo run -q -p mock-ne-huawei -- --model xyz              # erro de perfil desconhecido (exit 2)
```

As fixtures são texto simples (`tipo|campo=valor|...`, uma linha por registro). O que muda entre
famílias é o **dicionário MML** e os números do equipamento — o transporte e o contrato são os mesmos.
Os nomes MML são **placeholder de fixture**; os reais entram com a coleta de campo (Fase 2).

## CI e qualidade

`.github/workflows/ci.yml` roda dois jobs: `cargo test --workspace -j 4` e o build da UI — que roda
antes o lint anti-animação (`ui/scripts/no-animation.mjs`): se alguém animar a interface, **o build
falha** e nada é empacotado. O mesmo gate está em `scripts/verificar.sh` para rodar localmente.

## Por que os crates de domínio e transporte não têm dependências

É proposital: `cargo test` roda **offline** e sem hardware, e o núcleo não conhece transporte, banco
nem Tauri. As dependências de rede (tokio, russh, snmp) entram atrás do trait `Transport` na Fase 2,
quando existir OLT de laboratório para exercitar — o contrato e os testes não mudam.

## A tela que resolve o trabalho chato

Selecionar a ONU do cliente → a PON dela → a suíte lista **todas as ONUs da PON ordenadas pela hora da
queda**, agrupa por janela (5 min por padrão, ajustável) e destaca o maior grupo como **possível
massiva**, com export CSV para abrir chamado. É a operação que hoje custa clicar em cada cliente para
comparar horários.

Fora do app (navegador), a UI entra em **modo demonstração** e avisa na tela; dentro do app, todo dado vem
do Rust por `CanonicalCmd`.

## Escrita: por que ela é chata de errar (e como a suíte trava)

Nenhuma operação que muda equipamento sai de um clique só. O fluxo é sempre:

1. **prévia** — a suíte mostra exatamente o que vai acontecer ("reiniciar a ONU SN — cliente (id) na
   PON 0/1") e devolve um token de uso único, válido por 120 s, que carrega *o pedido*;
2. **confirmação** — executa o que foi mostrado. Se a tela mudar depois da prévia, não muda o alvo;
3. **auditoria** — sucesso, falha e tentativa com token inválido viram linha em
   `auditoria.jsonl`, no diretório de dados do app.

A credencial nunca fica em arquivo: o que o app guarda é uma referência (`cofre://lab-1`) e o segredo
mora no cofre do sistema (Windows Credential Manager, Secret Service, Keychain). Se o cofre não estiver
disponível, a suíte **recusa** em vez de cair para texto — cair para texto é o que os legados faziam.
