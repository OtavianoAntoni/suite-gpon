//! Trava das operações destrutivas (SPEC §9): **nada é executado sem prévia + confirmação**.
//!
//! O desenho resolve três riscos reais de campo:
//! 1. **Clique acidental** — a prévia mostra o que vai acontecer (ONU, SN, "de → para") e devolve um
//!    token; sem token válido o comando não roda.
//! 2. **Troca de operação no meio** — o token carrega *o pedido*, não apenas "autorizado". O que é
//!    executado é exatamente o que foi mostrado, mesmo que a tela mude.
//! 3. **Replay** — o token é de uso único e expira (120 s por padrão).
//!
//! Tudo o que é tentado — inclusive o que falha — vai para a auditoria, com autor e hora.

use gpon_transport::{AuthMode, CanonicalCmd, OltId, OnuId, PonRef};
use std::collections::BTreeMap;
use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Quanto tempo a prévia vale. Curto de propósito: prévia velha é prévia esquecida.
pub const VALIDADE_PADRAO: Duration = Duration::from_secs(120);

/// Limite de caracteres da descrição. O limite do NE em si só se confirma na Fase 2; até lá a suíte
/// recusa antes de mandar, em vez de descobrir no equipamento.
pub const MAX_DESCRICAO: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoEscrita {
    Reset,
    Descricao,
    Autorizar,
    Desautorizar,
}

impl TipoEscrita {
    pub const fn operacao(self) -> &'static str {
        match self {
            Self::Reset => "reset_onu",
            Self::Descricao => "set_onu_description",
            Self::Autorizar => "authorize_onu",
            Self::Desautorizar => "deauthorize_onu",
        }
    }

    pub fn de_str(s: &str) -> Option<Self> {
        match s {
            "reset" => Some(Self::Reset),
            "descricao" => Some(Self::Descricao),
            "autorizar" => Some(Self::Autorizar),
            "desautorizar" => Some(Self::Desautorizar),
            _ => None,
        }
    }
}

/// O que foi pedido — guardado no token, para que se execute **isto** e não outra coisa.
#[derive(Debug, Clone)]
pub struct Pedido {
    pub tipo: TipoEscrita,
    pub olt: String,
    pub rack: u8,
    pub subrack: u8,
    pub slot: u8,
    pub porta: u8,
    pub onu: String,
    pub sn: String,
    pub nome: String,
    /// Só em `Descricao`: texto novo e o atual (para a prévia e a auditoria mostrarem o diff).
    pub texto: Option<String>,
    pub texto_anterior: Option<String>,
    /// Só em `Autorizar`.
    pub modo: Option<String>,
    pub perfil: Option<String>,
}

impl Pedido {
    pub fn reset(olt: &str, rack: u8, subrack: u8, slot: u8, porta: u8, onu: &str, sn: &str, nome: &str) -> Self {
        Self {
            tipo: TipoEscrita::Reset,
            olt: olt.into(),
            rack,
            subrack,
            slot,
            porta,
            onu: onu.into(),
            sn: sn.into(),
            nome: nome.into(),
            texto: None,
            texto_anterior: None,
            modo: None,
            perfil: None,
        }
    }

    pub fn descricao(
        olt: &str,
        rack: u8,
        subrack: u8,
        slot: u8,
        porta: u8,
        onu: &str,
        sn: &str,
        nome: &str,
        atual: &str,
        novo: &str,
    ) -> Self {
        Self {
            tipo: TipoEscrita::Descricao,
            olt: olt.into(),
            rack,
            subrack,
            slot,
            porta,
            onu: onu.into(),
            sn: sn.into(),
            nome: nome.into(),
            texto: Some(novo.to_string()),
            texto_anterior: Some(atual.to_string()),
            modo: None,
            perfil: None,
        }
    }

    pub fn autorizar(olt: &str, rack: u8, subrack: u8, slot: u8, porta: u8, sn: &str, modo: &str, perfil: Option<&str>) -> Self {
        Self {
            tipo: TipoEscrita::Autorizar,
            olt: olt.into(),
            rack,
            subrack,
            slot,
            porta,
            onu: String::new(),
            sn: sn.into(),
            nome: String::new(),
            texto: None,
            texto_anterior: None,
            modo: Some(modo.to_string()),
            perfil: perfil.map(str::to_string),
        }
    }

    pub fn desautorizar(olt: &str, rack: u8, subrack: u8, slot: u8, porta: u8, onu: &str, sn: &str, nome: &str) -> Self {
        Self {
            tipo: TipoEscrita::Desautorizar,
            olt: olt.into(),
            rack,
            subrack,
            slot,
            porta,
            onu: onu.into(),
            sn: sn.into(),
            nome: nome.into(),
            texto: None,
            texto_anterior: None,
            modo: None,
            perfil: None,
        }
    }

