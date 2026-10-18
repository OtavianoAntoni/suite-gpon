//! Casca desktop (Tauri 2) — ponte entre o WebView e o núcleo de domínio.
//!
//! Duas regras que este arquivo respeita (SPEC §6/§9):
//! - **Nenhum dado canônico nasce aqui**: os comandos só *traduzem* o que `gpon-domain` já
//!   declara e testa. Se um número aparecer em dois lugares, ele vai divergir.
//! - **Nenhum runtime legado**: não há `Command::new`, nem ponte para os clientes antigos.
//!   A Fase 0/1 fala com o `mock-ne-huawei` (ainda por vir) através do trait `Transport`.

mod ne;

use gpon_domain::ops::Module;
use gpon_domain::phase1::{self, PHASE1_MODULE};
use gpon_domain::{entities, ops};
use serde::Serialize;
use std::collections::BTreeMap;

/// Um módulo A/B/C/D, como a barra superior da UI mostra.
#[derive(Serialize)]
pub struct ModuloInfo {
    tag: String,
    rotulo: String,
}

/// Uma operação canônica no formato que a UI consome (`ui/src/lib/ipc.ts`).
#[derive(Serialize)]
pub struct OperacaoDto {
    n: u8,
    id: String,
    assinatura: String,
    precondicoes: String,
    destrutiva: bool,
    /// Glifo de evidência por módulo: `{"A": "E", "B": "E", "C": "E", "D": "P"}`.
    evidencias: BTreeMap<String, String>,
}

/// Escopo da Fase 1 (SPEC §7.1/§7.2).
#[derive(Serialize)]
pub struct EscopoFase1Dto {
    modulo: String,
    leitura: Vec<String>,
    escrita: Vec<String>,
    /// (operação, nível esperado em C) — o que a §7.2 mediu.
    esperado_em_c: Vec<(String, String)>,
}

#[tauri::command]
fn modulos() -> Vec<ModuloInfo> {
    Module::ALL
        .iter()
        .map(|m| ModuloInfo { tag: m.tag().to_string(), rotulo: m.label().to_string() })
        .collect()
}

#[tauri::command]
fn operacoes_canonicas() -> Vec<OperacaoDto> {
    ops::OPS
        .iter()
        .map(|op| OperacaoDto {
            n: op.n,
            id: op.id.to_string(),
            assinatura: op.signature.to_string(),
            precondicoes: op.preconditions.to_string(),
            destrutiva: op.destructive,
            evidencias: Module::ALL
                .iter()
                .map(|m| (m.tag().to_string(), op.evidence.of(*m).glyph().to_string()))
                .collect(),
        })
        .collect()
}

#[tauri::command]
fn escopo_fase1() -> EscopoFase1Dto {
    EscopoFase1Dto {
        modulo: PHASE1_MODULE.tag().to_string(),
        leitura: phase1::PHASE1_READS.iter().map(|s| s.to_string()).collect(),
        escrita: phase1::PHASE1_WRITES.iter().map(|s| s.to_string()).collect(),
        esperado_em_c: phase1::phase1_expected_evidence_in_c()
            .iter()
            .map(|(id, e)| (id.to_string(), e.glyph().to_string()))
            .collect(),
    }
}

/// Entidades canônicas (§1.1) — usado pelo onboarding/painel F1.
#[tauri::command]
fn entidades_canonicas() -> Vec<(String, String)> {
    entities::ENTITIES.iter().map(|e| (e.name.to_string(), e.fields.to_string())).collect()
}

pub fn run() {
    // As fixtures dos dois perfis são carregadas aqui, uma vez: a UI nunca espera disco.
    let aplicacao = ne::Aplicacao::carregar();

    tauri::Builder::default()
        .manage(aplicacao)
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            modulos,
            operacoes_canonicas,
            escopo_fase1,
            entidades_canonicas,
            ne::fontes,
            ne::listar_olts,
            ne::listar_pons,
            ne::listar_onus,
            ne::buscar_onu,
            ne::massiva,
            ne::listar_alarmes
        ])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar a suíte");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O contrato com a UI é um dado, não uma promessa: se alguém mudar o núcleo e isso
    /// vazar para a tela, estes números quebram aqui primeiro.
    #[test]
    fn contrato_com_a_ui() {
        assert_eq!(modulos().len(), 4);
        assert_eq!(operacoes_canonicas().len(), 33);
        let escopo = escopo_fase1();
        assert_eq!(escopo.leitura.len(), 12);
        assert_eq!(escopo.escrita.len(), 4);
        assert_eq!(escopo.esperado_em_c.len(), 16);
        assert_eq!(escopo.modulo, "C");
        assert_eq!(entidades_canonicas().len(), 18);
    }

    #[test]
    fn cada_operacao_traz_os_4_modulos() {
        for op in operacoes_canonicas() {
            assert_eq!(op.evidencias.len(), 4, "operação {} sem os 4 módulos", op.id);
            assert!(op.evidencias.contains_key("C"), "operação {} sem coluna C", op.id);
        }
    }

    /// O app carrega os dois perfis das fixtures **embutidas** — é isso que garante que o `.exe`
    /// empacotado funcione sem a pasta `fixtures/` do lado.
    #[test]
    fn aplicacao_carrega_os_dois_perfis_do_binario() {
        let app = ne::Aplicacao::carregar();
        let fontes = ne::fontes_de(&app);
        assert_eq!(fontes.len(), 2);
        assert!(fontes.iter().all(|f| f.modelo.starts_with("MA5")), "{:?}", fontes.iter().map(|f| &f.modelo).collect::<Vec<_>>());
    }

    /// O caso de uso âncora, de ponta a ponta: lista da PON → clusters de queda.
    #[test]
    fn massiva_agrupa_as_quedas_da_pon_do_perfil() {
        let app = ne::Aplicacao::carregar();
        let fonte = ne::fontes_de(&app)[0].id.clone();
        let olt = ne::olts_de(&app, &fonte).unwrap()[0].id.clone();
        let pon = ne::pons_de(&app, &fonte, &olt).unwrap()[0].clone();

        let r = ne::massiva_de(&app, &fonte, &olt, pon.slot, pon.porta, Some(5)).unwrap();
        assert_eq!(r.janela_min, 5);
        assert_eq!(r.com_hora + r.sem_hora, pon.onus as usize, "toda ONU da PON entra na conta");
        assert!(r.maior_cluster >= 1);
    }

    #[test]
    fn operacao_destrutiva_esta_marcada_para_a_ui() {
        let destrutivas: Vec<_> = operacoes_canonicas().into_iter().filter(|o| o.destrutiva).collect();
        assert_eq!(destrutivas.len(), 6, "a UI precisa marcar as 6 destrutivas com ⚠");
    }
}
