//! Modelo de domínio canônico da suíte GPON/FTTH — transcrição de §1 da SPEC.
//!
//! Regras desta crate (SPEC §6 e §9):
//! - **Rust puro**: sem Tauri, sem I/O, sem socket → testável sem hardware e sem mock.
//! - **Zero dependência externa** nesta fatia: `cargo test` roda offline.
//! - **Nada aqui é invenção.** Cada linha é transcrição da SPEC, e `tests/spec_sync.rs`
//!   reabre o documento, lê a §1.3/§7.2 e falha se o código divergir dele.
//!
//! A regra de vocabulário da SPEC §1.2 vale no código: a UI e o domínio falam **só** o nome
//! canônico; os sinônimos dos vendors (autofind, Activate, REBOOT_ONT, backrest…) vivem no
//! adaptador e no tooltip, nunca no tipo compartilhado.

pub mod entities;
pub mod ops;
pub mod phase1;

pub use entities::{Entity, ENTITIES};
pub use ops::{CanonicalOp, Evidence, Module, ModuleEvidence, OPS};
pub use phase1::{phase1_ops, PHASE1_MODULE, PHASE1_READS, PHASE1_WRITES};