    /// Texto que o humano lê antes de confirmar. Curto, específico, sem jargão.
    pub fn resumo(&self) -> String {
        let alvo = if self.nome.is_empty() {
            format!("{} ({})", self.sn, self.onu)
        } else {
            format!("{} — {} ({})", self.sn, self.nome, self.onu)
        };
        match self.tipo {
            TipoEscrita::Reset => format!(
                "reiniciar a ONU {alvo} na PON {}/{}/{}/{}",
                self.rack, self.subrack, self.slot, self.porta
            ),
            TipoEscrita::Descricao => format!(
                "mudar a descrição de {alvo}: {:?} → {:?}",
                self.texto_anterior.clone().unwrap_or_default(),
                self.texto.clone().unwrap_or_default()
            ),
            TipoEscrita::Autorizar => format!(
                "autorizar a ONU {} na PON {}/{}/{}/{} (modo {})",
                self.sn,
                self.rack,
                self.subrack,
                self.slot,
                self.porta,
                self.modo.clone().unwrap_or_else(|| "sn".into())
            ),
            TipoEscrita::Desautorizar => {
                format!("desautorizar a ONU {alvo} — o cliente perde o serviço")
            }
        }
    }

    /// O comando canônico correspondente. É a **única** tradução de pedido para execução.
    pub fn comando(&self) -> CanonicalCmd {
        match self.tipo {
            TipoEscrita::Reset => CanonicalCmd::ResetOnu { onu: OnuId::novo(&self.onu) },
            TipoEscrita::Descricao => CanonicalCmd::SetOnuDescription {
                onu: OnuId::novo(&self.onu),
                texto: self.texto.clone().unwrap_or_default(),
            },
            TipoEscrita::Autorizar => CanonicalCmd::AuthorizeOnu {
                olt: OltId::novo(&self.olt),
                pon: PonRef::new(self.rack, self.subrack, self.slot, self.porta),
                sn: self.sn.clone(),
                modo: match self.modo.as_deref() {
                    Some("loid") => AuthMode::Loid,
                    Some("password") => AuthMode::Password,
                    Some("none") | Some("sem-autenticacao") => AuthMode::None,
                    _ => AuthMode::Sn,
                },
                perfil: self.perfil.clone(),
            },
            TipoEscrita::Desautorizar => {
                CanonicalCmd::DeauthorizeOnu { onu: OnuId::novo(&self.onu) }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroGuarda {
    TokenDesconhecido,
    TokenExpirado,
    /// Recusa antes de chegar ao NE: pedido sem sentido (texto vazio, texto igual, grande demais).
    PedidoInvalido { motivo: String },
}

impl fmt::Display for ErroGuarda {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TokenDesconhecido => f.write_str(
                "confirmação inválida: faça a prévia de novo (o token é de uso único)",
            ),
            Self::TokenExpirado => {
                f.write_str("prévia expirada: refaça a prévia antes de confirmar")
            }
            Self::PedidoInvalido { motivo } => write!(f, "pedido recusado: {motivo}"),
        }
    }
}

struct Pendente {
    pedido: Pedido,
    criado: Instant,
}

/// Guarda das escritas. Uma instância por aplicação.
pub struct Guarda {
    pendentes: Mutex<BTreeMap<String, Pendente>>,
    validade: Duration,
}

impl Default for Guarda {
    fn default() -> Self {
        Self::nova(VALIDADE_PADRAO)
    }
}

impl Guarda {
    pub fn nova(validade: Duration) -> Self {
        Self { pendentes: Mutex::new(BTreeMap::new()), validade }
    }

    /// Valida o pedido e emite o token. Nada é executado aqui.
    pub fn previa(&self, pedido: Pedido) -> Result<Token, ErroGuarda> {
        self.validar(&pedido)?;
        let token = token_novo();
        let mut p = self.pendentes.lock().expect("guarda não envenena");
        // limpa o que já morreu, para o mapa não crescer sem limite
        p.retain(|_, v| v.criado.elapsed() < self.validade);
        p.insert(token.chave.clone(), Pendente { pedido, criado: Instant::now() });
        Ok(token)
    }

    /// Consome o token e devolve **o pedido que foi mostrado**. Uso único.
    pub fn confirmar(&self, chave: &str) -> Result<Pedido, ErroGuarda> {
        let mut p = self.pendentes.lock().expect("guarda não envenena");
        let Some(pendente) = p.remove(chave) else {
            return Err(ErroGuarda::TokenDesconhecido);
        };
        if pendente.criado.elapsed() >= self.validade {
            return Err(ErroGuarda::TokenExpirado);
        }
        Ok(pendente.pedido)
    }

    #[allow(dead_code)] // usado nos testes e útil para diagnóstico
    pub fn pendentes(&self) -> usize {
        self.pendentes.lock().expect("guarda não envenena").len()
    }

    fn validar(&self, pedido: &Pedido) -> Result<(), ErroGuarda> {
        match pedido.tipo {
            TipoEscrita::Descricao => {
                let novo = pedido.texto.as_deref().unwrap_or("");
                if novo.trim().is_empty() {
                    return Err(ErroGuarda::PedidoInvalido {
                        motivo: "descrição vazia não é alteração (use o campo com o texto desejado)"
                            .into(),
                    });
                }
                if novo.chars().count() > MAX_DESCRICAO {
                    return Err(ErroGuarda::PedidoInvalido {
                        motivo: format!(
                            "descrição com {} caracteres; o limite aqui é {MAX_DESCRICAO}",
                            novo.chars().count()
                        ),
                    });
                }
                if Some(novo) == pedido.texto_anterior.as_deref() {
                    return Err(ErroGuarda::PedidoInvalido {
                        motivo: "o texto novo é igual ao atual — nada a alterar".into(),
                    });
                }
                if pedido.onu.trim().is_empty() {
                    return Err(ErroGuarda::PedidoInvalido { motivo: "ONU não informada".into() });
                }
            }
            TipoEscrita::Reset | TipoEscrita::Desautorizar => {
                if pedido.onu.trim().is_empty() {
                    return Err(ErroGuarda::PedidoInvalido { motivo: "ONU não informada".into() });
                }
            }
            TipoEscrita::Autorizar => {
                if pedido.sn.trim().is_empty() {
                    return Err(ErroGuarda::PedidoInvalido { motivo: "SN não informado".into() });
                }
            }
        }
        Ok(())
    }
}

/// Token opaco mostrado à tela. `chave` é o segredo; `texto` é o que a UI exibe.
#[derive(Debug, Clone)]
pub struct Token {
    /// O que a UI devolve na confirmação. Não é exibido por inteiro (ver `curto`).
    pub chave: String,
    pub validade_s: u64,
}

impl Token {
    /// Só os 8 primeiros caracteres vão para a tela — o suficiente para o operador ver que mudou.
    pub fn curto(&self) -> String {
        self.chave.chars().take(8).collect()
    }
}

/// Token sem dependência: dois `RandomState` (semeados pelo processo) viram 128 bits.
/// Não é fronteira de segurança contra atacante local — é contra clique repetido e replay.
fn token_novo() -> Token {
    let a = std::collections::hash_map::RandomState::new().build_hasher().finish();
    let b = std::collections::hash_map::RandomState::new().build_hasher().finish();
    Token {
        chave: format!("{a:016x}{b:016x}"),
        validade_s: VALIDADE_PADRAO.as_secs(),
    }
}

// ---------- auditoria ----------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistroAuditoria {
    pub quando: String,
    pub operacao: String,
    pub alvo: String,
    pub resumo: String,
    pub resultado: String,
    pub mensagem: String,
}

impl RegistroAuditoria {
    pub fn linha_json(&self) -> String {
        let esc = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
        format!(
            "{{\"quando\":{},\"operacao\":{},\"alvo\":{},\"resumo\":{},\"resultado\":{},\"mensagem\":{}}}",
            esc(&self.quando),
            esc(&self.operacao),
            esc(&self.alvo),
            esc(&self.resumo),
            esc(&self.resultado),
            esc(&self.mensagem)
        )
    }
}

/// Auditoria em memória, com espelho em arquivo quando o app informa o caminho (JSONL, append).
/// Escrever a auditoria é **secundário**: se o arquivo falhar, a operação não é desfeita — mas a
/// falha é avisada, não engolida.
#[derive(Default)]
pub struct Auditoria {
    registros: Mutex<Vec<RegistroAuditoria>>,
    caminho: Mutex<Option<std::path::PathBuf>>,
}

impl Auditoria {
    pub fn definir_caminho(&self, caminho: impl Into<std::path::PathBuf>) {
        *self.caminho.lock().expect("auditoria não envenena") = Some(caminho.into());
    }

