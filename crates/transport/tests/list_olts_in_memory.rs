//! O teste do passo 3a: **`list_olts` contra um transporte em memória**.
//!
//! Ele vale por três coisas ao mesmo tempo:
//!  1. prova que o trait `Transport` é executável por um chamador que só conhece o contrato
//!     (`&dyn Transport`) — é isso que permite trocar mock por rede sem tocar no chamador;
//!  2. prova que o resultado vem **tipado** (não texto para a UI parsear);
//!  3. trava a correspondência entre os comandos canônicos e o escopo da Fase 1 da SPEC.

use gpon_transport::testing::{bloquear, InMemoryTransport};
use gpon_transport::{
    CanonicalCmd, CanonicalResult, Module, OltId, OrdemOnu, Page, SegmentoOnu, SourceId,
    Transport,
    TransportError,
};
use std::collections::BTreeSet;

fn mock() -> InMemoryTransport {
    InMemoryTransport::amostra_dois_perfis()
}

#[test]
fn list_olts_contra_transporte_em_memoria() {
    let t = mock();
    // O chamador só enxerga o CONTRATO — não sabe que é memória, não sabe que é Huawei.
    let contrato: &dyn Transport = &t;

    let r = bloquear(contrato.exec(CanonicalCmd::ListOlts {
        source: SourceId::novo("lab-1"),
    }))
    .expect("list_olts deve responder a partir do mock em memória");

    match r {
        CanonicalResult::Olts(olts) => {
            assert_eq!(olts.len(), 2, "a amostra tem as duas famílias de fixture (P4)");

            // Contrato de ordenação: por id, ascendente, determinístico. A UI não pode depender
            // da ordem de descoberta do NE (que varia entre firmwares).
            let ids: Vec<&str> = olts.iter().map(|o| o.id.as_str()).collect();
            assert_eq!(ids, vec!["olt-ma5600t-1", "olt-ma5800-1"], "ordem por id");

            // As duas famílias de fixture decididas em P4 (§11.3).
            let modelos: BTreeSet<&str> = olts.iter().map(|o| o.modelo.as_str()).collect();
            assert_eq!(
                modelos,
                ["MA5600T", "MA5800-X7"].into_iter().collect::<BTreeSet<_>>(),
                "o mock nasce com MA5800 e MA5600T"
            );

            for olt in &olts {
                assert_eq!(olt.vendor, "Huawei");
                assert!(!olt.sw_version.is_empty(), "versão de software vazia em {}", olt.id);
                assert_eq!(olt.fonte.as_str(), "lab-1");
            }

            // Campos acessados por nome: o resultado é tipado, não texto para a UI parsear.
            let por_modelo = |m: &str| {
                olts.iter().find(|o| o.modelo == m).unwrap_or_else(|| panic!("faltou {m}"))
            };
            assert_eq!(por_modelo("MA5800-X7").slots, 17);
            assert!(por_modelo("MA5600T").padrao_temperatura_ok(), "temperatura do MA5600T fora do esperado");
        }
        outro => panic!("esperava CanonicalResult::Olts, veio {}", outro.tipo()),
    }
}

/// Extensão de leitura usada no teste, para deixar claro que temperatura é opcional (nem todo NE
/// expõe sensor — em A, por exemplo, não há evidência de leitura de temperatura).
trait TemperaturaOpcional {
    fn padrao_temperatura_ok(&self) -> bool;
}

impl TemperaturaOpcional for gpon_transport::Olt {
    fn padrao_temperatura_ok(&self) -> bool {
        matches!(self.temperatura_c, Some(t) if t < 60)
    }
}

#[test]
fn fonte_desconhecida_devolve_nao_encontrado() {
    let t = mock();
    let e = bloquear(t.exec(CanonicalCmd::ListOlts {
        source: SourceId::novo("fonte-que-nao-existe"),
    }))
    .expect_err("fonte inexistente não pode devolver lista vazia como se estivesse tudo bem");

    assert!(
        matches!(e, TransportError::NaoEncontrado { .. }),
        "veio {e:?}"
    );
    assert!(e.to_string().contains("fonte-que-nao-existe"));
}

