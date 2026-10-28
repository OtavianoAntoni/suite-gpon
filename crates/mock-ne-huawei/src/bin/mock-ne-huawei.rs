//! `mock-ne-huawei` — o "NE" das Fases 0 e 1, por linha de comando.
//!
//!   mock-ne-huawei                     lista os perfis de fixture disponíveis
//!   mock-ne-huawei --model ma5800      carrega o perfil e mostra o que ele contém
//!   mock-ne-huawei --model ma5600t --catalogo   imprime o dicionário MML do perfil
//!
//! Não abre porta e não fala rede: nesta fase o mock é carga em memória (decisão do dono,
//! 2026-10-17). O servidor SSH/SNMP de verdade é Fase 2, contra OLT de laboratório.

use gpon_transport::testing::bloquear;
use gpon_transport::{CanonicalCmd, CanonicalResult, Transport};
use mock_ne_huawei::{Fixture, ModeloNe, PERFIS};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        println!("perfis disponíveis: {}", PERFIS.join(", "));
        println!("uso: mock-ne-huawei --model <perfil> [--catalogo]");
        return;
    }

    let modelo = match args.iter().position(|a| a == "--model") {
        Some(i) => match args.get(i + 1) {
            Some(v) => match ModeloNe::do_arg(v) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(2);
                }
            },
            None => {
                eprintln!("--model exige um valor ({})", PERFIS.join(", "));
                std::process::exit(2);
            }
        },
        None => {
            eprintln!("informe --model <perfil> (ou --help)");
            std::process::exit(2);
        }
    };

    let f = match Fixture::carregar(modelo) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("fixture inválida: {e}");
            std::process::exit(1);
        }
    };

    println!("perfil:        {}", f.modelo);
    println!("fontes:        {}", f.transporte.fontes.len());
    println!("OLTs:          {}", f.transporte.olts.len());
    println!("portas PON:    {}", f.transporte.pons.len());
    println!("ONUs:          {}", f.transporte.onus.len());
    println!("perfis:        {}", f.transporte.perfis.len());
    println!("alarmes:       {}", f.transporte.alarmes.len());
    if let Some(olt) = f.transporte.olts.first() {
        println!("OLT:           {} · {} · {}", olt.id, olt.modelo, olt.sw_version);
    }

    if args.iter().any(|a| a == "--catalogo") {
        println!("\ncatálogo MML do perfil:");
        for (op, cmd) in &f.catalogo_mml {
            println!("  {op:<30} {cmd}");
        }
    }

    // Verificação rápida: a fixture responde pelo contrato?
    let fonte = f.transporte.fontes[0].id.clone();
    let mut falhas = 0;
    for (rotulo, cmd) in [
        ("list_sources", CanonicalCmd::ListSources),
        ("list_olts", CanonicalCmd::ListOlts { source: fonte.clone() }),
        (
            "list_onus",
            CanonicalCmd::ListOnus {
                olt: f.transporte.olts[0].id.clone(),
                pon: None,
                filtro: None,
                segmento: gpon_transport::SegmentoOnu::Todas,
                page: gpon_transport::Page::primeira(10),
                ordenar_por: gpon_transport::OrdemOnu::Sn,
            },
        ),
    ] {
        match bloquear(f.transporte.exec(cmd)) {
            Ok(r) => println!("self-test {rotulo:<14} ok  ({})", r.tipo()),
            Err(e) => {
                println!("self-test {rotulo:<14} FALHOU ({e})");
                falhas += 1;
            }
        }
    }

    // Prova de que a resposta vem tipada (não é texto para o chamador parsear).
    if let Ok(CanonicalResult::Onus { total, .. }) = bloquear(f.transporte.exec(CanonicalCmd::ListOnus {
        olt: f.transporte.olts[0].id.clone(),
        pon: None,
        filtro: None,
        segmento: gpon_transport::SegmentoOnu::Todas,
        page: gpon_transport::Page::primeira(1),
        ordenar_por: gpon_transport::OrdemOnu::Sn,
    })) {
        println!("total de ONUs pelo contrato: {total}");
    }

    std::process::exit(if falhas == 0 { 0 } else { 1 });
}
