//! Resultados canônicos — as formas que a UI consome.
//!
//! Cada struct é a projeção do que a §1.1 declara como entidade essencial. Campo que não tem
//! evidência de disco em nenhum módulo **não entra** aqui: é melhor a UI não mostrar do que mostrar
//! um campo inventado.

use crate::cmd::{AuthMode, OltId, OnuId, PonRef, ProfileKind, SourceId};
use std::fmt;

/// Fonte/alvo de gestão. `credencial_ref` é **referência ao cofre**, nunca o segredo (§9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub id: SourceId,
    pub rotulo: String,
    pub host: String,
    pub protocolo: String,
    pub credencial_ref: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tecnologia {
    Gpon,
    Epon,
    XgsPon,
    Desconhecida,
}

impl Tecnologia {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gpon => "GPON",
            Self::Epon => "EPON",
            Self::XgsPon => "XGS-PON",
            Self::Desconhecida => "?",
        }
    }
}

/// OLT / NE (SPEC §1.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Olt {
    pub id: OltId,
    pub fonte: SourceId,
    pub vendor: String,
    pub modelo: String,
    pub serial: String,
    pub ip: String,
    pub sw_version: String,
    pub uptime_s: u64,
    pub slots: u16,
    pub temperatura_c: Option<i16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PonPort {
    pub olt: OltId,
    pub pon: PonRef,
    pub tecnologia: Tecnologia,
    pub admin_up: bool,
    pub split_ratio: u8,
    pub onus: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnuEstado {
    Online,
    Offline,
    Los,
    Desconhecido,
}

impl OnuEstado {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Offline => "offline",
            Self::Los => "LOS",
            Self::Desconhecido => "?",
        }
    }
}

/// ONU / ONT (SPEC §1.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Onu {
    pub id: OnuId,
    pub olt: OltId,
    pub pon: PonRef,
    pub sn: String,
    pub mac: Option<String>,
    pub modelo_hw: String,
    pub modelo_sw: String,
    pub vendor: String,
    pub nome: String,
    pub descricao: String,
    pub estado: OnuEstado,
    pub distancia_m: Option<u32>,
    pub ultimo_registro: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub nome: String,
    pub kind: ProfileKind,
    pub versao: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severidade {
    Critica,
    Maior,
    Menor,
    Aviso,
    Info,
}

impl Severidade {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Critica => "critical",
            Self::Maior => "major",
            Self::Menor => "minor",
            Self::Aviso => "warning",
            Self::Info => "info",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alarm {
    pub id: String,
    pub severidade: Severidade,
    pub ne: OltId,
    pub objeto: String,
    pub tipo: String,
    pub primeira: String,
    pub ultima: String,
    pub ack: bool,
}

/// Resultado do teste de conexão.
///
/// Em A isto mede a **conexão com o servidor AMS**; o passo "servidor → NE" é opcional e pertence
/// à Fase 2 (§1.3, P3). O campo `alcancou_ne` existe para não deixar a diferença invisível.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectivityReport {
    pub alcancou_servidor: bool,
    pub alcancou_ne: Option<bool>,
    pub latencia_ms: Option<u64>,
    pub mensagem: String,
}

/// Confirmação de uma operação de escrita.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteAck {
    pub ok: bool,
    pub mensagem: String,
    /// Id do job/tarefa no NE, quando a operação é assíncrona (upgrade, batch).
    pub id_operacao: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryExport {
    pub caminho: String,
    pub linhas: usize,
    pub sha256: String,
}

/// Resultado de um `exec` — sempre tipado, nunca um blob de texto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalResult {
    Sources(Vec<Source>),
    Connectivity(ConnectivityReport),
    Olts(Vec<Olt>),
    Olt(Olt),
    PonPorts(Vec<PonPort>),
    Onus { itens: Vec<Onu>, total: usize },
    Onu(Onu),
    AuthMode(AuthMode),
    Profiles(Vec<Profile>),
    Alarms(Vec<Alarm>),
    Export(InventoryExport),
    Ack(WriteAck),
}

impl CanonicalResult {
    /// Rótulo curto para log/auditoria e para o teste de contrato.
    pub fn tipo(&self) -> &'static str {
        match self {
            Self::Sources(_) => "sources",
            Self::Connectivity(_) => "connectivity",
            Self::Olts(_) => "olts",
            Self::Olt(_) => "olt",
            Self::PonPorts(_) => "pon_ports",
            Self::Onus { .. } => "onus",
            Self::Onu(_) => "onu",
            Self::AuthMode(_) => "auth_mode",
            Self::Profiles(_) => "profiles",
            Self::Alarms(_) => "alarms",
            Self::Export(_) => "export",
            Self::Ack(_) => "ack",
        }
    }
}

impl fmt::Display for CanonicalResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}>", self.tipo())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onus_carrega_total_para_paginacao() {
        // A UI precisa saber que há mais páginas: `total` é o total do NE, `itens` é a janela.
        let r = CanonicalResult::Onus { itens: vec![], total: 512 };
        assert_eq!(r.tipo(), "onus");
        match r {
            CanonicalResult::Onus { itens, total } => {
                assert!(itens.is_empty());
                assert_eq!(total, 512);
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn rotulos_de_tecnologia_e_estado_sao_estaveis() {
        assert_eq!(Tecnologia::XgsPon.as_str(), "XGS-PON");
        assert_eq!(OnuEstado::Los.as_str(), "LOS");
        assert_eq!(Severidade::Critica.as_str(), "critical");
    }
}
