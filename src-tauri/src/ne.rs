//! Ponte entre a UI e o NE (Fases 0/1: o NE é o `mock-ne-huawei`, embutido no binário).
//!
//! Duas regras:
//! - **Todo acesso passa por `CanonicalCmd`** — a UI nunca lê campo de struct direto do mock.
//!   Trocar o mock por SSH na Fase 2 não muda nenhum comando daqui.
//! - **A UI nunca recebe mais do que precisa**: os DTOs são achatados e trazem rótulo pronto
//!   (estado, tecnologia), porque formatação de domínio na tela é onde o legado erra primeiro.

use gpon_transport::analise::{agrupar_massiva, JANELA_PADRAO_MIN};
use gpon_transport::testing::bloquear;
use gpon_transport::{
    CanonicalCmd, CanonicalResult, CriterioBusca, OltId, OrdemOnu, Page, PonRef, SegmentoOnu,
    SourceId, Transport,
};
use mock_ne_huawei::{Fixture, ModeloNe};
use serde::Serialize;

/// Um NE carregado: o perfil (família) e o transporte que responde por ele.
pub struct Ne {
    pub fonte: SourceId,
    pub rotulo: String,
    pub perfil: String,
    pub modelo: String,
    transporte: gpon_transport::testing::InMemoryTransport,
}

pub struct Aplicacao {
    nes: Vec<Ne>,
}

impl Aplicacao {
    /// Carrega os dois perfis das fixtures embutidas. Falhar aqui é bug de build, não de ambiente:
    /// por isso o `expect` (e o CI roda este caminho nos testes).
    pub fn carregar() -> Self {
        let mut nes = Vec::new();
        for (modelo, rotulo) in
            [(ModeloNe::Ma5800, "lab-ma5800"), (ModeloNe::Ma5600t, "lab-ma5600t")]
        {
            let f = Fixture::carregar_embutido(modelo).expect("fixture embutida inválida");
            let fonte = f.transporte.fontes[0].id.clone();
            nes.push(Ne {
                fonte,
                rotulo: rotulo.to_string(),
                perfil: modelo.perfil().to_string(),
                modelo: f.transporte.olts[0].modelo.clone(),
                transporte: f.transporte,
            });
        }
        Self { nes }
    }

    fn ne(&self, fonte: &str) -> Result<&Ne, String> {
        self.nes
            .iter()
            .find(|ne| ne.fonte.as_str() == fonte)
            .ok_or_else(|| format!("fonte desconhecida: {fonte}"))
    }
}

// ---------- DTOs (o que a tela recebe) ----------

#[derive(Serialize)]
pub struct FonteDto {
    pub id: String,
    pub rotulo: String,
    pub perfil: String,
    pub modelo: String,
    pub host: String,
    pub protocolo: String,
    /// Sempre `C` nesta fase: o mock é a família Huawei.
    pub modulo: String,
}

#[derive(Serialize)]
pub struct OltDto {
    pub id: String,
    pub modelo: String,
    pub vendor: String,
    pub ip: String,
    pub sw_version: String,
    pub slots: u16,
    pub uptime_dias: u64,
    pub temperatura_c: Option<i16>,
}

#[derive(Serialize, Clone)]
pub struct PonDto {
    pub olt: String,
    pub slot: u8,
    pub porta: u8,
    pub rotulo: String,
    pub tecnologia: String,
    pub admin_up: bool,
    pub split_ratio: u8,
    pub onus: u32,
}

#[derive(Serialize)]
pub struct OnuDto {
    pub id: String,
    pub olt: String,
    pub slot: u8,
    pub porta: u8,
    pub sn: String,
    pub nome: String,
    pub descricao: String,
    pub estado: String,
    pub modelo_hw: String,
    pub modelo_sw: String,
    pub vendor: String,
    /// `None` = o NE não informou distância (não é zero!).
    pub distancia_m: Option<u32>,
    /// `None` = o NE não informou hora de registro/queda.
    pub ultimo_registro: Option<String>,
}

#[derive(Serialize)]
pub struct PaginaOnusDto {
    /// Total no NE, não o tamanho da página.
    pub total: usize,
    pub itens: Vec<OnuDto>,
    /// Explica degradação na cara do operador (ex.: NE sem hora de queda).
    pub aviso: Option<String>,
    pub modulo: String,
}