    pub fn registrar(&self, r: RegistroAuditoria) -> Option<String> {
        let linha = r.linha_json();
        self.registros.lock().expect("auditoria não envenena").push(r);
        let caminho = self.caminho.lock().expect("auditoria não envenena").clone();
        if let Some(c) = caminho {
            use std::io::Write;
            if let Some(dir) = c.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let escrito = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&c)
                .and_then(|mut f| f.write_all(format!("{linha}\n").as_bytes()));
            if let Err(e) = escrito {
                return Some(format!("auditoria não pôde ser gravada em {}: {e}", c.display()));
            }
        }
        None
    }

    pub fn ultimos(&self, n: usize) -> Vec<RegistroAuditoria> {
        let g = self.registros.lock().expect("auditoria não envenena");
        g.iter().rev().take(n).cloned().collect()
    }

    pub fn total(&self) -> usize {
        self.registros.lock().expect("auditoria não envenena").len()
    }
}

/// Hora legível em UTC, sem dependência (`YYYY-MM-DDTHH:MM:SSZ`).
pub fn agora_utc() -> String {
    let s = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!(
        "{}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        ano_de(s),
        mes_de(s),
        dia_de(s),
        (s % 86_400) / 3600,
        (s % 3600) / 60,
        s % 60
    )
}

