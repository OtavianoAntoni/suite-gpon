//! Escopo da **Fase 1** (módulo piloto **C — Huawei U2000**, ambiente 100% mock) — SPEC §7.1/§7.2.
//!
//! 12 leituras + 4 escritas = **16** operações distintas. (§7.1 registra que a v0.1 contava
//! `list_alarms` duas vezes: o total distinto real era 15, não 16 — daí o teste de unicidade.)

use crate::ops::{by_id, CanonicalOp, Evidence, Module};

/// Módulo piloto decidido pelo dono (§0.2). Fases 0–1 não usam hardware real.
pub const PHASE1_MODULE: Module = Module::C;

/// As 12 leituras da Fase 1 (§7.1).
pub const PHASE1_READS: [&str; 12] = [
    "list_sources",
    "test_connectivity",
    "list_olts",
    "get_olt_info",
    "list_pon_ports",
    "list_onus",
    "find_onu_by_sn",
    "get_onu",
    "list_profiles",
    "list_alarms",
    "export_inventory",
    "get_onu_auth_mode",
];

/// As 4 **escritas** da Fase 1 (§7.1). `ack_alarm/clear_alarm` saiu daqui: em C é `P` (§7.2).
pub const PHASE1_WRITES: [&str; 4] = [
    "authorize_onu",
    "deauthorize_onu",
    "reset_onu",
    "set_onu_description",
];

/// As 16 operações da Fase 1, resolvidas contra a tabela canônica.
pub fn phase1_ops() -> Vec<&'static CanonicalOp> {
    PHASE1_READS
        .iter()
        .chain(PHASE1_WRITES.iter())
        .map(|id| by_id(id).unwrap_or_else(|| panic!("§7.1 cita operação fora da §1.3: {id}")))
        .collect()
}

/// Nível de evidência **em C** esperado para cada operação da Fase 1 (§7.2, medido).
/// Devolve `(operation_id, evidence)`.
pub fn phase1_expected_evidence_in_c() -> Vec<(&'static str, Evidence)> {
    phase1_ops().iter().map(|o| (o.id, o.evidence.of(Module::C))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn fase_1_tem_16_operacoes_distintas() {
        let ops = phase1_ops();
        assert_eq!(ops.len(), 16, "12 leituras + 4 escritas");
        let unicos: HashSet<_> = ops.iter().map(|o| o.id).collect();
        assert_eq!(unicos.len(), 16, "a v0.1 contava list_alarms duas vezes (§7.1)");
    }

    #[test]
    fn leituras_e_escritas_nao_se_sobrepoem() {
        for r in PHASE1_READS {
            assert!(!PHASE1_WRITES.contains(&r), "{r} não pode ser leitura e escrita");
        }
    }

    #[test]
    fn modulo_piloto_e_c_huawei() {
        assert_eq!(PHASE1_MODULE, Module::C);
        assert_eq!(PHASE1_MODULE.label(), "Huawei U2000");
    }

    #[test]
    fn em_c_a_fase_1_tem_15_e_mais_1_p() {
        // Número publicado na §7.2: "15 de 16 com evidência E em C; 1 em P".
        let ev = phase1_expected_evidence_in_c();
        let diretas = ev.iter().filter(|(_, e)| *e == Evidence::Direct).count();
        let provaveis = ev.iter().filter(|(_, e)| *e == Evidence::Probable).count();
        assert_eq!(diretas, 15, "evidência E em C");
        assert_eq!(provaveis, 1, "a única P é get_onu_auth_mode");
        assert_eq!(
            ev.iter().find(|(_, e)| *e == Evidence::Probable).map(|(id, _)| *id),
            Some("get_onu_auth_mode")
        );
    }

    #[test]
    fn nenhuma_operacao_da_fase_1_e_sem_evidencia_em_c() {
        for op in phase1_ops() {
            let e = op.evidence.of(Module::C);
            assert_ne!(e, Evidence::None, "{} não tem evidência em C (§7.2)", op.id);
        }
    }

    #[test]
    fn get_onu_optical_fica_para_a_fase_2() {
        // §7.2: em C é `P` (os hits de "optical" são de porta/threshold de NNI, não de
        // potência da ONU) → é o primeiro item da Fase 2, não da Fase 1.
        assert!(!PHASE1_READS.contains(&"get_onu_optical"));
        assert!(!PHASE1_WRITES.contains(&"get_onu_optical"));
        assert_eq!(by_id("get_onu_optical").unwrap().evidence.of(Module::C), Evidence::Probable);
    }
}
