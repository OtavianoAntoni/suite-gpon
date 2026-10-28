//! Busca por critério, ordenação por hora de queda e os casos de borda do contrato.
//!
//! Estes testes existem por causa de duas coisas concretas: (a) o trabalho de campo do dono — ele
//! procura ONU por **SN, ALIAS ou VLAN** e a tarefa mais chata é identificar **possível massiva**
//! (§7.4/§7.5); (b) as três lacunas apontadas no pacote de revisão do 3a — **lista vazia**,
//! **volume alto** e **resposta inválida**.

use gpon_transport::testing::{bloquear, InMemoryTransport};
use gpon_transport::{
    CanonicalCmd, CanonicalResult, CriterioBusca, Module, Onu, OnuEstado, OnuId, OrdemOnu, OltId,
    Page, PonRef, SegmentoOnu, SourceId, Transport, TransportError,
};

fn fonte() -> SourceId {
    SourceId::novo("lab-1")
}

fn olt() -> OltId {
    OltId::novo("olt-ma5800-1")
}

/// ONU sintética para os testes (a fixture real tem só 2–3, e aqui precisamos de borda/volume).
fn onu(id: &str, sn: &str, nome: &str, desc: &str, ultimo: Option<&str>) -> Onu {
    Onu {
        id: OnuId::novo(id),
        olt: olt(),
        pon: PonRef::new(1, 1, 1, 1),
        sn: sn.to_string(),
        mac: None,
        modelo_hw: "HG8245H".into(),
        modelo_sw: "V5R019C10S270".into(),
        vendor: "Huawei".into(),
        nome: nome.to_string(),
        descricao: desc.to_string(),
        estado: OnuEstado::Online,
        distancia_m: None,
        ultima_queda: ultimo.map(str::to_string),
        ultimo_up: None,
        motivo_queda: None,
    }
}

fn com_onus(onus: Vec<Onu>) -> InMemoryTransport {
    let mut t = InMemoryTransport::amostra_dois_perfis();
    t.onus = onus;
    t
}

fn listar(t: &InMemoryTransport, page: Page, ordem: OrdemOnu) -> (usize, usize) {
    match bloquear(t.exec(CanonicalCmd::ListOnus {
        olt: olt(),
        pon: None,
        filtro: None,
        segmento: SegmentoOnu::Todas,
        page,
        ordenar_por: ordem,
    }))
    .expect("list_onus responde")
    {
        CanonicalResult::Onus { itens, total } => (itens.len(), total),
        outro => panic!("esperava Onus, veio {}", outro.tipo()),
    }
}

// ---------- busca (Find by... — §8.2) ----------

#[test]
fn busca_por_sn_com_valor_vazio_nao_devolve_o_primeiro_que_aparecer() {
    let t = com_onus(vec![onu("onu-1", "SN1", "a", "", None)]);

    let e = bloquear(t.exec(CanonicalCmd::BuscarOnu {
        source: fonte(),
        criterio: CriterioBusca::Sn,
        valor: "   ".into(),
    }))
    .expect_err("valor vazio não é busca");

    assert!(matches!(e, TransportError::NaoEncontrado { .. }), "veio {e:?}");
    assert!(e.to_string().contains("valor vazio"), "{e}");
}

#[test]
fn busca_por_alias_acha_pelo_nome_e_pela_descricao() {
    let t = com_onus(vec![
        onu("onu-1", "SN1", "cliente-0001", "portaria", None),
        onu("onu-2", "SN2", "cliente-0002", "loja", None),
    ]);

    for alvo in ["cliente-0002", "LOJA"] {
        let r = bloquear(t.exec(CanonicalCmd::BuscarOnu {
            source: fonte(),
            criterio: CriterioBusca::Alias,
            valor: alvo.into(),
        }))
        .expect("alias existe");

        match r {
            CanonicalResult::Onu(o) => assert_eq!(o.sn, "SN2"),
            outro => panic!("esperava Onu, veio {}", outro.tipo()),
        }
    }
}

