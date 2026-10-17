//! Os **16 comandos da Fase 1 rodando duas vezes** — uma por família de fixture (decisão P4).
//!
//! Cada operação vira um teste próprio em cada perfil, então uma falha aponta a operação **e** a
//! família. O que os testes provam, além do contrato: as fixtures são de fato *parametrizadas* —
//! números, perfis e alarmes diferentes entre MA5800 e MA5600T, com o MESMO código de transporte.

use gpon_transport::testing::bloquear;
use gpon_transport::{
    AuthMode, CanonicalCmd, CanonicalResult, OltId, OnuId, Page, PonRef, ProfileKind, SegmentoOnu,
    SourceId, Transport,
};
use mock_ne_huawei::{Fixture, ModeloNe};
use std::collections::BTreeSet;

struct Expectativa {
    modelo: &'static str,
    slots: u16,
    split: u8,
    pons: usize,
    onus: usize,
    perfil_line: &'static str,
    alarmes: usize,
}

fn expectativa(m: ModeloNe) -> Expectativa {
    match m {
        ModeloNe::Ma5800 => Expectativa {
            modelo: "MA5800-X7",
            slots: 17,
            split: 128,
            pons: 2,
            onus: 2,
            perfil_line: "line-1G",
            alarmes: 1,
        },
        ModeloNe::Ma5600t => Expectativa {
            modelo: "MA5600T",
            slots: 16,
            split: 64,
            pons: 2,
            onus: 3,
            perfil_line: "line-512M",
            alarmes: 2,
        },
    }
}

fn fonte() -> SourceId {
    SourceId::novo("lab-1")
}

fn carregar(m: ModeloNe) -> Fixture {
    Fixture::carregar(m).unwrap_or_else(|e| panic!("fixture do perfil {m}: {e}"))
}

fn olt(f: &Fixture) -> OltId {
    f.transporte.olts[0].id.clone()
}

fn onu_id(f: &Fixture) -> OnuId {
    f.transporte.onus[0].id.clone()
}

// ---------- as 16 operações ----------

fn verifica_op01_list_sources(f: &Fixture) {
    let r = bloquear(f.transporte.exec(CanonicalCmd::ListSources)).expect("list_sources");
    match r {
        CanonicalResult::Sources(v) => {
            assert_eq!(v.len(), 1);
            assert_eq!(v[0].id.as_str(), "lab-1");
            // §9: no produto a credencial é REFERÊNCIA ao cofre, nunca o segredo.
            assert!(v[0].credencial_ref.starts_with("cofre://"), "credencial não é referência de cofre");
        }
        outro => panic!("esperava Sources, veio {}", outro.tipo()),
    }
}

fn verifica_op02_test_connectivity(f: &Fixture) {
    let r = bloquear(f.transporte.exec(CanonicalCmd::TestConnectivity { source: fonte() }))
        .expect("test_connectivity");
    match r {
        CanonicalResult::Connectivity(c) => {
            assert!(c.alcancou_servidor);
            assert_eq!(c.alcancou_ne, Some(true));
        }
        outro => panic!("esperava Connectivity, veio {}", outro.tipo()),
    }
}

fn verifica_op03_list_olts(f: &Fixture, e: &Expectativa) {
    let r = bloquear(f.transporte.exec(CanonicalCmd::ListOlts { source: fonte() })).expect("list_olts");
    match r {
        CanonicalResult::Olts(v) => {
            assert_eq!(v.len(), 1);
            assert_eq!(v[0].modelo, e.modelo, "modelo do perfil");
            assert_eq!(v[0].vendor, "Huawei");
        }
        outro => panic!("esperava Olts, veio {}", outro.tipo()),
    }
}

fn verifica_op04_get_olt_info(f: &Fixture, e: &Expectativa) {
    let r = bloquear(f.transporte.exec(CanonicalCmd::GetOltInfo { olt: olt(f) })).expect("get_olt_info");
    match r {
        CanonicalResult::Olt(o) => {
            assert_eq!(o.slots, e.slots, "slots do perfil");
            assert!(o.temperatura_c.is_some(), "fixture tem sensor de temperatura");
        }
        outro => panic!("esperava Olt, veio {}", outro.tipo()),
    }
}

