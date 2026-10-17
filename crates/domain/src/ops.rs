//! Operações atômicas canônicas (SPEC §1.3) e o nível de **evidência** por módulo.
//!
//! O nível mede **evidência**, não permissão de escrita (a SPEC corrige isso explicitamente):
//! a capacidade de escrita é declarada no manifesto de capacidades (§3.1) como
//! `read_only | read_write | experimental`. São dois eixos diferentes e a UI mostra os dois.

/// Módulo (aplicação legada) que pode servir a operação. SPEC §2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Module {
    /// A — Nokia/Alcatel-Lucent 5520 AMS (thin client; catálogo TL1 mora no servidor).
    A,
    /// B — ZTE NetNumen U31.
    B,
    /// C — Huawei U2000 / iMAP (módulo **piloto**).
    C,
    /// D — FiberHome UNM2000 (thin client ICE, **sem IDL** → leitura/experimental).
    D,
}

impl Module {
    pub const ALL: [Module; 4] = [Module::A, Module::B, Module::C, Module::D];

    pub const fn tag(self) -> &'static str {
        match self {
            Module::A => "A",
            Module::B => "B",
            Module::C => "C",
            Module::D => "D",
        }
    }

    /// Rótulo humano — é o que a badge "via {módulo}" (§3.4) escreve na tela.
    pub const fn label(self) -> &'static str {
        match self {
            Module::A => "AMS · Nokia 5520",
            Module::B => "ZTE U31",
            Module::C => "Huawei U2000",
            Module::D => "FiberHome UNM2000",
        }
    }
}

/// Nível de evidência medido em disco. O glifo é o que aparece na UI (§3.4) e na matriz (§3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    /// `E` — comando ou classe/artefato **nomeado** em disco.
    Direct,
    /// `P` — caminho plausível, sem comando nomeado.
    Probable,
    /// `?` — indício insuficiente.
    Unknown,
    /// `—` — **nenhuma evidência no cliente** (o catálogo vive no servidor do vendor).
    None,
}

impl Evidence {
    pub const fn glyph(self) -> &'static str {
        match self {
            Evidence::Direct => "E",
            Evidence::Probable => "P",
            Evidence::Unknown => "?",
            Evidence::None => "—",
        }
    }

    pub fn from_glyph(g: &str) -> Option<Evidence> {
        match g {
            "E" => Some(Evidence::Direct),
            "P" => Some(Evidence::Probable),
            "?" => Some(Evidence::Unknown),
            "—" | "-" => Some(Evidence::None),
            _ => None,
        }
    }

    /// `true` quando a operação pode ser oferecida: só `—` a torna inexistente no módulo.
    pub const fn is_usable(self) -> bool {
        !matches!(self, Evidence::None)
    }
}

/// Evidência da mesma operação nos 4 módulos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleEvidence {
    pub a: Evidence,
    pub b: Evidence,
    pub c: Evidence,
    pub d: Evidence,
}

impl ModuleEvidence {
    pub const fn new(a: Evidence, b: Evidence, c: Evidence, d: Evidence) -> Self {
        Self { a, b, c, d }
    }

    pub const fn of(&self, m: Module) -> Evidence {
        match m {
            Module::A => self.a,
            Module::B => self.b,
            Module::C => self.c,
            Module::D => self.d,
        }
    }

    pub fn count(&self, e: Evidence) -> usize {
        Module::ALL.iter().filter(|m| self.of(**m) == e).count()
    }
}

/// Uma operação atômica canônica. `id` é o **nome canônico** (vocabulário único da UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalOp {
    /// Posição na tabela §1.3 (1..=33) — mantida para o teste de sincronia com a SPEC.
    pub n: u8,
    /// Nome canônico. A UI fala só isto; os sinônimos ficam no adaptador (§1.2).
    pub id: &'static str,
    /// Assinatura como está na §1.3 (com os parâmetros).
    pub signature: &'static str,
    /// Pré-condição declarada na §1.3 (`—` = nenhuma).
    pub preconditions: &'static str,
    /// `true` se a SPEC marca ⚠ → a UI exige **confirmação dupla** antes de executar.
    pub destructive: bool,
    pub evidence: ModuleEvidence,
}

