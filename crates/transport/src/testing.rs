//! Transporte **em memória** — o "NE" das Fases 0 e 1 (SPEC §0.2: 100% mock).
//!
//! Não abre porta, não usa rede, não depende de tempo: responde a partir de vetores em memória.
//! Serve para (a) provar o contrato e (b) alimentar a UI enquanto a Fase 1 não tem fixtures reais.
//!
//! O que ele **não** faz: simular latência, erro de firmware ou concorrência de rede. Isso é
//! trabalho do `mock-ne-huawei` com fixtures (passo 3b) e, na rede de verdade, da Fase 2.

use crate::cmd::{
    CanonicalCmd, OltId, OnuId, Page, PonRef, ProfileKind, SegmentoOnu, SourceId,
};
use crate::error::TransportError;
use crate::result::{
    Alarm, CanonicalResult, ConnectivityReport, InventoryExport, Olt, Onu, OnuEstado, PonPort,
    Profile, Source, Tecnologia, WriteAck,
};
use crate::{BoxFuture, Module, Transport};
use std::sync::Mutex;

/// Transporte de teste/mock: dados em vetores, resposta determinística e auditoria das chamadas.
#[derive(Debug)]
pub struct InMemoryTransport {
    /// Módulo que este mock representa (a badge "via {módulo}" sai daqui).
    module: Module,
    pub fontes: Vec<Source>,
    pub olts: Vec<Olt>,
    pub pons: Vec<PonPort>,
    pub onus: Vec<Onu>,
    pub perfis: Vec<Profile>,
    pub alarmes: Vec<Alarm>,
    /// Quando `Some`, todo `exec` devolve este erro — usado para testar a UI de falha.
    pub derrubar: Option<TransportError>,
    /// Registro do caminho canônico executado (id por chamada). Prova que não há atalho paralelo.
    chamadas: Mutex<Vec<&'static str>>,
}

impl InMemoryTransport {
    pub fn new(module: Module) -> Self {
        Self {
            module,
            fontes: Vec::new(),
            olts: Vec::new(),
            pons: Vec::new(),
            onus: Vec::new(),
            perfis: Vec::new(),
            alarmes: Vec::new(),
            derrubar: None,
            chamadas: Mutex::new(Vec::new()),
        }
    }