fn verifica_op05_list_pon_ports(f: &Fixture, e: &Expectativa) {
    let r = bloquear(f.transporte.exec(CanonicalCmd::ListPonPorts { olt: olt(f) }))
        .expect("list_pon_ports");
    match r {
        CanonicalResult::PonPorts(v) => {
            assert_eq!(v.len(), e.pons, "portas PON do perfil");
            let primeira = v.iter().find(|p| p.pon == PonRef::new(0, 1)).expect("porta 0/1");
            assert_eq!(primeira.split_ratio, e.split, "split ratio do perfil");
            assert!(primeira.admin_up);
        }
        outro => panic!("esperava PonPorts, veio {}", outro.tipo()),
    }
}

fn verifica_op06_list_onus(f: &Fixture, e: &Expectativa) {
    let r = bloquear(f.transporte.exec(CanonicalCmd::ListOnus {
        olt: olt(f),
        pon: None,
        filtro: None,
        segmento: SegmentoOnu::Todas,
        page: Page::primeira(100),
    }))
    .expect("list_onus");

    match r {
        CanonicalResult::Onus { itens, total } => {
            assert_eq!(total, e.onus, "total de ONUs do perfil");
            assert_eq!(itens.len(), e.onus, "cabem todos na janela de 100");
        }
        outro => panic!("esperava Onus, veio {}", outro.tipo()),
    }

    // janela de 1: o total continua sendo o do NE (paginação correta)
    let r = bloquear(f.transporte.exec(CanonicalCmd::ListOnus {
        olt: olt(f),
        pon: None,
        filtro: None,
        segmento: SegmentoOnu::Todas,
        page: Page { offset: 0, limit: 1 },
    }))
    .expect("list_onus paginado");
    match r {
        CanonicalResult::Onus { itens, total } => {
            assert_eq!(itens.len(), 1);
            assert_eq!(total, e.onus);
        }
        outro => panic!("esperava Onus, veio {}", outro.tipo()),
    }
}

fn verifica_op07_find_onu_by_sn(f: &Fixture) {
    let sn = f.transporte.onus[0].sn.clone();
    let r = bloquear(f.transporte.exec(CanonicalCmd::FindOnuBySn { source: fonte(), sn: sn.clone() }))
        .expect("find_onu_by_sn");
    match r {
        CanonicalResult::Onu(o) => assert_eq!(o.sn, sn, "achou pela SN"),
        outro => panic!("esperava Onu, veio {}", outro.tipo()),
    }
}

fn verifica_op08_get_onu(f: &Fixture) {
    let r = bloquear(f.transporte.exec(CanonicalCmd::GetOnu { onu: onu_id(f) })).expect("get_onu");
    match r {
        CanonicalResult::Onu(o) => {
            assert_eq!(o.id, onu_id(f));
            assert!(!o.modelo_hw.is_empty(), "modelo de hardware presente");
        }
        outro => panic!("esperava Onu, veio {}", outro.tipo()),
    }
}

fn verifica_op09_get_onu_auth_mode(f: &Fixture) {
    let r = bloquear(f.transporte.exec(CanonicalCmd::GetOnuAuthMode { onu: onu_id(f) }))
        .expect("get_onu_auth_mode");
    match r {
        CanonicalResult::AuthMode(m) => assert_eq!(m, AuthMode::Sn),
        outro => panic!("esperava AuthMode, veio {}", outro.tipo()),
    }
}

fn verifica_op10_list_profiles(f: &Fixture, e: &Expectativa) {
    let r = bloquear(f.transporte.exec(CanonicalCmd::ListProfiles {
        source: fonte(),
        kind: ProfileKind::Line,
    }))
    .expect("list_profiles");
    match r {
        CanonicalResult::Profiles(v) => {
            assert!(!v.is_empty());
            let nomes: BTreeSet<&str> = v.iter().map(|p| p.nome.as_str()).collect();
            assert!(
                nomes.contains(e.perfil_line),
                "perfil de linha esperado {:?} não está em {:?}",
                e.perfil_line,
                nomes
            );
        }
        outro => panic!("esperava Profiles, veio {}", outro.tipo()),
    }
}