const E: Evidence = Evidence::Direct;
const P: Evidence = Evidence::Probable;
const Q: Evidence = Evidence::Unknown;
const X: Evidence = Evidence::None;

/// As **33** operações atômicas canônicas, na ordem da §1.3 da SPEC.
///
/// A contagem é invariante: ela é a base do critério de aceitação "100% das 33 (§1.3) nos
/// módulos elegíveis" (§7.3). Não acrescente operação aqui sem revisar a SPEC — o teste
/// `spec_sync` compara esta tabela linha a linha com o documento.
pub const OPS: [CanonicalOp; 33] = [
    CanonicalOp { n: 1, id: "list_sources", signature: "list_sources()", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(E, E, E, E) },
    CanonicalOp { n: 2, id: "test_connectivity", signature: "test_connectivity(source)", preconditions: "credencial no cofre", destructive: false,
        evidence: ModuleEvidence::new(E, E, E, P) },
    CanonicalOp { n: 3, id: "list_olts", signature: "list_olts(source)", preconditions: "source ok", destructive: false,
        evidence: ModuleEvidence::new(X, E, E, P) },
    CanonicalOp { n: 4, id: "get_olt_info", signature: "get_olt_info(olt)", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(X, E, E, P) },
    CanonicalOp { n: 5, id: "list_pon_ports", signature: "list_pon_ports(olt)", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(P, E, E, P) },
    CanonicalOp { n: 6, id: "list_onus", signature: "list_onus(olt, pon?, filtro, página)", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(X, E, E, P) },
    CanonicalOp { n: 7, id: "find_onu_by_sn", signature: "find_onu_by_sn(source, sn)", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(X, E, E, P) },
    CanonicalOp { n: 8, id: "get_onu", signature: "get_onu(onu)", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(X, E, E, P) },
    CanonicalOp { n: 9, id: "get_onu_optical", signature: "get_onu_optical(onu)", preconditions: "ONU online", destructive: false,
        evidence: ModuleEvidence::new(X, P, P, P) },
    CanonicalOp { n: 10, id: "get_onu_auth_mode", signature: "get_onu_auth_mode(onu)", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(Q, E, P, P) },
    CanonicalOp { n: 11, id: "list_onu_register_history", signature: "list_onu_register_history(olt, janela)", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(Q, E, P, Q) },
    CanonicalOp { n: 12, id: "list_profiles", signature: "list_profiles(source, kind)", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(P, E, E, P) },
    CanonicalOp { n: 13, id: "get_profile", signature: "get_profile(source, kind, nome)", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(P, E, E, P) },
    CanonicalOp { n: 14, id: "list_services", signature: "list_services(onu)", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(P, E, E, P) },
    CanonicalOp { n: 15, id: "list_alarms", signature: "list_alarms(source, filtro)", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(E, E, E, E) },
    // Em C, ack/clear é P: a família `alarm` do catálogo do U2000 só tem leitura (§7.2).
    CanonicalOp { n: 16, id: "ack_alarm/clear_alarm", signature: "ack_alarm/clear_alarm(alarme)", preconditions: "permissão", destructive: false,
        evidence: ModuleEvidence::new(P, E, P, P) },
    CanonicalOp { n: 17, id: "get_performance", signature: "get_performance(objeto, contador, janela)", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(P, E, E, P) },
    CanonicalOp { n: 18, id: "list_tasks/get_task", signature: "list_tasks(source) / get_task(id)", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(E, E, E, E) },
    CanonicalOp { n: 19, id: "export_inventory", signature: "export_inventory(filtro)", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(E, E, E, E) },
    CanonicalOp { n: 20, id: "download_backup", signature: "download_backup(ne, arquivo)", preconditions: "backup existente", destructive: false,
        evidence: ModuleEvidence::new(E, E, E, P) },
    CanonicalOp { n: 21, id: "authorize_onu", signature: "authorize_onu(olt, pon, sn, modo, perfil?)", preconditions: "perfil existe", destructive: false,
        evidence: ModuleEvidence::new(X, E, E, P) },
    CanonicalOp { n: 22, id: "deauthorize_onu", signature: "deauthorize_onu(onu)", preconditions: "confirmação dupla", destructive: true,
        evidence: ModuleEvidence::new(X, E, E, P) },
    // Evidência do ZTE em reset_onu é `REBOOT_ONT` — NÃO `reboot-onu`, que é só prosa (§11.2).
    CanonicalOp { n: 23, id: "reset_onu", signature: "reset_onu(onu)", preconditions: "ONU online", destructive: true,
        evidence: ModuleEvidence::new(X, E, E, P) },
    CanonicalOp { n: 24, id: "set_onu_description", signature: "set_onu_description(onu, texto)", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(P, E, E, P) },
    CanonicalOp { n: 25, id: "create_profile/delete_profile/apply_profile", signature: "create_profile/delete_profile/apply_profile", preconditions: "—", destructive: false,
        evidence: ModuleEvidence::new(X, E, E, P) },
    CanonicalOp { n: 26, id: "add_service", signature: "add_service(onu, uni, vlan, gem, tcont, qos)", preconditions: "ONU autorizada", destructive: false,
        evidence: ModuleEvidence::new(P, E, E, P) },
    CanonicalOp { n: 27, id: "remove_service", signature: "remove_service(serviço)", preconditions: "—", destructive: true,
        evidence: ModuleEvidence::new(X, E, E, X) },
    CanonicalOp { n: 28, id: "upgrade_ont_firmware", signature: "upgrade_ont_firmware(alvo, imagem)", preconditions: "imagem no storage", destructive: true,
        evidence: ModuleEvidence::new(E, E, E, P) },
    CanonicalOp { n: 29, id: "batch_upgrade", signature: "batch_upgrade(olt, imagem, filtro)", preconditions: "—", destructive: true,
        evidence: ModuleEvidence::new(P, E, E, P) },
    CanonicalOp { n: 30, id: "backup_ne/restore_ne", signature: "backup_ne(olt) / restore_ne(olt, arq)", preconditions: "—", destructive: true,
        evidence: ModuleEvidence::new(E, E, E, P) },
    CanonicalOp { n: 31, id: "discover_ne", signature: "discover_ne(source, faixa)", preconditions: "credencial", destructive: false,
        evidence: ModuleEvidence::new(E, E, E, P) },
    CanonicalOp { n: 32, id: "set_source_credentials", signature: "set_source_credentials(source, protocolo, usuário, ref)", preconditions: "autorização do usuário", destructive: false,
        evidence: ModuleEvidence::new(E, E, E, E) },
    CanonicalOp { n: 33, id: "cancel_task", signature: "cancel_task(id)", preconditions: "tarefa ativa", destructive: false,
        evidence: ModuleEvidence::new(X, E, P, P) },
];