#[derive(Serialize)]
pub struct BuscaDto {
    pub criterio: String,
    pub valor: String,
    /// Zero, um ou vários (ALIAS não é único).
    pub itens: Vec<OnuDto>,
    pub aviso: Option<String>,
    pub modulo: String,
}

#[derive(Serialize)]
pub struct QuedaDto {
    pub sn: String,
    pub nome: String,
    pub quando: String,
}

#[derive(Serialize)]
pub struct ClusterDto {
    /// Hora da queda mais antiga do grupo (texto do NE, sem reformatar).
    pub inicio: String,
    pub fim: String,
    pub tamanho: usize,
    pub duracao_min: i64,
    pub quedas: Vec<QuedaDto>,
}

#[derive(Serialize)]
pub struct MassivaDto {
    pub janela_min: i64,
    /// ONUs da PON que entraram na análise (as que têm hora de queda).
    pub com_hora: usize,
    /// ONUs da PON sem hora de queda — a tela precisa dizer isso, não esconder.
    pub sem_hora: usize,
    /// Maior grupo = candidato a massiva.
    pub maior_cluster: usize,
    pub clusters: Vec<ClusterDto>,
    pub modulo: String,
}

// ---------- conversões ----------

fn onu_dto(o: &gpon_transport::Onu) -> OnuDto {
    OnuDto {
        id: o.id.as_str().to_string(),
        olt: o.olt.as_str().to_string(),
        slot: o.pon.slot,
        porta: o.pon.porta,
        sn: o.sn.clone(),
        nome: o.nome.clone(),
        descricao: o.descricao.clone(),
        estado: o.estado.as_str().to_string(),
        modelo_hw: o.modelo_hw.clone(),
        modelo_sw: o.modelo_sw.clone(),
        vendor: o.vendor.clone(),
        distancia_m: o.distancia_m,
        ultimo_registro: o.ultimo_registro.clone(),
    }
}

fn onus_de(ne: &Ne, olt: &str, pon: Option<(u8, u8)>, filtro: Option<String>, limit: usize, ordem: OrdemOnu) -> Result<PaginaOnusDto, String> {
    let cmd = CanonicalCmd::ListOnus {
        olt: OltId::novo(olt),
        pon: pon.map(|(s, p)| PonRef::new(s, p)),
        filtro,
        segmento: SegmentoOnu::Todas,
        page: Page { offset: 0, limit },
        ordenar_por: ordem,
    };
    match bloquear(ne.transporte.exec(cmd)).map_err(|e| e.to_string())? {
        CanonicalResult::Onus { itens, total } => {
            let sem_hora = itens.iter().filter(|o| o.ultimo_registro.is_none()).count();
            Ok(PaginaOnusDto {
                total,
                itens: itens.iter().map(onu_dto).collect(),
                aviso: (sem_hora > 0 && !itens.is_empty()).then(|| {
                    format!("{sem_hora} ONU(s) sem hora de registro informada pelo NE")
                }),
                modulo: "C".into(),
            })
        }
        outro => Err(format!("list_onus devolveu {}", outro.tipo())),
    }
}

// ---------- lógica (pura em relação ao Tauri — é o que os testes exercitam) ----------

pub fn fontes_de(app: &Aplicacao) -> Vec<FonteDto> {
    app.nes
        .iter()
        .map(|ne| FonteDto {
            id: ne.fonte.as_str().to_string(),
            rotulo: ne.rotulo.clone(),
            perfil: ne.perfil.clone(),
            modelo: ne.modelo.clone(),
            host: ne.transporte.fontes[0].host.clone(),
            protocolo: ne.transporte.fontes[0].protocolo.clone(),
            modulo: "C".into(),
        })
        .collect()
}

pub fn olts_de(app: &Aplicacao, fonte: &str) -> Result<Vec<OltDto>, String> {
    let ne = app.ne(fonte)?;
    // `list_olts` não pagina no contrato: devolve as OLTs da fonte (§1.3).
    match bloquear(ne.transporte.exec(CanonicalCmd::ListOlts { source: SourceId::novo(fonte) }))
    .map_err(|e| e.to_string())?
    {
        CanonicalResult::Olts(itens) => Ok(itens
            .iter()
            .map(|o| OltDto {
                id: o.id.as_str().to_string(),
                modelo: o.modelo.clone(),
                vendor: o.vendor.clone(),
                ip: o.ip.clone(),
                sw_version: o.sw_version.clone(),
                slots: o.slots,
                uptime_dias: o.uptime_s / 86_400,
                temperatura_c: o.temperatura_c,
            })
            .collect()),
        outro => Err(format!("list_olts devolveu {}", outro.tipo())),
    }
}