fn verifica_op11_list_alarms(f: &Fixture, e: &Expectativa) {
    let r = bloquear(f.transporte.exec(CanonicalCmd::ListAlarms { source: fonte(), filtro: None }))
        .expect("list_alarms");
    match r {
        CanonicalResult::Alarms(v) => {
            assert_eq!(v.len(), e.alarmes, "alarmes do perfil");
            // ordenação por severidade (Crítica antes de Maior antes de Aviso)
            let mut anterior = v[0].severidade;
            for a in &v[1..] {
                assert!(anterior <= a.severidade, "alarmes fora de ordem de severidade");
                anterior = a.severidade;
            }
        }
        outro => panic!("esperava Alarms, veio {}", outro.tipo()),
    }
}

fn verifica_op12_export_inventory(f: &Fixture, e: &Expectativa) {
    let r = bloquear(f.transporte.exec(CanonicalCmd::ExportInventory { filtro: None }))
        .expect("export_inventory");
    match r {
        CanonicalResult::Export(x) => assert_eq!(x.linhas, e.onus, "linhas exportadas = ONUs do perfil"),
        outro => panic!("esperava Export, veio {}", outro.tipo()),
    }
}

fn verifica_op13_authorize_onu(f: &Fixture) {
    let sn = "48575443000000AA";
    let r = bloquear(f.transporte.exec(CanonicalCmd::AuthorizeOnu {
        olt: olt(f),
        pon: PonRef::new(0, 1),
        sn: sn.to_string(),
        modo: AuthMode::Sn,
        perfil: Some("line".into()),
    }))
    .expect("authorize_onu");
    match r {
        CanonicalResult::Ack(a) => {
            assert!(a.ok);
            assert!(a.mensagem.contains(sn));
        }
        outro => panic!("esperava Ack, veio {}", outro.tipo()),
    }
}

fn verifica_op14_deauthorize_onu(f: &Fixture) {
    let r = bloquear(f.transporte.exec(CanonicalCmd::DeauthorizeOnu { onu: onu_id(f) }))
        .expect("deauthorize_onu");
    match r {
        CanonicalResult::Ack(a) => assert!(a.ok),
        outro => panic!("esperava Ack, veio {}", outro.tipo()),
    }
}

fn verifica_op15_reset_onu(f: &Fixture) {
    let r = bloquear(f.transporte.exec(CanonicalCmd::ResetOnu { onu: onu_id(f) })).expect("reset_onu");
    match r {
        CanonicalResult::Ack(a) => {
            assert!(a.ok);
            assert!(a.id_operacao.is_some(), "reset é assíncrono: precisa devolver id de operação");
        }
        outro => panic!("esperava Ack, veio {}", outro.tipo()),
    }
}

fn verifica_op16_set_onu_description(f: &Fixture) {
    let r = bloquear(f.transporte.exec(CanonicalCmd::SetOnuDescription {
        onu: onu_id(f),
        texto: "sala-tecnica".into(),
    }))
    .expect("set_onu_description");
    match r {
        CanonicalResult::Ack(a) => {
            assert!(a.ok);
            assert!(a.mensagem.contains("sala-tecnica"));
        }
        outro => panic!("esperava Ack, veio {}", outro.tipo()),
    }
}

// ---------- 16 testes por perfil ----------

