//! Contrato entre o núcleo e **qualquer** jeito de falar com o NE.
//!
//! A regra que faz este crate existir (SPEC §6): quem chama uma operação canônica não sabe — nem
//! pode saber — se a resposta veio de SSH, SNMP, FTP, de um mock em memória ou de um replay de
//! tráfego real. Trocar isso é trocar a implementação de `Transport`, não o chamador.
//!
//! Consequência prática nas Fases 0/1: o "NE" é o [`testing::InMemoryTransport`], sem porta, sem
//! rede, 100% determinístico. Na Fase 2 o mesmo `CanonicalCmd` passa a ser servido por um adaptador
//! de rede de verdade, e nenhum teste de domínio muda.

pub mod cmd;
pub mod error;
pub mod result;
pub mod testing;

pub use cmd::{
    AuthMode, CanonicalCmd, CriterioBusca, OltId, OnuId, OrdemOnu, Page, PonRef, ProfileKind,
    SegmentoOnu, SourceId,
};
pub use error::TransportError;
pub use result::{
    Alarm, CanonicalResult, ConnectivityReport, InventoryExport, Olt, Onu, OnuEstado, PonPort,
    Profile, Severidade, Source, Tecnologia, WriteAck,
};

use std::future::Future;
use std::pin::Pin;

/// O módulo que atende este transporte (A/B/C/D). É o que vira a badge "via {módulo}" (§3.4).
pub use gpon_domain::ops::Module;

/// Futuro boxeado — o contrato é async, mas o crate segue std-only (sem tokio/async-trait).
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// O contrato único de execução.
///
/// `&self` (não `&mut self`) de propósito: a UI pode ter várias telas lendo o mesmo transporte ao
/// mesmo tempo, e um adaptador de rede guarda pool/conexão internamente, não na chamada.
pub trait Transport: Send + Sync {
    /// Módulo que serve este transporte.
    fn module(&self) -> Module;

    /// Executa um comando canônico. **Não existe** segunda forma de executar a mesma operação:
    /// um caminho canônico por operação é decisão de produto (SPEC §9), e o teste
    /// `o_transporte_registra_o_caminho_canonico` guarda isso.
    fn exec(&self, cmd: CanonicalCmd) -> BoxFuture<'_, Result<CanonicalResult, TransportError>>;
}

/// Executa e devolve o resultado, com o erro já convertido em texto — atalho para a casca Tauri.
pub async fn exec_para_resultado(
    t: &dyn Transport,
    cmd: CanonicalCmd,
) -> Result<CanonicalResult, String> {
    t.exec(cmd).await.map_err(|e| e.to_string())
}