pub fn pons_de(app: &Aplicacao, fonte: &str, olt: &str) -> Result<Vec<PonDto>, String> {
    let ne = app.ne(fonte)?;
    match bloquear(ne.transporte.exec(CanonicalCmd::ListPonPorts { olt: OltId::novo(olt) }))
        .map_err(|e| e.to_string())?
    {
        CanonicalResult::PonPorts(itens) => Ok(itens
            .iter()
            .map(|p| PonDto {
                olt: p.olt.as_str().to_string(),
                slot: p.pon.slot,
                porta: p.pon.porta,
                rotulo: format!("{}/{}", p.pon.slot, p.pon.porta),
                tecnologia: p.tecnologia.as_str().to_string(),
                admin_up: p.admin_up,
                split_ratio: p.split_ratio,
                onus: p.onus,
            })
            .collect()),
        outro => Err(format!("list_pon_ports devolveu {}", outro.tipo())),
    }
}

pub fn onus_da_pon(
    app: &Aplicacao,
    fonte: &str,
    olt: &str,
    pon: Option<(u8, u8)>,
    filtro: Option<String>,
    ordem: OrdemOnu,
) -> Result<PaginaOnusDto, String> {
    onus_de(app.ne(fonte)?, olt, pon, filtro, 4096, ordem)
}

pub fn busca_de(
    app: &Aplicacao,
    fonte: &str,
    criterio: &str,
    valor: &str,
) -> Result<BuscaDto, String> {
    let ne = app.ne(fonte)?;
    let crit = match criterio {
        "alias" => CriterioBusca::Alias,
        "vlan" => CriterioBusca::Vlan,
        _ => CriterioBusca::Sn,
    };
    let cmd = CanonicalCmd::BuscarOnu {
        source: SourceId::novo(fonte),
        criterio: crit,
        valor: valor.to_string(),
    };

    match bloquear(ne.transporte.exec(cmd)) {
        Ok(CanonicalResult::Onu(o)) => Ok(BuscaDto {
            criterio: criterio.into(),
            valor: valor.into(),
            itens: vec![onu_dto(&o)],
            aviso: None,
            modulo: "C".into(),
        }),
        Ok(CanonicalResult::Onus { itens, .. }) => Ok(BuscaDto {
            criterio: criterio.into(),
            valor: valor.into(),
            aviso: Some(format!(
                "ALIAS não é único: {} ONUs atendem a este texto",
                itens.len()
            )),
            itens: itens.iter().map(onu_dto).collect(),
            modulo: "C".into(),
        }),
        Ok(outro) => Err(format!("busca devolveu {}", outro.tipo())),
        // "não achei" e "não existe nesta fase" são coisas diferentes para o operador.
        Err(e) => Err(e.to_string()),
    }
}

/// O caso de uso âncora (§7.4): ONUs da PON agrupadas por janela de queda.
pub fn massiva_de(
    app: &Aplicacao,
    fonte: &str,
    olt: &str,
    slot: u8,
    porta: u8,
    janela_min: Option<i64>,
) -> Result<MassivaDto, String> {
    let ne = app.ne(fonte)?;
    let janela = janela_min.unwrap_or(JANELA_PADRAO_MIN).clamp(1, 240);
    let cmd = CanonicalCmd::ListOnus {
        olt: OltId::novo(olt),
        pon: Some(PonRef::new(slot, porta)),
        filtro: None,
        segmento: SegmentoOnu::Todas,
        page: Page { offset: 0, limit: 4096 },
        ordenar_por: OrdemOnu::UltimoRegistro,
    };
    // As ONUs vêm do transporte (estado real inclusive); nada é reconstruído à mão.
    let onus = match bloquear(ne.transporte.exec(cmd)).map_err(|e| e.to_string())? {
        CanonicalResult::Onus { itens, .. } => itens,
        outro => return Err(format!("list_onus devolveu {}", outro.tipo())),
    };

    let clusters = agrupar_massiva(&onus, janela);
    let com_hora: usize = clusters.iter().map(|c| c.quedas.len()).sum();

    Ok(MassivaDto {
        janela_min: janela,
        com_hora,
        sem_hora: onus.len().saturating_sub(com_hora),
        maior_cluster: clusters.iter().map(|c| c.quedas.len()).max().unwrap_or(0),
        clusters: clusters
            .iter()
            .map(|c| ClusterDto {
                inicio: c.quedas.last().map(|q| q.quando_texto.clone()).unwrap_or_default(),
                fim: c.quedas.first().map(|q| q.quando_texto.clone()).unwrap_or_default(),
                tamanho: c.quedas.len(),
                duracao_min: c.duracao_s() / 60,
                quedas: c
                    .quedas
                    .iter()
                    .map(|q| QuedaDto {
                        sn: q.sn.clone(),
                        nome: q.nome.clone(),
                        quando: q.quando_texto.clone(),
                    })
                    .collect(),
            })
            .collect(),
        modulo: "C".into(),
    })
}