#[test]
fn busca_por_alias_ambiguo_devolve_a_lista_em_vez_de_escolher_um() {
    // ALIAS não é único: dois clientes podem ter a mesma descrição ("loja").
    let t = com_onus(vec![
        onu("onu-1", "SN1", "cliente-0001", "loja centro", None),
        onu("onu-2", "SN2", "cliente-0002", "loja bairro", None),
    ]);

    let r = bloquear(t.exec(CanonicalCmd::BuscarOnu {
        source: fonte(),
        criterio: CriterioBusca::Alias,
        valor: "loja".into(),
    }))
    .expect("alias acha os dois");

    match r {
        CanonicalResult::Onus { itens, total } => {
            assert_eq!(total, 2);
            assert_eq!(itens.len(), 2);
        }
        outro => panic!("esperava Onus (ambíguo), veio {}", outro.tipo()),
    }
}

#[test]
fn busca_por_vlan_avisa_que_depende_da_fase_2() {
    let t = com_onus(vec![onu("onu-1", "SN1", "a", "", None)]);

    let e = bloquear(t.exec(CanonicalCmd::BuscarOnu {
        source: fonte(),
        criterio: CriterioBusca::Vlan,
        valor: "1000".into(),
    }))
    .expect_err("VLAN exige serviço (Fase 2)");

    assert!(
        matches!(e, TransportError::IndisponivelNestaFase { .. }),
        "veio {e:?}"
    );
    assert!(e.to_string().contains("Fase 2"), "{e}");
}

// ---------- ordenação (caso de uso âncora — §7.4) ----------

#[test]
fn ordem_por_ultima_queda_traz_o_mais_recente_primeiro_e_sem_queda_no_fim() {
    let t = com_onus(vec![
        onu("onu-antiga", "SN-ANTIGA", "a", "", Some("2026-10-17T10:00:00-03:00")),
        onu("onu-sem-registro", "SN-SEM-REG", "b", "", None),
        onu("onu-recente", "SN-RECENTE", "c", "", Some("2026-10-17T14:06:00-03:00")),
    ]);

    let r = bloquear(t.exec(CanonicalCmd::ListOnus {
        olt: olt(),
        pon: None,
        filtro: None,
        segmento: SegmentoOnu::Todas,
        page: Page::primeira(10),
        ordenar_por: OrdemOnu::UltimaQueda,
    }))
    .expect("list_onus ordenado");

    match r {
        CanonicalResult::Onus { itens, .. } => {
            let sns: Vec<&str> = itens.iter().map(|o| o.sn.as_str()).collect();
            assert_eq!(
                sns,
                vec!["SN-RECENTE", "SN-ANTIGA", "SN-SEM-REG"],
                "mais recente primeiro; sem registro no fim"
            );
        }
        outro => panic!("esperava Onus, veio {}", outro.tipo()),
    }
}

// ---------- as três lacunas do 3a ----------

#[test]
fn fonte_valida_sem_onus_devolve_lista_vazia_e_total_zero() {
    let t = com_onus(vec![]);
    let (itens, total) = listar(&t, Page::primeira(100), OrdemOnu::Sn);

    assert_eq!(itens, 0, "lista vazia é resposta válida, não erro");
    assert_eq!(total, 0);
}