    /// Amostra mínima com os **dois perfis** de fixture decididos em P4 (MA5800 e MA5600T).
    ///
    /// Serve para teste e para a UI ter algo coerente na Fase 0. Não é o fixture rico do passo 3b:
    /// aqui o objetivo é exercitar o contrato, não o dicionário MML.
    pub fn amostra_dois_perfis() -> Self {
        let mut t = Self::new(Module::C);
        let fonte = SourceId::novo("lab-1");
        t.fontes.push(Source {
            id: fonte.clone(),
            rotulo: "Laboratório (mock)".into(),
            host: "10.0.0.10".into(),
            protocolo: "ssh+mml".into(),
            credencial_ref: "cofre://lab-1".into(),
        });

        t.olts.push(Olt {
            id: OltId::novo("olt-ma5800-1"),
            fonte: fonte.clone(),
            vendor: "Huawei".into(),
            modelo: "MA5800-X7".into(),
            serial: "48575443A1B2C3D4".into(),
            ip: "10.0.0.11".into(),
            sw_version: "V100R021C10".into(),
            uptime_s: 1_296_000,
            slots: 17,
            temperatura_c: Some(41),
        });
        t.olts.push(Olt {
            id: OltId::novo("olt-ma5600t-1"),
            fonte,
            vendor: "Huawei".into(),
            modelo: "MA5600T".into(),
            serial: "48575443E5F6A7B8".into(),
            ip: "10.0.0.12".into(),
            sw_version: "V800R018C10".into(),
            uptime_s: 8_640_000,
            slots: 16,
            temperatura_c: Some(44),
        });

        let olt_a = OltId::novo("olt-ma5800-1");
        let olt_b = OltId::novo("olt-ma5600t-1");
        t.pons.push(PonPort {
            olt: olt_a.clone(),
            pon: PonRef::new(0, 1),
            tecnologia: Tecnologia::Gpon,
            admin_up: true,
            split_ratio: 128,
            onus: 2,
        });
        t.pons.push(PonPort {
            olt: olt_b.clone(),
            pon: PonRef::new(0, 1),
            tecnologia: Tecnologia::Gpon,
            admin_up: true,
            split_ratio: 64,
            onus: 1,
        });

        t.onus.push(Onu {
            id: OnuId::novo("onu-1"),
            olt: olt_a.clone(),
            pon: PonRef::new(0, 1),
            sn: "48575443A1B2C3D4".into(),
            mac: Some("00:1a:2b:3c:4d:5e".into()),
            modelo_hw: "HG8245H".into(),
            modelo_sw: "V5R019C10S270".into(),
            vendor: "Huawei".into(),
            nome: "cliente-0001".into(),
            descricao: "portaria".into(),
            estado: OnuEstado::Online,
            distancia_m: Some(1240),
            ultimo_registro: Some("2026-10-16T21:40:00-03:00".into()),
        });
        t.onus.push(Onu {
            id: OnuId::novo("onu-2"),
            olt: olt_a,
            pon: PonRef::new(0, 1),
            sn: "48575443A1B2C3D5".into(),
            mac: None,
            modelo_hw: "HG8010H".into(),
            modelo_sw: "V5R019C10S270".into(),
            vendor: "Huawei".into(),
            nome: "cliente-0002".into(),
            descricao: String::new(),
            estado: OnuEstado::Los,
            distancia_m: None,
            ultimo_registro: None,
        });
        t.onus.push(Onu {
            id: OnuId::novo("onu-3"),
            olt: olt_b,
            pon: PonRef::new(0, 1),
            sn: "48575443E5F6A7B9".into(),
            mac: None,
            modelo_hw: "HG8310M".into(),
            modelo_sw: "V5R019C10S270".into(),
            vendor: "Huawei".into(),
            nome: "cliente-0003".into(),
            descricao: String::new(),
            estado: OnuEstado::Offline,
            distancia_m: Some(2310),
            ultimo_registro: Some("2026-10-15T09:12:00-03:00".into()),
        });

        t.perfis.push(Profile { nome: "line-1G".into(), kind: ProfileKind::Line, versao: "1".into() });
        t.perfis.push(Profile { nome: "dba-1G".into(), kind: ProfileKind::Dba, versao: "1".into() });
        t.alarmes.push(Alarm {
            id: "ALM-1".into(),
            severidade: crate::result::Severidade::Maior,
            ne: OltId::novo("olt-ma5800-1"),
            objeto: "0/1/0".into(),
            tipo: "LOS".into(),
            primeira: "2026-10-16T20:00:00-03:00".into(),
            ultima: "2026-10-16T21:59:00-03:00".into(),
            ack: false,
        });
        t
    }