/// Total de operações canônicas. Invariante da SPEC (§1.3).
pub const COUNT: usize = OPS.len();

/// Busca pelo nome canônico.
pub fn by_id(id: &str) -> Option<&'static CanonicalOp> {
    OPS.iter().find(|o| o.id == id)
}

/// Operações que o módulo consegue servir (evidência diferente de `—`).
pub fn serviceable_by(m: Module) -> impl Iterator<Item = &'static CanonicalOp> {
    OPS.iter().filter(move |o| o.evidence.of(m).is_usable())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sao_33_operacoes_canonicas() {
        assert_eq!(COUNT, 33, "a SPEC §1.3 fixa 33 operações canônicas");
    }

    #[test]
    fn numeracao_e_sequencial_e_sem_repeticao() {
        for (i, op) in OPS.iter().enumerate() {
            assert_eq!(op.n as usize, i + 1, "operação fora de ordem: {}", op.id);
        }
        for op in OPS.iter() {
            assert_eq!(
                OPS.iter().filter(|o| o.id == op.id).count(),
                1,
                "id canônico duplicado: {}",
                op.id
            );
        }
    }

    #[test]
    fn nenhum_modulo_serve_zero_operacoes() {
        // Garante que a coluna não foi transcrita toda como `—` por engano.
        for m in Module::ALL {
            assert!(
                serviceable_by(m).count() > 0,
                "módulo {} sem nenhuma operação utilizável — transcrição suspeita",
                m.tag()
            );
        }
    }

    #[test]
    fn apenas_operacoes_marcadas_sao_destrutivas() {
        assert_eq!(OPS.iter().filter(|o| o.destructive).count(), 6);
        let ids: Vec<_> = OPS.iter().filter(|o| o.destructive).map(|o| o.id).collect();
        assert_eq!(
            ids,
            vec![
                "deauthorize_onu",
                "reset_onu",
                "remove_service",
                "upgrade_ont_firmware",
                "batch_upgrade",
                "backup_ne/restore_ne"
            ]
        );
    }

    #[test]
    fn em_a_nenhuma_operacao_de_onu_olt_tem_evidencia_direta() {
        // Achado do REA, virado teste: em A o catálogo de ONU/OLT vive no servidor.
        // Se alguém "melhorar" a coluna A sem medição, isto falha.
        for id in [
            "list_olts",
            "get_olt_info",
            "list_onus",
            "find_onu_by_sn",
            "get_onu",
            "get_onu_optical",
            "authorize_onu",
            "deauthorize_onu",
            "reset_onu",
        ] {
            let op = by_id(id).expect("operação prevista na §1.3");
            assert_ne!(
                op.evidence.a,
                Evidence::Direct,
                "A não tem evidência direta para {id} — ver §1.3 e §11"
            );
        }
    }

    #[test]
    fn distribuicao_de_evidencia_bate_com_a_spec() {
        // Números MEDIDOS na §1.3 (linha "Distribuição de evidência medida") e recontados
        // campo a campo pela régua v052: A 10/9/2/12 · B 32/1 · C 28/5 · D 5/26/1/1.
        //
        // Armadilha registrada: a prosa da §1.3 dizia "9 operações sem evidência" em A e
        // "as três E de D" — as duas NÃO batiam com a própria tabela (12 e 5). A prosa foi
        // corrigida; este teste trava o número certo.
        let conta = |m: Module, e: Evidence| OPS.iter().filter(|o| o.evidence.of(m) == e).count();

        assert_eq!((conta(Module::A, Evidence::Direct), conta(Module::A, Evidence::Probable),
                    conta(Module::A, Evidence::Unknown), conta(Module::A, Evidence::None)),
                   (10, 9, 2, 12), "coluna A");
        assert_eq!((conta(Module::B, Evidence::Direct), conta(Module::B, Evidence::Probable)),
                   (32, 1), "coluna B");
        assert_eq!((conta(Module::C, Evidence::Direct), conta(Module::C, Evidence::Probable)),
                   (28, 5), "coluna C");
        assert_eq!((conta(Module::D, Evidence::Direct), conta(Module::D, Evidence::Probable),
                    conta(Module::D, Evidence::Unknown), conta(Module::D, Evidence::None)),
                   (5, 26, 1, 1), "coluna D");

        // As 5 operações com `E` em D, nomeadas (2 são gestão local de fonte, 3 são
        // "módulo nomeado existe"): §1.3, nota de honestidade.
        let ed: Vec<_> = OPS.iter().filter(|o| o.evidence.d == Evidence::Direct).map(|o| o.id).collect();
        assert_eq!(ed, vec!["list_sources", "list_alarms", "list_tasks/get_task",
                            "export_inventory", "set_source_credentials"]);
    }
}
