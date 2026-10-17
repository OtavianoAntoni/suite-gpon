//! Erro de transporte — tipado, porque a UI precisa reagir diferente a cada caso.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// A operação não existe neste módulo (na §1.3 é a célula `—`).
    NaoSuportado {
        modulo: String,
        operacao: &'static str,
    },
    /// O alvo citado não existe na fonte/NE (id errado, ONU removida, OLT fora da fonte).
    NaoEncontrado { o_que: String, chave: String },
    /// O NE respondeu, mas com erro de protocolo/CLI (ex.: comando negado por perfil de usuário).
    FalhaDoNe { mensagem: String },
    /// Falha de rede: conexão, timeout, TLS. É o que a Fase 2 vai produzir de verdade.
    Rede { mensagem: String },
    /// Resposta chegou e o parser não reconheceu o formato — o risco real de firmware diferente.
    RespostaInvalida {
        esperado: &'static str,
        recebido: String,
    },
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NaoSuportado { modulo, operacao } => {
                write!(f, "o módulo {modulo} não oferece `{operacao}`")
            }
            Self::NaoEncontrado { o_que, chave } => {
                write!(f, "{o_que} `{chave}` não encontrado na fonte")
            }
            Self::FalhaDoNe { mensagem } => write!(f, "o NE recusou/falhou: {mensagem}"),
            Self::Rede { mensagem } => write!(f, "falha de rede: {mensagem}"),
            Self::RespostaInvalida { esperado, recebido } => {
                write!(f, "resposta fora do formato esperado ({esperado}): {recebido}")
            }
        }
    }
}

impl std::error::Error for TransportError {}