#[derive(Serialize)]
pub struct AlarmeDto {
    pub id: String,
    pub severidade: String,
    pub ne: String,
    pub objeto: String,
    pub tipo: String,
    pub primeira: String,
    pub ultima: String,
    pub ack: bool,
}

pub fn alarmes_de(
    app: &Aplicacao,
    fonte: &str,
    filtro: Option<String>,
) -> Result<Vec<AlarmeDto>, String> {
    let ne = app.ne(fonte)?;
    let cmd = CanonicalCmd::ListAlarms { source: SourceId::novo(fonte), filtro };
    match bloquear(ne.transporte.exec(cmd)).map_err(|e| e.to_string())? {
        CanonicalResult::Alarms(itens) => Ok(itens
            .iter()
            .map(|a| AlarmeDto {
                id: a.id.clone(),
                severidade: a.severidade.as_str().to_string(),
                ne: a.ne.as_str().to_string(),
                objeto: a.objeto.clone(),
                tipo: a.tipo.clone(),
                primeira: a.primeira.clone(),
                ultima: a.ultima.clone(),
                ack: a.ack,
            })
            .collect()),
        outro => Err(format!("list_alarms devolveu {}", outro.tipo())),
    }
}

// ---------- comandos Tauri (só tradução; nenhuma regra mora aqui) ----------

#[tauri::command]
pub fn fontes(app: tauri::State<'_, Aplicacao>) -> Vec<FonteDto> {
    fontes_de(&app)
}

#[tauri::command]
pub fn listar_olts(app: tauri::State<'_, Aplicacao>, fonte: String) -> Result<Vec<OltDto>, String> {
    olts_de(&*app, &fonte)
}

#[tauri::command]
pub fn listar_pons(
    app: tauri::State<'_, Aplicacao>,
    fonte: String,
    olt: String,
) -> Result<Vec<PonDto>, String> {
    pons_de(&*app, &fonte, &olt)
}

#[tauri::command]
pub fn listar_onus(
    app: tauri::State<'_, Aplicacao>,
    fonte: String,
    olt: String,
    slot: Option<u8>,
    porta: Option<u8>,
    filtro: Option<String>,
    ordem: Option<String>,
) -> Result<PaginaOnusDto, String> {
    let pon = match (slot, porta) {
        (Some(s), Some(p)) => Some((s, p)),
        _ => None,
    };
    let ordem = match ordem.as_deref() {
        Some("ultimo_registro") => OrdemOnu::UltimoRegistro,
        _ => OrdemOnu::Sn,
    };
    onus_da_pon(&*app, &fonte, &olt, pon, filtro, ordem)
}

#[tauri::command]
pub fn buscar_onu(
    app: tauri::State<'_, Aplicacao>,
    fonte: String,
    criterio: String,
    valor: String,
) -> Result<BuscaDto, String> {
    busca_de(&*app, &fonte, &criterio, &valor)
}

#[tauri::command]
pub fn massiva(
    app: tauri::State<'_, Aplicacao>,
    fonte: String,
    olt: String,
    slot: u8,
    porta: u8,
    janela_min: Option<i64>,
) -> Result<MassivaDto, String> {
    massiva_de(&*app, &fonte, &olt, slot, porta, janela_min)
}

#[tauri::command]
pub fn listar_alarmes(
    app: tauri::State<'_, Aplicacao>,
    fonte: String,
    filtro: Option<String>,
) -> Result<Vec<AlarmeDto>, String> {
    alarmes_de(&*app, &fonte, filtro)
}
