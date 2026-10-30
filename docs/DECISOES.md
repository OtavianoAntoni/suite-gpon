# Decisões do dono — errata viva da SPEC

Este arquivo existe porque a SPEC (`SPEC-suite-gpon-rust-tauri.md`, v0.6.2, fora do repo) diz em
três lugares que a decisão do módulo piloto e da navegação está **"congelada, não reabrir"** — e o
dono reabriu. Enquanto a SPEC não for regerada, **esta é a errata que vale**, e ela supersede os
pontos listados abaixo.

Regra de leitura: **o que está aqui ganha da SPEC; o que não está aqui continua valendo da SPEC.**
E o que é *medição* (nível de evidência) **não é decisão** — medição não muda por decisão.

---

## 2026-10-30 — piloto do adaptador passa a ser A (Nokia AMS)

**Decisão do dono:** o módulo piloto do adaptador é **A — Nokia 5520 AMS**, falando **TL1 via
TL1GW** direto no NE. Confirmado junto: a **UI espelha o AMS** (decisão de 2026-10-28, reafirmada).

### O que isto supersede na SPEC

| Ponto da SPEC | Dizia | Passa a valer |
|---|---|---|
| §0.2 `{{MODULO_PILOTO}}` | "C — Huawei U2000" | **A — Nokia 5520 AMS** |
| §7 (Fase 1) | "piloto **C (Huawei U2000)**, ambiente 100% mock" | piloto **A**, ambiente 100% mock |
| §7.2 | evidência remedida **no catálogo do U2000** (15 `E` + 1 `P`) | precisa ser **remedida em A** (ver tabela abaixo) |
| §8.1 | "estrutura de navegação **espelha a U2000**" | espelha o **AMS** (já era a decisão de 28/10) |
| §7 P4 | `{{MODELO_NE_HUAWEI}}` = MA5800 + MA5600T | deixa de ser o piloto; vira o **mock do 2º módulo** (C), se ele for usado |

### O que **não** muda

- **A matriz de evidência (§1.3)**: 33 operações × 4 módulos. Evidência é medição em disco, não
  preferência. A coluna **A continua como foi medida** — inclusive as células `—`.
- As **33** operações canônicas e as **16** da Fase 1 (12 leituras + 4 escritas).
- O trait `Transport`, os comandos, os resultados, a trava de escrita, o cofre, a auditoria.

### O número que a decisão expõe (medido no código, não estimado)

`crates/domain/src/phase1.rs` + `crates/domain/src/ops.rs` — as 16 da Fase 1 na coluna **A**:

| Nível em A | Qtd | Operações |
|---|---|---|
| `E` (evidência direta) | **4** | `list_sources`, `test_connectivity`, `list_alarms`, `export_inventory` |
| `P` (provável, sem comando nomeado) | **3** | `list_pon_ports`, `list_profiles`, `set_onu_description` |
| `?` (indício insuficiente) | **1** | `get_onu_auth_mode` |
| `—` (**nenhuma evidência no cliente**) | **8** | `list_olts`, `get_olt_info`, `list_onus`, `find_onu_by_sn`, `get_onu`, `authorize_onu`, `deauthorize_onu`, `reset_onu` |

**Consequência dura:** 8 das 16 — incluindo **as 3 escritas de ONU** — não têm evidência no cliente
do AMS. Não é falha da decisão: é o que o REA mediu. O catálogo TL1 do AMS **mora no servidor**, e o
cliente só expõe comandos de sessão/gateway (`ACT-USER`, `RTRV-GW-*`, `ED-GW-CONFIG`).

`test_connectivity` em A mede **a conexão com o servidor AMS**, não o alcance até a OLT (§2.1, P3).

### O que destrava (porta da Fase 2 de A)

Duas fontes de evidência, nesta ordem de preferência:

1. **Catálogo TL1 do servidor AMS** (arquivo, sem tocar em rede) → promove as 8 células `—` a
   `E-operador` com comando nomeado.
2. **Captura de uma sessão AMS ↔ OLT** (cut-through/TL1GW) com autorização explícita → mesma coisa,
   por engenharia reversa do tráfego, mais caro e mais frágil.

Sem (1) ou (2), o escopo real da Fase 1 em A é de **4 operações com evidência + 3 prováveis** — e
prometer 16 seria vender o que não foi medido.

### Ponto de código a virar (quando a evidência de A chegar)

Nada foi virado ainda, de propósito: mudar o rótulo sem a evidência faria a suíte **afirmar** A com
dado de mock Huawei — mentira na tela.

| Onde | Hoje | Vira |
|---|---|---|
| `crates/domain/src/phase1.rs` → `PHASE1_MODULE` | `Module::C` | `Module::A` |
| `crates/mock-ne-huawei/` | 2 perfis Huawei (MML) | fixture Nokia (TL1) — o mock deixa de se chamar "huawei" |
| `src-tauri/src/ne.rs` → `modulo: "C"` nas DTOs | `"C"` | `"A"` |
| `ui/src/routes/+page.svelte` → badge | `<span class="badge">via C · E</span>` (literal) | derivada do DTO (dívida registrada: hoje é hardcode) |
| `src-tauri/src/lib.rs` → testes de contrato | `escopo.modulo == "C"`, "15 `E` em C" | equivalentes em A |

### Guarda de integridade que **não** pode ser afrouxada

`crates/domain/src/ops.rs::em_a_nenhuma_operacao_de_onu_olt_tem_evidencia_direta` falha se alguém
promover a coluna A sem medição nova. Quando A vira piloto existe a tentação de "melhorar" a
evidência para o plano ficar bonito. **Esse teste é a trava** — se ele começar a atrapalhar, o
problema é a promoção sem medição, não o teste.

---

## 2026-10-28 — a navegação espelha o Nokia AMS

**Decisão do dono:** a árvore e o fluxo da suíte espelham o **AMS**, não o U2000 — porque o AMS está
vivo e observável nesta máquina, e a fraqueza de evidência estática de A se compensa por observação
direta.

Consequências já implementadas: árvore `Fonte > NE > Rack > Subrack > Slot > PON > ONU` com carga
sob demanda; `Slot 1` exibido como `LT1`; rack/subrack variáveis no caminho físico
(`PonRef { rack, subrack, slot, porta }`, exibido como `1/1/1/1`).

Supersede: SPEC §8.1 ("espelha a U2000") e o §4 (mapa de intenções), que ficam como histórico.