#[test]
fn listagem_de_onus_pagina_e_devolve_o_total() {
    let t = mock();
    let r = bloquear(t.exec(CanonicalCmd::ListOnus {
        olt: OltId::novo("olt-ma5800-1"),
        pon: None,
        filtro: None,
        segmento: SegmentoOnu::Todas,
        page: Page { offset: 0, limit: 1 },
        ordenar_por: OrdemOnu::Sn,
    }))
    .expect("list_onus responde");

    match r {
        CanonicalResult::Onus { itens, total } => {
            assert_eq!(itens.len(), 1, "a janela respeita o limite pedido");
            assert_eq!(total, 2, "o total é o do NE, não o da janela (§1.1: paginação obrigatória)");
        }
        outro => panic!("esperava Onus, veio {}", outro.tipo()),
    }
}

#[test]
fn operacao_que_o_modulo_nao_serve_e_recusada_pelo_dominio() {
    // Em A, `list_olts` é `—` na §1.3 (o catálogo mora no servidor AMS). O mock não pode
    // "ajudar" e responder: ele consulta o próprio domínio antes.
    let a = InMemoryTransport::new(Module::A); // sem dados, mas com a evidência de A
    let e = bloquear(a.exec(CanonicalCmd::ListOlts {
        source: SourceId::novo("lab-1"),
    }))
    .expect_err("A não oferece list_olts");

    assert!(matches!(e, TransportError::NaoSuportado { .. }), "veio {e:?}");
    assert!(e.to_string().contains("list_olts"));
}

#[test]
fn toda_operacao_do_escopo_da_fase_1_tem_comando_canonico() {
    // O enum de comandos e o escopo da §7.1 têm de ser o MESMO conjunto.
    let do_enum: BTreeSet<&str> =
        CanonicalCmd::todos_do_escopo_fase1().iter().map(|c| c.id()).collect();

    let da_spec: BTreeSet<&str> = gpon_domain::phase1::PHASE1_READS
        .iter()
        .chain(gpon_domain::phase1::PHASE1_WRITES.iter())
        .copied()
        .collect();

    assert_eq!(
        do_enum, da_spec,
        "comandos canônicos ≠ escopo da Fase 1 (faltando: {:?}; sobrando: {:?})",
        da_spec.difference(&do_enum).collect::<Vec<_>>(),
        do_enum.difference(&da_spec).collect::<Vec<_>>()
    );

    // E todo id do enum existe na §1.3 do domínio.
    for cmd in CanonicalCmd::todos_do_escopo_fase1() {
        assert!(
            gpon_domain::ops::by_id(cmd.id()).is_some(),
            "`{}` não existe na §1.3",
            cmd.id()
        );
    }
}

#[test]
fn marca_de_destrutiva_vem_do_dominio_nao_do_transporte() {
    let reset = CanonicalCmd::ResetOnu { onu: gpon_transport::OnuId::novo("onu-1") };
    let desativar = CanonicalCmd::DeauthorizeOnu { onu: gpon_transport::OnuId::novo("onu-1") };
    let listar = CanonicalCmd::ListOlts { source: SourceId::novo("lab-1") };

    assert!(reset.destrutiva(), "reset_onu é ⚠ na §1.3");
    assert!(desativar.destrutiva(), "deauthorize_onu é ⚠ na §1.3");
    assert!(!listar.destrutiva(), "list_olts não é destrutiva");
}

#[test]
fn o_transporte_registra_o_caminho_canonico_executado() {
    // Um caminho canônico por operação (§9): a trilha mostra exatamente o id da §1.3.
    let t = mock();
    let _ = bloquear(t.exec(CanonicalCmd::ListOlts { source: SourceId::novo("lab-1") }));
    let _ = bloquear(t.exec(CanonicalCmd::GetOltInfo {
        olt: OltId::novo("olt-ma5800-1"),
    }));

    assert_eq!(t.chamadas(), vec!["list_olts", "get_olt_info"]);
}

#[test]
fn erro_de_rede_configurado_derruba_a_chamada() {
    // Este é o caminho que a Fase 2 vai exercitar de verdade; aqui só provamos que o erro
    // atravessa o contrato sem virar string solta.
    let mut t = mock();
    t.derrubar = Some(TransportError::Rede { mensagem: "connection refused".into() });

    let e = bloquear(t.exec(CanonicalCmd::ListOlts { source: SourceId::novo("lab-1") }))
        .expect_err("transporte derrubado deve falhar");
    assert_eq!(e, TransportError::Rede { mensagem: "connection refused".into() });
    assert!(e.to_string().starts_with("falha de rede:"));
}