    /// Ids das operações executadas, na ordem — a trilha de auditoria do caminho canônico.
    pub fn chamadas(&self) -> Vec<&'static str> {
        self.chamadas.lock().map(|c| c.clone()).unwrap_or_default()
    }

    fn registrar(&self, id: &'static str) {
        if let Ok(mut c) = self.chamadas.lock() {
            c.push(id);
        }
    }

    fn olt(&self, id: &OltId) -> Result<Olt, TransportError> {
        self.olts
            .iter()
            .find(|o| &o.id == id)
            .cloned()
            .ok_or_else(|| TransportError::NaoEncontrado {
                o_que: "OLT".into(),
                chave: id.to_string(),
            })
    }

    fn onu(&self, id: &OnuId) -> Result<Onu, TransportError> {
        self.onus
            .iter()
            .find(|o| &o.id == id)
            .cloned()
            .ok_or_else(|| TransportError::NaoEncontrado {
                o_que: "ONU".into(),
                chave: id.to_string(),
            })
    }

    fn responder(&self, cmd: CanonicalCmd) -> Result<CanonicalResult, TransportError> {
        if let Some(e) = &self.derrubar {
            return Err(e.clone());
        }
        if !self.suportado(cmd.id()) {
            return Err(TransportError::NaoSuportado {
                modulo: self.module.tag().to_string(),
                operacao: cmd.id(),
            });
        }
        match cmd {
            CanonicalCmd::ListSources => Ok(CanonicalResult::Sources(self.fontes.clone())),

            CanonicalCmd::TestConnectivity { source } => {
                let existe = self.fontes.iter().any(|f| f.id == source);
                Ok(CanonicalResult::Connectivity(ConnectivityReport {
                    alcancou_servidor: existe,
                    alcancou_ne: if existe { Some(true) } else { None },
                    latencia_ms: if existe { Some(3) } else { None },
                    mensagem: if existe {
                        format!("fonte {source} respondeu (mock em memória)")
                    } else {
                        format!("fonte {source} não cadastrada")
                    },
                }))
            }

            CanonicalCmd::ListOlts { source } => {
                if !self.fontes.iter().any(|f| f.id == source) {
                    return Err(TransportError::NaoEncontrado {
                        o_que: "fonte".into(),
                        chave: source.to_string(),
                    });
                }
                let mut olts: Vec<Olt> =
                    self.olts.iter().filter(|o| o.fonte == source).cloned().collect();
                olts.sort_by(|a, b| a.id.cmp(&b.id));
                Ok(CanonicalResult::Olts(olts))
            }

            CanonicalCmd::GetOltInfo { olt } => Ok(CanonicalResult::Olt(self.olt(&olt)?)),

            CanonicalCmd::ListPonPorts { olt } => {
                let _ = self.olt(&olt)?;
                let mut pons: Vec<PonPort> =
                    self.pons.iter().filter(|p| p.olt == olt).cloned().collect();
                pons.sort_by_key(|p| (p.pon.slot, p.pon.porta));
                Ok(CanonicalResult::PonPorts(pons))
            }

            CanonicalCmd::ListOnus { olt, pon, filtro, segmento, page } => {
                let _ = self.olt(&olt)?;
                let mut itens: Vec<Onu> = self
                    .onus
                    .iter()
                    .filter(|o| o.olt == olt)
                    .filter(|o| pon.map_or(true, |p| o.pon == p))
                    .filter(|o| match segmento {
                        SegmentoOnu::Todas => true,
                        SegmentoOnu::Online => o.estado == OnuEstado::Online,
                        SegmentoOnu::Offline => o.estado == OnuEstado::Offline,
                        SegmentoOnu::Los => o.estado == OnuEstado::Los,
                    })
                    .filter(|o| match &filtro {
                        None => true,
                        Some(f) => {
                            let f = f.to_lowercase();
                            o.sn.to_lowercase().contains(&f)
                                || o.nome.to_lowercase().contains(&f)
                                || o.descricao.to_lowercase().contains(&f)
                        }
                    })
                    .cloned()
                    .collect();
                itens.sort_by(|a, b| a.sn.cmp(&b.sn));
                let total = itens.len();
                let janela: Vec<Onu> = paginar(itens, page);
                Ok(CanonicalResult::Onus { itens: janela, total })
            }

            CanonicalCmd::FindOnuBySn { source, sn } => {
                if !self.fontes.iter().any(|f| f.id == source) {
                    return Err(TransportError::NaoEncontrado {
                        o_que: "fonte".into(),
                        chave: source.to_string(),
                    });
                }
                let achada = self.onus.iter().find(|o| o.sn.eq_ignore_ascii_case(&sn)).cloned();
                match achada {
                    Some(onu) => Ok(CanonicalResult::Onu(onu)),
                    None => Err(TransportError::NaoEncontrado {
                        o_que: "ONU com SN".into(),
                        chave: sn,
                    }),
                }
            }

            CanonicalCmd::GetOnu { onu } => Ok(CanonicalResult::Onu(self.onu(&onu)?)),

            CanonicalCmd::GetOnuAuthMode { onu } => {
                let onu = self.onu(&onu)?;
                // Mock: se tem SN, o modo é SN (o modo real vem do NE na Fase 2).
                let modo = if onu.sn.is_empty() {
                    crate::cmd::AuthMode::None
                } else {
                    crate::cmd::AuthMode::Sn
                };
                Ok(CanonicalResult::AuthMode(modo))
            }

            CanonicalCmd::ListProfiles { source, kind } => {
                if !self.fontes.iter().any(|f| f.id == source) {
                    return Err(TransportError::NaoEncontrado {
                        o_que: "fonte".into(),
                        chave: source.to_string(),
                    });
                }
                let perfis: Vec<Profile> =
                    self.perfis.iter().filter(|p| p.kind == kind).cloned().collect();
                Ok(CanonicalResult::Profiles(perfis))
            }

            CanonicalCmd::ListAlarms { source, filtro } => {
                if !self.fontes.iter().any(|f| f.id == source) {
                    return Err(TransportError::NaoEncontrado {
                        o_que: "fonte".into(),
                        chave: source.to_string(),
                    });
                }
                let mut alarmes: Vec<Alarm> = self
                    .alarmes
                    .iter()
                    .filter(|a| match &filtro {
                        None => true,
                        Some(f) => {
                            let f = f.to_lowercase();
                            a.tipo.to_lowercase().contains(&f) || a.objeto.contains(&f)
                        }
                    })
                    .cloned()
                    .collect();
                alarmes.sort_by(|a, b| a.severidade.cmp(&b.severidade));
                Ok(CanonicalResult::Alarms(alarmes))
            }

            CanonicalCmd::ExportInventory { filtro: _ } => Ok(CanonicalResult::Export(
                InventoryExport {
                    caminho: "(mock) inventario.csv".into(),
                    linhas: self.onus.len(),
                    // Hash do conteúdo do mock, não de um arquivo real.
                    sha256: "(mock) 0000000000000000".into(),
                },
            )),

            CanonicalCmd::AuthorizeOnu { olt, pon, sn, modo, perfil } => {
                let _ = self.olt(&olt)?;
                if self.onus.iter().any(|o| o.sn.eq_ignore_ascii_case(&sn)) {
                    return Err(TransportError::FalhaDoNe {
                        mensagem: format!("SN {sn} já autorizado"),
                    });
                }
                Ok(CanonicalResult::Ack(WriteAck {
                    ok: true,
                    mensagem: format!(
                        "(mock) autorização de {sn} em {}/{pon} com modo {} e perfil {}",
                        olt,
                        modo.as_str(),
                        perfil.unwrap_or_else(|| "-".into())
                    ),
                    id_operacao: None,
                }))
            }

            CanonicalCmd::DeauthorizeOnu { onu } => {
                let a = self.onu(&onu)?;
                Ok(CanonicalResult::Ack(WriteAck {
                    ok: true,
                    mensagem: format!("(mock) {onu} ({}) removida", a.sn),
                    id_operacao: None,
                }))
            }

            CanonicalCmd::ResetOnu { onu } => {
                let a = self.onu(&onu)?;
                Ok(CanonicalResult::Ack(WriteAck {
                    ok: true,
                    mensagem: format!("(mock) reset de {onu} ({})", a.sn),
                    id_operacao: Some("job-reset-1".into()),
                }))
            }

            CanonicalCmd::SetOnuDescription { onu, texto } => {
                let a = self.onu(&onu)?;
                Ok(CanonicalResult::Ack(WriteAck {
                    ok: true,
                    mensagem: format!("(mock) descrição de {onu} ({}) = {texto:?}", a.sn),
                    id_operacao: None,
                }))
            }
        }
    }

    /// Consulta a evidência do próprio domínio (§1.3): se o módulo não serve a operação, o
    /// transporte recusa em vez de inventar resposta.
    fn suportado(&self, id: &str) -> bool {
        gpon_domain::ops::by_id(id)
            .map(|o| o.evidence.of(self.module).is_usable())
            .unwrap_or(false)
    }
}

fn paginar<T: Clone>(mut itens: Vec<T>, page: Page) -> Vec<T> {
    if page.limit == 0 {
        return Vec::new();
    }
    if page.offset >= itens.len() {
        return Vec::new();
    }
    let fim = (page.offset + page.limit).min(itens.len());
    itens.drain(..page.offset);
    itens.truncate(fim - page.offset);
    itens
}

impl Transport for InMemoryTransport {
    fn module(&self) -> Module {
        self.module
    }

    fn exec(&self, cmd: CanonicalCmd) -> BoxFuture<'_, Result<CanonicalResult, TransportError>> {
        self.registrar(cmd.id());
        let r = self.responder(cmd);
        Box::pin(async move { r })
    }
}

/// Executor mínimo para as futures do mock (que resolvem na primeira `poll`).
///
/// Existe para o crate continuar **sem dependências**: nada de tokio/futures só para rodar teste.
/// A camada de rede real (Fase 2) usa tokio; o contrato não muda.
pub fn bloquear<F: std::future::Future>(fut: F) -> F::Output {
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    fn nada(_: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, nada, nada, nada);

    let waker = unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) };
    let mut cx = Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}