fn dias_desde_epoca(ano: i64, mes: i64, dia: i64) -> i64 {
    let y = if mes <= 2 { ano - 1 } else { ano };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let ano_na_era = y - era * 400;
    let mes_deslocado = (mes + 9) % 12;
    let dia_do_ano = (153 * mes_deslocado + 2) / 5 + dia - 1;
    era * 146_097 + ano_na_era * 365 + ano_na_era / 4 - ano_na_era / 100 + dia_do_ano - 719_468
}

/// Inverso de `dias_desde_epoca` (Hinnant): dias → ano, mês, dia.
fn civil_de_dias(dias: i64) -> (i64, i64, i64) {
    let z = dias + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn ano_de(s: u64) -> i64 {
    civil_de_dias((s / 86_400) as i64).0
}
fn mes_de(s: u64) -> i64 {
    civil_de_dias((s / 86_400) as i64).1
}
fn dia_de(s: u64) -> i64 {
    civil_de_dias((s / 86_400) as i64).2
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pedido_reset() -> Pedido {
        Pedido::reset("olt-ma5800-1", 1, 1, 1, 1, "onu-1", "48575443A1B2C3D4", "cliente-0001")
    }

    fn pedido_desc(novo: &str) -> Pedido {
        Pedido::descricao(
            "olt-ma5800-1",
            1,
            1,
            1,
            1,
            "onu-1",
            "48575443A1B2C3D4",
            "cliente-0001",
            "portaria",
            novo,
        )
    }

    #[test]
    fn sem_previa_nao_confirma() {
        let g = Guarda::default();
        assert_eq!(g.confirmar("deadbeef").unwrap_err(), ErroGuarda::TokenDesconhecido);
        assert_eq!(g.pendentes(), 0);
    }

    #[test]
    fn token_e_de_uso_unico() {
        let g = Guarda::default();
        let t = g.previa(pedido_reset()).unwrap();
        assert!(g.confirmar(&t.chave).is_ok(), "primeira confirmação vale");
        assert_eq!(
            g.confirmar(&t.chave).unwrap_err(),
            ErroGuarda::TokenDesconhecido,
            "segunda confirmação tem de falhar"
        );
    }

    #[test]
    fn previa_expirada_nao_executa() {
        let g = Guarda::nova(Duration::from_millis(1));
        let t = g.previa(pedido_reset()).unwrap();
        std::thread::sleep(Duration::from_millis(10));
        assert_eq!(g.confirmar(&t.chave).unwrap_err(), ErroGuarda::TokenExpirado);
    }

    #[test]
    fn confirmar_devolve_o_pedido_mostrado_nao_outro() {
        let g = Guarda::default();
        let t = g.previa(pedido_desc("loja centro")).unwrap();
        let p = g.confirmar(&t.chave).unwrap();
        assert_eq!(p.tipo, TipoEscrita::Descricao);
        assert_eq!(p.texto.as_deref(), Some("loja centro"));
        assert_eq!(p.texto_anterior.as_deref(), Some("portaria"));
        // e o comando executado é o de descrição, não outro
        assert!(matches!(p.comando(), CanonicalCmd::SetOnuDescription { .. }));
    }

    #[test]
    fn tokens_diferem_entre_previas() {
        let g = Guarda::default();
        let a = g.previa(pedido_reset()).unwrap();
        let b = g.previa(pedido_reset()).unwrap();
        assert_ne!(a.chave, b.chave);
        assert_eq!(g.pendentes(), 2);
    }

    #[test]
    fn descricao_vazia_e_recusada_antes_do_ne() {
        let g = Guarda::default();
        let e = g.previa(pedido_desc("   ")).unwrap_err();
        assert!(matches!(e, ErroGuarda::PedidoInvalido { .. }), "{e:?}");
        assert!(e.to_string().contains("vazia"));
        assert_eq!(g.pendentes(), 0);
    }

    #[test]
    fn descricao_igual_a_atual_e_recusada() {
        let g = Guarda::default();
        let e = g.previa(pedido_desc("portaria")).unwrap_err();
        assert!(e.to_string().contains("igual ao atual"));
    }

    #[test]
    fn descricao_longa_demais_e_recusada() {
        let g = Guarda::default();
        let e = g.previa(pedido_desc(&"x".repeat(MAX_DESCRICAO + 1))).unwrap_err();
        assert!(e.to_string().contains(&MAX_DESCRICAO.to_string()));
    }

    #[test]
    fn resumo_diz_o_que_vai_acontecer() {
        assert_eq!(
            pedido_reset().resumo(),
            "reiniciar a ONU 48575443A1B2C3D4 — cliente-0001 (onu-1) na PON 1/1/1/1"
        );
        assert_eq!(
            pedido_desc("loja").resumo(),
            "mudar a descrição de 48575443A1B2C3D4 — cliente-0001 (onu-1): \"portaria\" → \"loja\""
        );
        assert!(Pedido::desautorizar("o", 1, 1, 1, 1, "onu-1", "SN", "n").resumo().contains("perde o serviço"));
    }

    #[test]
    fn reset_e_descricao_viram_os_comandos_certos() {
        assert!(matches!(pedido_reset().comando(), CanonicalCmd::ResetOnu { .. }));
        assert!(matches!(pedido_desc("x").comando(), CanonicalCmd::SetOnuDescription { .. }));
        assert!(matches!(
            Pedido::desautorizar("o", 1, 1, 1, 1, "onu-1", "SN", "n").comando(),
            CanonicalCmd::DeauthorizeOnu { .. }
        ));
        assert!(matches!(
            Pedido::autorizar("o", 1, 1, 1, 1, "SN", "sn", None).comando(),
            CanonicalCmd::AuthorizeOnu { .. }
        ));
    }

    #[test]
    fn auditoria_guarda_e_espelha_em_arquivo() {
        let dir = std::env::temp_dir().join(format!("suite-gpon-audit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let a = Auditoria::default();
        a.definir_caminho(dir.join("auditoria.jsonl"));
        let aviso = a.registrar(RegistroAuditoria {
            quando: agora_utc(),
            operacao: "reset_onu".into(),
            alvo: "onu-1".into(),
            resumo: "reiniciar a ONU X".into(),
            resultado: "ok".into(),
            mensagem: "(mock) reset".into(),
        });
        assert!(aviso.is_none(), "gravar auditoria não deve falhar: {aviso:?}");
        assert_eq!(a.total(), 1);
        assert_eq!(a.ultimos(5)[0].resultado, "ok");

        let texto = std::fs::read_to_string(dir.join("auditoria.jsonl")).unwrap();
        assert!(texto.contains("\"operacao\":\"reset_onu\""));
        assert!(texto.trim_end().ends_with('}'), "uma linha JSON por registro");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn auditoria_registra_falha_tambem() {
        let a = Auditoria::default();
        a.registrar(RegistroAuditoria {
            quando: agora_utc(),
            operacao: "reset_onu".into(),
            alvo: "onu-9".into(),
            resumo: "reiniciar".into(),
            resultado: "falha".into(),
            mensagem: "não encontrado".into(),
        });
        assert_eq!(a.total(), 1);
        assert_eq!(a.ultimos(1)[0].resultado, "falha");
    }

    #[test]
    fn hora_utc_tem_formato_iso() {
        let h = agora_utc();
        assert_eq!(h.len(), 20, "{h}");
        assert!(h.ends_with('Z'));
        assert_eq!(&h[4..5], "-");
        assert_eq!(&h[10..11], "T");
        // 2026-10-17T… (a data do sistema nesta sessão)
        assert!(h.starts_with("20"), "{h}");
    }

    #[test]
    fn conversao_de_dias_ida_e_volta() {
        // 1970-01-01 e a data do projeto
        assert_eq!(dias_desde_epoca(1970, 1, 1), 0);
        assert_eq!(civil_de_dias(0), (1970, 1, 1));
        let d = dias_desde_epoca(2026, 10, 17);
        assert_eq!(civil_de_dias(d), (2026, 10, 17));
    }
}
