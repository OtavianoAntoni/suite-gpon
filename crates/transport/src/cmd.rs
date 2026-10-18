//! Comandos canônicos: o vocabulário ÚNICO de execução.
//!
//! Os nomes aqui são os da §1.3 da SPEC (a UI e o domínio falam só isto). O que muda entre módulos
//! é a *tradução* (`list_olts` em C é `display ont...`, em B é SNMP): isso vive no adaptador, nunca
//! neste enum.

use std::fmt;

macro_rules! id_newtype {
    ($nome:ident, $rotulo:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $nome(pub String);

        impl $nome {
            pub fn novo(v: impl Into<String>) -> Self {
                Self(v.into())
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $nome {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<&str> for $nome {
            fn from(v: &str) -> Self {
                Self(v.to_string())
            }
        }

        impl AsRef<str> for $nome {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

id_newtype!(SourceId, "fonte");
id_newtype!(OltId, "olt");
id_newtype!(OnuId, "onu");

/// Janela de paginação. Toda listagem pagina: uma OLT de campo tem milhares de ONUs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    pub offset: usize,
    pub limit: usize,
}

impl Page {
    pub const fn primeira(limit: usize) -> Self {
        Self { offset: 0, limit }
    }
}

/// Referência a uma porta PON dentro da OLT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PonRef {
    pub slot: u8,
    pub porta: u8,
}

impl PonRef {
    pub const fn new(slot: u8, porta: u8) -> Self {
        Self { slot, porta }
    }
}

impl fmt::Display for PonRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.slot, self.porta)
    }
}

/// Tipo de perfil/template. Os nomes dos fornecedores variam (§1.2: "templet"), o tipo não.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProfileKind {
    Line,
    Dba,
    Bandwidth,
    QinQ,
    Vlan,
    Voip,
    OnuType,
    CapabilitySet,
}

impl ProfileKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Dba => "dba",
            Self::Bandwidth => "bandwidth",
            Self::QinQ => "qinq",
            Self::Vlan => "vlan",
            Self::Voip => "voip",
            Self::OnuType => "onu_type",
            Self::CapabilitySet => "capability_set",
        }
    }

    pub const TODOS: [ProfileKind; 8] = [
        Self::Line,
        Self::Dba,
        Self::Bandwidth,
        Self::QinQ,
        Self::Vlan,
        Self::Voip,
        Self::OnuType,
        Self::CapabilitySet,
    ];
}

/// Modo de autorização da ONU. Em D existe ainda "authorization number", que NÃO é SN (§1.1) —
/// por isso o tipo é enum e não string: a UI não pode confundir os dois.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    Sn,
    Loid,
    Password,
    None,
}

impl AuthMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sn => "sn",
            Self::Loid => "loid",
            Self::Password => "password",
            Self::None => "none",
        }
    }
}

/// Segmento de estado da ONU para filtrar listagem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentoOnu {
    Todas,
    Online,
    Offline,
    Los,
}

/// **Por que** o operador está procurando a ONU. SN e ALIAS vêm do trabalho de campo (§7.5);
/// VLAN depende do serviço da ONU e só existe a partir da Fase 2 (`list_services`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriterioBusca {
    Sn,
    Alias,
    Vlan,
}

impl CriterioBusca {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sn => "sn",
            Self::Alias => "alias",
            Self::Vlan => "vlan",
        }
    }
}

/// Ordem da listagem de ONUs.
///
/// `UltimoRegistro` existe por causa do caso de uso âncora (§7.4): identificar possível massiva
/// é olhar a PON **ordenada pela hora da queda**, não pelo SN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrdemOnu {
    #[default]
    Sn,
    /// Mais recente primeiro; ONU sem registro vai para o fim (ver `testing.rs`).
    UltimoRegistro,
}

impl OrdemOnu {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sn => "sn",
            Self::UltimoRegistro => "ultimo_registro",
        }
    }
}

