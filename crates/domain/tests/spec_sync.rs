//! **Teste de sincronia código ↔ SPEC.** Este é o teste que impede a divergência mais cara:
//! o código "melhorar" a evidência de uma operação (ou alguém trocar um número) e a SPEC —
//! que é o documento contratual — ficar dizendo outra coisa.
//!
//! Ele **reabre a SPEC em Markdown, parseia as tabelas §1.3 e §7.2 e compara** linha a linha
//! com `gpon_domain::ops::OPS` e `gpon_domain::phase1`.
//!
//! Caminho da SPEC: variável `GPON_SPEC`; default = artifact do profile netdev. Se o arquivo
//! não existir (máquina de CI, Windows sem o artifact), o teste **avisa e passa** — ele é uma
//! trava de auditoria local, não um requisito de build.

use gpon_domain::ops::{Evidence, Module, OPS};
use gpon_domain::phase1::{phase1_ops, PHASE1_MODULE};
use std::path::PathBuf;

/// Caminho padrão da SPEC neste ambiente (derivado de $HOME, para o repo não carregar
/// caminho absoluto de máquina nenhuma). Em CI ou fora daqui, use `GPON_SPEC`.
fn spec_padrao() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home)
        .join(".hermes/profiles/netdev/artifacts/SPEC-suite-gpon-rust-tauri.md")
}

fn spec_path() -> PathBuf {
    std::env::var("GPON_SPEC").map(PathBuf::from).unwrap_or_else(|_| spec_padrao())
}

fn load_spec() -> Option<String> {
    let p = spec_path();
    match std::fs::read_to_string(&p) {
        Ok(s) => {
            println!("SPEC lida: {} ({} bytes)", p.display(), s.len());
            Some(s)
        }
        Err(e) => {
            println!("AVISO: SPEC não encontrada em {} ({e}) — teste de sincronia PULADO", p.display());
            None
        }
    }
}

/// Recorta o corpo de uma seção: do header `ini` até o header `fim` (exclusivo).
fn section<'a>(spec: &'a str, ini: &str, fim: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut dentro = false;
    for line in spec.lines() {
        if line.starts_with(ini) {
            dentro = true;
            continue;
        }
        if dentro && line.starts_with(fim) {
            break;
        }
        if dentro {
            out.push(line);
        }
    }
    out
}

/// Glifo de evidência de uma célula da tabela: o **primeiro token** da célula depois de
/// remover formatação (`**`, setas). Células como `**P ↓** (só VAP/SPFE)` ou
/// `E (\`REBOOT_ONT\`)` são o motivo de ser o primeiro token e não um `contains`.
fn cell_evidence(cell: &str) -> Option<Evidence> {
    let limpa = cell.replace('*', "").replace('↓', "").replace('↑', "");
    let tok = limpa.split_whitespace().next()?;
    Evidence::from_glyph(tok)
}

/// Campos de uma linha de tabela markdown (`| a | b |` → ["a", "b"]).
fn cells(line: &str) -> Vec<&str> {
    line.trim().trim_start_matches('|').trim_end_matches('|').split('|').map(str::trim).collect()
}

#[test]
fn spec_1_3_bate_com_a_tabela_do_codigo() {
    let Some(spec) = load_spec() else { return };

    let corpo = section(&spec, "### 1.3", "### 1.4");
    assert!(!corpo.is_empty(), "§1.3 não encontrada na SPEC");

    let linhas: Vec<Vec<&str>> = corpo
        .iter()
        .filter(|l| l.starts_with("| "))
        .map(|l| cells(l))
        .filter(|c| c.first().is_some_and(|n| n.chars().all(|ch| ch.is_ascii_digit())))
        .collect();

    assert_eq!(linhas.len(), OPS.len(), "a §1.3 tem {} linhas numeradas; o código tem {}", linhas.len(), OPS.len());

    for c in linhas {
        // | # | Operação | Pré-condições | A | B | C | D |
        let n: u8 = c[0].parse().expect("número da linha");
        let assinatura = c[1].trim_matches('`');
        let op = OPS.iter().find(|o| o.n == n).unwrap_or_else(|| panic!("operação {n} existe na SPEC e não no código"));

        // A célula da §1.3 carrega PARÂMETROS e agrupa operações compostas com um só par de
        // crases (`list_tasks(source)` / `get_task(id)`) — comparar a string inteira com o id
        // acusa falso positivo (foi o que aconteceu na primeira execução). Compara-se o
        // PRIMEIRO identificador dos dois lados, que é o que identifica a operação.
        let cell_first = assinatura.split(['(', ' ', '/']).next().unwrap_or("").trim();
        let id_first = op.id.split('/').next().unwrap_or("").trim();
        assert_eq!(
            cell_first, id_first,
            "linha {n}: SPEC diz `{assinatura}` e o código diz `{}`",
            op.id
        );

        for (m, cell) in [(Module::A, c[3]), (Module::B, c[4]), (Module::C, c[5]), (Module::D, c[6])] {
            let esperado = cell_evidence(cell)
                .unwrap_or_else(|| panic!("linha {n} coluna {}: célula sem glifo E/P/?/—: {cell:?}", m.tag()));
            assert_eq!(
                op.evidence.of(m),
                esperado,
                "linha {n} (`{}`) coluna {}: SPEC diz {esperado:?} e o código diz {:?}",
                op.id,
                m.tag(),
                op.evidence.of(m)
            );
        }
    }
}

#[test]
fn spec_7_2_bate_com_a_fase_1_do_codigo() {
    let Some(spec) = load_spec() else { return };

    let corpo = section(&spec, "### 7.2", "### 7.3");
    let linhas: Vec<Vec<&str>> = corpo
        .iter()
        .filter(|l| l.starts_with("| "))
        .map(|l| cells(l))
        .filter(|c| c.first().is_some_and(|n| n.chars().all(|ch| ch.is_ascii_digit())))
        .collect();

    assert_eq!(linhas.len(), 16, "§7.2 lista as 16 operações da Fase 1");
    assert_eq!(linhas.len(), phase1_ops().len());

    for c in linhas {
        // | # | Operação da Fase 1 | Evidência real em C | Nível |
        let id = c[1].trim_matches('`');
        let nivel = cell_evidence(c[3]).unwrap_or_else(|| panic!("§7.2 linha de {id} sem nível"));
        let op = phase1_ops()
            .into_iter()
            .find(|o| o.id == id)
            .unwrap_or_else(|| panic!("§7.2 cita `{id}`, que não está no escopo da Fase 1 do código"));
        assert_eq!(
            op.evidence.of(PHASE1_MODULE),
            nivel,
            "`{id}`: §7.2 diz {nivel:?} em C e o código diz {:?}",
            op.evidence.of(PHASE1_MODULE)
        );
    }
}

#[test]
fn spec_ainda_declara_33_operacoes_e_modulo_piloto_c() {
    let Some(spec) = load_spec() else { return };

    // §1.3 título, §1.1 contagem de entidades e §0.2 (decisões congeladas).
    assert!(
        spec.contains("### 1.3 Operações atômicas canônicas (33)"),
        "a §1.3 deixou de declarar 33 operações — o código precisa acompanhar"
    );
    assert!(
        spec.contains("**C — Huawei U2000**"),
        "o módulo piloto da SPEC deixou de ser C — o código precisa acompanhar"
    );
    assert!(PHASE1_MODULE == Module::C);

    // §9: nenhuma credencial real, nenhum runtime legado.
    assert!(spec.contains("Sem runtime dos legados"), "§9 mudou de conteúdo");
}