#[test]
fn mil_e_vinte_e_quatro_onus_pagina_conta_e_ordena() {
    // Uma OLT de campo tem centenas de ONUs; 1024 é ordem de grandeza real de PON densa.
    let mut onus = Vec::with_capacity(1024);
    for i in 0..1024u32 {
        // timestamps sintéticos, zero-padded: ordem lexicográfica == cronológica
        let ts = format!("2026-10-17T{:02}:{:02}:00-03:00", i / 60, i % 60);
        onus.push(onu(
            &format!("onu-{i:04}"),
            &format!("SN{i:016X}"),
            &format!("cliente-{i:04}"),
            "",
            Some(&ts),
        ));
    }
    let t = com_onus(onus);

    // janela de 100, total real
    let (na_janela, total) = listar(&t, Page::primeira(100), OrdemOnu::Sn);
    assert_eq!(na_janela, 100, "a janela respeita o limite");
    assert_eq!(total, 1024, "o total é o do NE, não o da janela");

    // segunda página não repete a primeira
    let primeira = bloquear(t.exec(CanonicalCmd::ListOnus {
        olt: olt(), pon: None, filtro: None, segmento: SegmentoOnu::Todas,
        page: Page { offset: 0, limit: 10 }, ordenar_por: OrdemOnu::Sn,
    })).expect("página 1");
    let segunda = bloquear(t.exec(CanonicalCmd::ListOnus {
        olt: olt(), pon: None, filtro: None, segmento: SegmentoOnu::Todas,
        page: Page { offset: 10, limit: 10 }, ordenar_por: OrdemOnu::Sn,
    })).expect("página 2");

    let (CanonicalResult::Onus { itens: a, .. }, CanonicalResult::Onus { itens: b, .. }) = (primeira, segunda) else {
        panic!("list_onus devolveu tipo inesperado")
    };
    let sns_a: Vec<&str> = a.iter().map(|o| o.sn.as_str()).collect();
    let sns_b: Vec<&str> = b.iter().map(|o| o.sn.as_str()).collect();
    assert!(sns_a.iter().all(|s| !sns_b.contains(s)), "páginas se sobrepõem: {sns_a:?} × {sns_b:?}");

    // ordenação por hora de queda em volume: o mais recente (i=1023 → 17:03) vem primeiro
    let ordenado = bloquear(t.exec(CanonicalCmd::ListOnus {
        olt: olt(), pon: None, filtro: None, segmento: SegmentoOnu::Todas,
        page: Page { offset: 0, limit: 5 }, ordenar_por: OrdemOnu::UltimaQueda,
    })).expect("ordenado por queda");
    match ordenado {
        CanonicalResult::Onus { itens, total } => {
            assert_eq!(total, 1024);
            assert_eq!(itens[0].sn, "SN00000000000003FF", "o mais recente deve vir primeiro");
        }
        outro => panic!("esperava Onus, veio {}", outro.tipo()),
    }
}

#[test]
fn busca_por_sn_continua_funcionando_em_volume() {
    // A busca não pode depender de a ONU estar na primeira página.
    let mut onus = Vec::with_capacity(500);
    for i in 0..500u32 {
        onus.push(onu(&format!("onu-{i:04}"), &format!("SN{i:016X}"), "", "", None));
    }
    let t = com_onus(onus);

    let r = bloquear(t.exec(CanonicalCmd::BuscarOnu {
        source: fonte(),
        criterio: CriterioBusca::Sn,
        valor: "sn00000000000001f3".into(), // minúsculo de propósito: comparação normalizada
    }))
    .expect("acha a ONU 499");

    match r {
        CanonicalResult::Onu(o) => assert_eq!(o.id.as_str(), "onu-0499"),
        outro => panic!("esperava Onu, veio {}", outro.tipo()),
    }
}

#[test]
fn modulo_que_nao_serve_a_operacao_continua_recusando() {
    // Guarda-corpo da §1.3 exercitado também na busca (A não tem `find_onu_by_sn`).
    let a = InMemoryTransport::new(Module::A);
    let e = bloquear(a.exec(CanonicalCmd::BuscarOnu {
        source: fonte(),
        criterio: CriterioBusca::Sn,
        valor: "QUALQUER".into(),
    }))
    .expect_err("A não serve find_onu_by_sn");

    assert!(matches!(e, TransportError::NaoSuportado { .. }), "veio {e:?}");
}