/// Um comando canônico. Cobre o escopo da **Fase 1** (16 operações, §7.1); as outras 17 entram
/// quando a fase delas chegar — e o teste `toda_operacao_do_escopo_da_fase_1_tem_comando_canonico`
/// impede que este enum e a SPEC divergirem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalCmd {
    ListSources,
    TestConnectivity {
        source: SourceId,
    },
    ListOlts {
        source: SourceId,
    },
    GetOltInfo {
        olt: OltId,
    },
    ListPonPorts {
        olt: OltId,
    },
    ListOnus {
        olt: OltId,
        pon: Option<PonRef>,
        filtro: Option<String>,
        segmento: SegmentoOnu,
        page: Page,
        ordenar_por: OrdemOnu,
    },
    /// Mesma operação canônica da §1.3 (`find_onu_by_sn`), com **critério explícito**: o operador
    /// procura por SN, por ALIAS (nome/descrição) ou, na Fase 2, por VLAN do cliente.
    BuscarOnu {
        source: SourceId,
        criterio: CriterioBusca,
        valor: String,
    },
    GetOnu {
        onu: OnuId,
    },
    GetOnuAuthMode {
        onu: OnuId,
    },
    ListProfiles {
        source: SourceId,
        kind: ProfileKind,
    },
    ListAlarms {
        source: SourceId,
        filtro: Option<String>,
    },
    ExportInventory {
        filtro: Option<String>,
    },
    AuthorizeOnu {
        olt: OltId,
        pon: PonRef,
        sn: String,
        modo: AuthMode,
        perfil: Option<String>,
    },
    DeauthorizeOnu {
        onu: OnuId,
    },
    ResetOnu {
        onu: OnuId,
    },
    SetOnuDescription {
        onu: OnuId,
        texto: String,
    },
}

impl CanonicalCmd {
    /// Nome canônico — a chave que liga o comando à §1.3 e à auditoria.
    pub fn id(&self) -> &'static str {
        match self {
            Self::ListSources => "list_sources",
            Self::TestConnectivity { .. } => "test_connectivity",
            Self::ListOlts { .. } => "list_olts",
            Self::GetOltInfo { .. } => "get_olt_info",
            Self::ListPonPorts { .. } => "list_pon_ports",
            Self::ListOnus { .. } => "list_onus",
            // A operação canônica continua sendo a §1.3; o critério é parâmetro (§7.4/§7.5).
            Self::BuscarOnu { .. } => "find_onu_by_sn",
            Self::GetOnu { .. } => "get_onu",
            Self::GetOnuAuthMode { .. } => "get_onu_auth_mode",
            Self::ListProfiles { .. } => "list_profiles",
            Self::ListAlarms { .. } => "list_alarms",
            Self::ExportInventory { .. } => "export_inventory",
            Self::AuthorizeOnu { .. } => "authorize_onu",
            Self::DeauthorizeOnu { .. } => "deauthorize_onu",
            Self::ResetOnu { .. } => "reset_onu",
            Self::SetOnuDescription { .. } => "set_onu_description",
        }
    }

    /// Espelha a marca ⚠ da §1.3 — **sem duplicar a decisão**: quem manda é o domínio.
    pub fn destrutiva(&self) -> bool {
        gpon_domain::ops::by_id(self.id()).map(|o| o.destructive).unwrap_or(false)
    }

    /// As 16 operações da Fase 1, com payloads vazios — usado para conferir cobertura contra a SPEC.
    pub fn todos_do_escopo_fase1() -> Vec<CanonicalCmd> {
        let s = SourceId::novo("fonte-1");
        let o = OltId::novo("olt-1");
        let u = OnuId::novo("onu-1");
        vec![
            Self::ListSources,
            Self::TestConnectivity { source: s.clone() },
            Self::ListOlts { source: s.clone() },
            Self::GetOltInfo { olt: o.clone() },
            Self::ListPonPorts { olt: o.clone() },
            Self::ListOnus {
                olt: o.clone(),
                pon: None,
                filtro: None,
                segmento: SegmentoOnu::Todas,
                page: Page::primeira(100),
                ordenar_por: OrdemOnu::Sn,
            },
            Self::BuscarOnu {
                source: s.clone(),
                criterio: CriterioBusca::Sn,
                valor: String::new(),
            },
            Self::GetOnu { onu: u.clone() },
            Self::GetOnuAuthMode { onu: u.clone() },
            Self::ListProfiles { source: s.clone(), kind: ProfileKind::Line },
            Self::ListAlarms { source: s.clone(), filtro: None },
            Self::ExportInventory { filtro: None },
            Self::AuthorizeOnu {
                olt: o.clone(),
                pon: PonRef::new(1, 1),
                sn: String::new(),
                modo: AuthMode::Sn,
                perfil: None,
            },
            Self::DeauthorizeOnu { onu: u.clone() },
            Self::ResetOnu { onu: u.clone() },
            Self::SetOnuDescription { onu: u, texto: String::new() },
        ]
    }
}