macro_rules! suite_do_perfil {
    ($modulo:ident, $modelo:expr) => {
        mod $modulo {
            use super::*;

            #[test]
            fn op01_list_sources() {
                let f = carregar($modelo);
                verifica_op01_list_sources(&f);
            }
            #[test]
            fn op02_test_connectivity() {
                let f = carregar($modelo);
                verifica_op02_test_connectivity(&f);
            }
            #[test]
            fn op03_list_olts() {
                let f = carregar($modelo);
                let e = expectativa($modelo);
                verifica_op03_list_olts(&f, &e);
            }
            #[test]
            fn op04_get_olt_info() {
                let f = carregar($modelo);
                let e = expectativa($modelo);
                verifica_op04_get_olt_info(&f, &e);
            }
            #[test]
            fn op05_list_pon_ports() {
                let f = carregar($modelo);
                let e = expectativa($modelo);
                verifica_op05_list_pon_ports(&f, &e);
            }
            #[test]
            fn op06_list_onus() {
                let f = carregar($modelo);
                let e = expectativa($modelo);
                verifica_op06_list_onus(&f, &e);
            }
            #[test]
            fn op07_find_onu_by_sn() {
                let f = carregar($modelo);
                verifica_op07_find_onu_by_sn(&f);
            }
            #[test]
            fn op08_get_onu() {
                let f = carregar($modelo);
                verifica_op08_get_onu(&f);
            }
            #[test]
            fn op09_get_onu_auth_mode() {
                let f = carregar($modelo);
                verifica_op09_get_onu_auth_mode(&f);
            }
            #[test]
            fn op10_list_profiles() {
                let f = carregar($modelo);
                let e = expectativa($modelo);
                verifica_op10_list_profiles(&f, &e);
            }
            #[test]
            fn op11_list_alarms() {
                let f = carregar($modelo);
                let e = expectativa($modelo);
                verifica_op11_list_alarms(&f, &e);
            }
            #[test]
            fn op12_export_inventory() {
                let f = carregar($modelo);
                let e = expectativa($modelo);
                verifica_op12_export_inventory(&f, &e);
            }
            #[test]
            fn op13_authorize_onu() {
                let f = carregar($modelo);
                verifica_op13_authorize_onu(&f);
            }
            #[test]
            fn op14_deauthorize_onu() {
                let f = carregar($modelo);
                verifica_op14_deauthorize_onu(&f);
            }
            #[test]
            fn op15_reset_onu() {
                let f = carregar($modelo);
                verifica_op15_reset_onu(&f);
            }
            #[test]
            fn op16_set_onu_description() {
                let f = carregar($modelo);
                verifica_op16_set_onu_description(&f);
            }
        }
    };
}

suite_do_perfil!(perfil_ma5800, ModeloNe::Ma5800);
suite_do_perfil!(perfil_ma5600t, ModeloNe::Ma5600t);

// ---------- o que só faz sentido comparando os dois perfis ----------

#[test]
fn os_catalogos_mml_diferem_entre_as_familias() {
    let a = carregar(ModeloNe::Ma5800);
    let b = carregar(ModeloNe::Ma5600t);
    let diferentes: Vec<&String> = a
        .catalogo_mml
        .iter()
        .filter(|(op, cmd)| b.catalogo_mml.get(*op).is_some_and(|c| c != *cmd))
        .map(|(op, _)| op)
        .collect();
    assert!(
        !diferentes.is_empty(),
        "se os dois catálogos são idênticos, o perfil não está parametrizando nada"
    );
    // o dicionário é o que muda entre famílias; o conjunto de operações é o mesmo
    assert_eq!(
        a.catalogo_mml.keys().collect::<Vec<_>>(),
        b.catalogo_mml.keys().collect::<Vec<_>>(),
        "as duas famílias cobrem as mesmas operações canônicas"
    );
}

#[test]
fn os_dois_perfis_devolvem_dados_diferentes_com_o_mesmo_codigo() {
    let a = carregar(ModeloNe::Ma5800);
    let b = carregar(ModeloNe::Ma5600t);

    let olts_a = bloquear(a.transporte.exec(CanonicalCmd::ListOlts { source: fonte() })).unwrap();
    let olts_b = bloquear(b.transporte.exec(CanonicalCmd::ListOlts { source: fonte() })).unwrap();

    let (CanonicalResult::Olts(va), CanonicalResult::Olts(vb)) = (olts_a, olts_b) else {
        panic!("list_olts devolveu tipo inesperado")
    };
    assert_ne!(va[0].modelo, vb[0].modelo, "mesma chamada, famílias diferentes");
    assert_ne!(va[0].ip, vb[0].ip);
    assert_ne!(va[0].sw_version, vb[0].sw_version);
}

#[test]
fn nenhuma_fixture_carrega_segredo_em_texto() {
    for m in ModeloNe::todos() {
        let f = carregar(m);
        for s in &f.transporte.fontes {
            assert!(
                s.credencial_ref.starts_with("cofre://"),
                "perfil {m}: credencial deveria ser referência de cofre, veio {:?}",
                s.credencial_ref
            );
        }
    }
}
