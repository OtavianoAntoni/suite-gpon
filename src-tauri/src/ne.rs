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
    CanonicalCmd, CanonicalResult, CriterioBusca, OltId, OnuId, OrdemOnu, Page, PonRef, SegmentoOnu,
    SourceId, Transport,
};
use mock_ne_huawei::{Fixture, ModeloNe};
use crate::cofre_sistema::CofreDoSistema;
use crate::guarda::{agora_utc, Auditoria, Guarda, Pedido, RegistroAuditoria, TipoEscrita};
use gpon_cofre::{Cofre, Segredo};
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
    /// Cofre do sistema (keyring). A UI só toca aqui por comando explícito — não há auto-seed.
    pub cofre: Box<dyn Cofre>,
    pub guarda: Guarda,
    pub auditoria: Auditoria,
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
        Self {
            nes,
            cofre: Box::new(CofreDoSistema::default()),
            guarda: Guarda::default(),
            auditoria: Auditoria::default(),
        }
    }

    /// O pedido de escrita carrega a OLT, não a fonte: quem sabe qual NE serve é o app.
    fn ne_por_olt(&self, olt: &str) -> Result<&Ne, String> {
        self.nes
            .iter()
            .find(|ne| ne.transporte.olts.iter().any(|o| o.id.as_str() == olt))
            .ok_or_else(|| format!("nenhuma fonte serve a OLT {olt}"))
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

#[derive(Serialize, Debug, Clone)]
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
    /// `None` = o NE não informou a hora da **queda** (não é "agora"!).
    pub ultima_queda: Option<String>,
    /// `None` = o NE não informou a hora do **retorno** (ou a ONU não voltou).
    pub ultimo_up: Option<String>,
    /// `None` = o NE não informou o motivo ("LOS", "DyingGasp").
    pub motivo_queda: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
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
    /// Id da ONU — é o que permite clicar na linha e abrir o cliente (§7.4).
    pub id: String,
    pub sn: String,
    pub nome: String,
    /// Hora da queda, como o NE mandou.
    pub quando: String,
    /// Hora do retorno; `None` = não voltou ou o NE não informa.
    pub ultimo_up: Option<String>,
    /// Motivo no vocabulário do NE.
    pub motivo: Option<String>,
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
        ultima_queda: o.ultima_queda.clone(),
        ultimo_up: o.ultimo_up.clone(),
        motivo_queda: o.motivo_queda.clone(),
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
            let sem_hora = itens.iter().filter(|o| o.ultima_queda.is_none()).count();
            Ok(PaginaOnusDto {
                total,
                itens: itens.iter().map(onu_dto).collect(),
                aviso: (sem_hora > 0 && !itens.is_empty()).then(|| {
                    format!("{sem_hora} ONU(s) sem hora de queda informada pelo NE")
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

/// `get_olt_info` — o detalhe que a lista não traz: serial, uptime cru e temperatura.
///
/// A contagem de PONs vem de `list_pon_ports` (operação canônica própria). Não inventamos campo
/// novo aqui para não criar um segundo caminho para o mesmo dado (§1.3: um caminho por operação).
#[derive(Serialize, Debug, Clone)]
pub struct OltDetalheDto {
    pub id: String,
    pub fonte: String,
    pub modelo: String,
    pub vendor: String,
    /// Serial do equipamento — o campo que a lista de OLTs não devolve.
    pub serial: String,
    pub ip: String,
    pub sw_version: String,
    pub slots: u16,
    pub uptime_dias: u64,
    pub uptime_horas: u64,
    /// `None` = o NE não informou temperatura (não é zero!).
    pub temperatura_c: Option<i16>,
    /// `None` = a contagem de PONs não pôde ser lida agora (a tela diz isso, não mostra "0").
    pub pons: Option<usize>,
}

pub fn detalhe_olt_de(app: &Aplicacao, fonte: &str, olt: &str) -> Result<OltDetalheDto, String> {
    let ne = app.ne(fonte)?;
    // Pedir detalhe de uma OLT que esta fonte não serve é erro de digitação, não "dado vazio":
    // falhar alto aqui evita a tela mostrar uma OLT em branco como se o NE não soubesse responder.
    if !ne.transporte.olts.iter().any(|o| o.id.as_str() == olt) {
        return Err(format!("a fonte {fonte} não serve a OLT {olt}"));
    }

    let detalhe = match bloquear(ne.transporte.exec(CanonicalCmd::GetOltInfo { olt: OltId::novo(olt) }))
        .map_err(|e| e.to_string())?
    {
        CanonicalResult::Olt(o) => OltDetalheDto {
            id: o.id.as_str().to_string(),
            fonte: o.fonte.as_str().to_string(),
            modelo: o.modelo.clone(),
            vendor: o.vendor.clone(),
            serial: o.serial.clone(),
            ip: o.ip.clone(),
            sw_version: o.sw_version.clone(),
            slots: o.slots,
            uptime_dias: o.uptime_s / 86_400,
            uptime_horas: o.uptime_s / 3_600,
            temperatura_c: o.temperatura_c,
            pons: None,
        },
        outro => return Err(format!("get_olt_info devolveu {}", outro.tipo())),
    };

    let pons = match bloquear(ne.transporte.exec(CanonicalCmd::ListPonPorts { olt: OltId::novo(olt) })) {
        Ok(CanonicalResult::PonPorts(itens)) => Some(itens.len()),
        _ => None,
    };

    Ok(OltDetalheDto { pons, ..detalhe })
}

/// `get_onu` — uma ONU pelo id, sem depender da página que a tela tinha em mãos.
///
/// Existe porque a tabela de quedas mostra SN/ALIAS de ONUs que podem estar fora da página
/// carregada: buscar por id é o caminho canônico (§1.3), não um truque de UI.
pub fn onu_de(app: &Aplicacao, fonte: &str, onu: &str) -> Result<OnuDto, String> {
    let ne = app.ne(fonte)?;
    match bloquear(ne.transporte.exec(CanonicalCmd::GetOnu { onu: OnuId::novo(onu) }))
        .map_err(|e| e.to_string())?
    {
        CanonicalResult::Onu(o) => Ok(onu_dto(&o)),
        outro => Err(format!("get_onu devolveu {}", outro.tipo())),
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
        ordenar_por: OrdemOnu::UltimaQueda,
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
                        id: q.id.clone(),
                        sn: q.sn.clone(),
                        nome: q.nome.clone(),
                        quando: q.quando_texto.clone(),
                        ultimo_up: q.ultimo_up.clone(),
                        motivo: q.motivo.clone(),
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

// ---------- escrita: prévia → confirmação → auditoria (SPEC §9) ----------

#[derive(serde::Deserialize)]
pub struct PedidoEntrada {
    pub tipo: String,
    pub olt: String,
    pub slot: u8,
    pub porta: u8,
    #[serde(default)]
    pub onu: String,
    #[serde(default)]
    pub sn: String,
    #[serde(default)]
    pub nome: String,
    #[serde(default)]
    pub atual: String,
    #[serde(default)]
    pub novo: String,
    #[serde(default)]
    pub modo: Option<String>,
    #[serde(default)]
    pub perfil: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct PreviaDto {
    pub token: String,
    pub token_curto: String,
    pub validade_s: u64,
    pub operacao: String,
    pub alvo: String,
    /// Texto que o humano lê: "reiniciar a ONU X — cliente Y (onu-1) na PON 0/1".
    pub resumo: String,
}

#[derive(Serialize, Debug, Clone)]
pub struct ConfirmacaoDto {
    pub ok: bool,
    pub mensagem: String,
    pub id_operacao: Option<String>,
    pub operacao: String,
    /// Aviso quando a auditoria não pôde ser gravada em arquivo (a operação NÃO é desfeita).
    pub aviso_auditoria: Option<String>,
}

#[derive(Serialize)]
pub struct AuditoriaDto {
    pub quando: String,
    pub operacao: String,
    pub alvo: String,
    pub resumo: String,
    pub resultado: String,
    pub mensagem: String,
}

#[derive(Serialize)]
pub struct CofreDto {
    pub referencia: String,
    pub existe: bool,
    pub aviso: Option<String>,
}

fn pedido_de(e: PedidoEntrada) -> Result<Pedido, String> {
    match TipoEscrita::de_str(&e.tipo).ok_or_else(|| format!("tipo de escrita desconhecido: {}", e.tipo))? {
        TipoEscrita::Reset => Ok(Pedido::reset(&e.olt, e.slot, e.porta, &e.onu, &e.sn, &e.nome)),
        TipoEscrita::Descricao => Ok(Pedido::descricao(
            &e.olt, e.slot, e.porta, &e.onu, &e.sn, &e.nome, &e.atual, &e.novo,
        )),
        TipoEscrita::Autorizar => Ok(Pedido::autorizar(
            &e.olt,
            e.slot,
            e.porta,
            &e.sn,
            e.modo.as_deref().unwrap_or("sn"),
            e.perfil.as_deref(),
        )),
        TipoEscrita::Desautorizar => {
            Ok(Pedido::desautorizar(&e.olt, e.slot, e.porta, &e.onu, &e.sn, &e.nome))
        }
    }
}

/// Passo 1: mostra o que vai acontecer e emite o token. **Nada é executado.**
pub fn previa_escrita_de(app: &Aplicacao, entrada: PedidoEntrada) -> Result<PreviaDto, String> {
    let pedido = pedido_de(entrada)?;
    // Recusar aqui (ONU inexistente) evita emissão de token para alvo fantasma.
    let ne = app.ne_por_olt(&pedido.olt)?;
    if !pedido.onu.is_empty() && !ne.transporte.onus.iter().any(|o| o.id.as_str() == pedido.onu) {
        return Err(format!("ONU inexistente nesta fonte: {}", pedido.onu));
    }
    let token = app.guarda.previa(pedido.clone()).map_err(|e| e.to_string())?;
    Ok(PreviaDto {
        token: token.chave.clone(),
        token_curto: token.curto(),
        validade_s: token.validade_s,
        operacao: pedido.tipo.operacao().to_string(),
        alvo: pedido.onu.clone(),
        resumo: pedido.resumo(),
    })
}

/// Passo 2: executa **exatamente o que foi mostrado** e audita (sucesso e falha).
pub fn confirmar_escrita_de(app: &Aplicacao, token: &str) -> Result<ConfirmacaoDto, String> {
    let pedido = app.guarda.confirmar(token).map_err(|e| {
        app.auditoria.registrar(RegistroAuditoria {
            quando: agora_utc(),
            operacao: "confirmacao_recusada".into(),
            alvo: token.chars().take(8).collect(),
            resumo: "token inválido, expirado ou já usado".into(),
            resultado: "falha".into(),
            mensagem: e.to_string(),
        });
        e.to_string()
    })?;

    let ne = match app.ne_por_olt(&pedido.olt) {
        Ok(ne) => ne,
        Err(e) => {
            app.auditoria.registrar(RegistroAuditoria {
                quando: agora_utc(),
                operacao: pedido.tipo.operacao().into(),
                alvo: pedido.onu.clone(),
                resumo: pedido.resumo(),
                resultado: "falha".into(),
                mensagem: e.clone(),
            });
            return Err(e);
        }
    };

    let resultado = bloquear(ne.transporte.exec(pedido.comando()));

    match resultado {
        Ok(CanonicalResult::Ack(ack)) => {
            let aviso = app.auditoria.registrar(RegistroAuditoria {
                quando: agora_utc(),
                operacao: pedido.tipo.operacao().into(),
                alvo: if pedido.onu.is_empty() { pedido.sn.clone() } else { pedido.onu.clone() },
                resumo: pedido.resumo(),
                resultado: "ok".into(),
                mensagem: ack.mensagem.clone(),
            });
            Ok(ConfirmacaoDto {
                ok: ack.ok,
                mensagem: ack.mensagem,
                id_operacao: ack.id_operacao,
                operacao: pedido.tipo.operacao().to_string(),
                aviso_auditoria: aviso,
            })
        }
        Ok(outro) => {
            let msg = format!("resposta inesperada do NE: {}", outro.tipo());
            app.auditoria.registrar(RegistroAuditoria {
                quando: agora_utc(),
                operacao: pedido.tipo.operacao().into(),
                alvo: pedido.onu.clone(),
                resumo: pedido.resumo(),
                resultado: "falha".into(),
                mensagem: msg.clone(),
            });
            Err(msg)
        }
        Err(e) => {
            app.auditoria.registrar(RegistroAuditoria {
                quando: agora_utc(),
                operacao: pedido.tipo.operacao().into(),
                alvo: pedido.onu.clone(),
                resumo: pedido.resumo(),
                resultado: "falha".into(),
                mensagem: e.to_string(),
            });
            Err(e.to_string())
        }
    }
}

pub fn auditoria_recente_de(app: &Aplicacao, n: usize) -> Vec<AuditoriaDto> {
    app.auditoria
        .ultimos(n.clamp(1, 200))
        .into_iter()
        .map(|r| AuditoriaDto {
            quando: r.quando,
            operacao: r.operacao,
            alvo: r.alvo,
            resumo: r.resumo,
            resultado: r.resultado,
            mensagem: r.mensagem,
        })
        .collect()
}

/// Estado do cofre por fonte: existe credencial? **Não** cria nada ao abrir o app.
pub fn cofre_estado_de(app: &Aplicacao) -> Vec<CofreDto> {
    app.nes
        .iter()
        .map(|ne| {
            let referencia = ne.transporte.fontes[0].credencial_ref.clone();
            let existe = app.cofre.existe(&referencia);
            let aviso = match app.cofre.ler(&referencia) {
                Err(gpon_cofre::ErroCofre::FalhaDoSistema { motivo, .. }) => Some(motivo),
                _ => None,
            };
            CofreDto { referencia, existe, aviso }
        })
        .collect()
}

/// Gravação explícita (a UI chama só por ação do operador). Nunca em arquivo, nunca em log.
pub fn cofre_gravar_de(app: &Aplicacao, referencia: &str, segredo: &str) -> Result<(), String> {
    app.cofre
        .guardar(referencia, &Segredo::novo(segredo))
        .map_err(|e| e.to_string())
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
pub fn detalhe_onu(
    app: tauri::State<'_, Aplicacao>,
    fonte: String,
    onu: String,
) -> Result<OnuDto, String> {
    onu_de(&*app, &fonte, &onu)
}

#[tauri::command]
pub fn detalhe_olt(
    app: tauri::State<'_, Aplicacao>,
    fonte: String,
    olt: String,
) -> Result<OltDetalheDto, String> {
    detalhe_olt_de(&*app, &fonte, &olt)
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
        Some("ultima_queda") => OrdemOnu::UltimaQueda,
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

#[tauri::command]
pub fn previa_escrita(
    app: tauri::State<'_, Aplicacao>,
    pedido: PedidoEntrada,
) -> Result<PreviaDto, String> {
    previa_escrita_de(&*app, pedido)
}

#[tauri::command]
pub fn confirmar_escrita(
    app: tauri::State<'_, Aplicacao>,
    token: String,
) -> Result<ConfirmacaoDto, String> {
    confirmar_escrita_de(&*app, &token)
}

#[tauri::command]
pub fn auditoria_recente(app: tauri::State<'_, Aplicacao>, n: Option<usize>) -> Vec<AuditoriaDto> {
    auditoria_recente_de(&*app, n.unwrap_or(50))
}

#[tauri::command]
pub fn cofre_estado(app: tauri::State<'_, Aplicacao>) -> Vec<CofreDto> {
    cofre_estado_de(&*app)
}

#[tauri::command]
pub fn cofre_gravar(
    app: tauri::State<'_, Aplicacao>,
    referencia: String,
    segredo: String,
) -> Result<(), String> {
    cofre_gravar_de(&*app, &referencia, &segredo)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn app() -> Aplicacao {
        Aplicacao::carregar()
    }

    #[test]
    fn detalhe_da_olt_traz_o_que_a_lista_nao_traz() {
        let app = app();
        let fonte = app.nes[0].fonte.as_str().to_string();
        let olt = app.nes[0].transporte.olts[0].id.as_str().to_string();

        let lista = olts_de(&app, &fonte).expect("listar OLTs");
        let detalhe = detalhe_olt_de(&app, &fonte, &olt).expect("detalhe da OLT");

        // O serial é o motivo de `get_olt_info` existir: ele NÃO está no DTO da lista.
        assert!(!detalhe.serial.is_empty(), "serial vazio na fixture do piloto");
        assert_eq!(detalhe.id, olt);
        assert_eq!(detalhe.fonte, fonte);
        assert_eq!(detalhe.modelo, lista[0].modelo);
        assert_eq!(detalhe.ip, lista[0].ip);
        // Uptime em dias e em horas vêm do MESMO segundo do NE: não podem divergir.
        assert_eq!(detalhe.uptime_dias * 24, detalhe.uptime_horas);
        // A contagem de PONs é de `list_pon_ports`, não campo inventado no detalhe.
        assert_eq!(detalhe.pons, Some(pons_de(&app, &fonte, &olt).expect("listar PONs").len()));
    }

    #[test]
    fn detalhe_de_olt_de_outra_fonte_falha_alto() {
        let app = app();
        let fonte_a = app.nes[0].fonte.as_str().to_string();
        let olt_b = app.nes[1].transporte.olts[0].id.as_str().to_string();

        let erro = detalhe_olt_de(&app, &fonte_a, &olt_b).expect_err("deveria recusar");
        assert!(
            erro.contains("não serve a OLT"),
            "mensagem precisa dizer que a fonte não serve a OLT: {erro}"
        );
    }

    #[test]
    fn detalhe_com_fonte_desconhecida_falha_alto() {
        let app = app();
        let olt = app.nes[0].transporte.olts[0].id.as_str().to_string();

        let erro = detalhe_olt_de(&app, "fonte-que-nao-existe", &olt).expect_err("deveria recusar");
        assert!(erro.contains("fonte desconhecida"), "mensagem: {erro}");
    }

    #[test]
    fn as_duas_familias_respondem_o_detalhe() {
        // O mock tem dois perfis (MA5800 e MA5600T): o detalhe não pode funcionar só num deles.
        let app = app();
        assert_eq!(app.nes.len(), 2, "esperava dois perfis de fixture");

        let mut modelos = Vec::new();
        for ne in &app.nes {
            let fonte = ne.fonte.as_str().to_string();
            let olt = ne.transporte.olts[0].id.as_str().to_string();
            let d = detalhe_olt_de(&app, &fonte, &olt).expect("detalhe em cada perfil");
            modelos.push(d.modelo.clone());
        }
        modelos.sort();
        modelos.dedup();
        assert_eq!(modelos.len(), 2, "os dois perfis devolveram o mesmo modelo: {modelos:?}");
    }
}
