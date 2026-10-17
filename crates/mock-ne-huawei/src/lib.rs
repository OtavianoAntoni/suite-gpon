//! `mock-ne-huawei` — o "NE" das Fases 0 e 1, agora dirigido por **fixtures por perfil**.
//!
//! Decisão P4 (§0.2): o mock nasce com **duas famílias** — MA5800 e MA5600T — e o perfil entra como
//! **parâmetro**, não como `cfg!`. O que realmente muda entre famílias é (a) os números do
//! equipamento e (b) o **dicionário MML**; o transporte e o contrato canônico são os mesmos.
//!
//! O transporte continua sendo o [`InMemoryTransport`] do crate `gpon-transport`: as fixtures são
//! a *carga* dele. Trocar fixture por rede, na Fase 2, não toca em quem consome o contrato.

use gpon_domain::ops::Module;
use gpon_transport::testing::InMemoryTransport;
use gpon_transport::{
    Alarm, Olt, OltId, Onu, OnuEstado, OnuId, PonPort, PonRef, Profile, ProfileKind, Severidade,
    Source, SourceId, Tecnologia,
};
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

/// Perfis de fixture disponíveis (P4: as duas famílias).
pub const PERFIS: [&str; 2] = ["ma5800", "ma5600t"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeloNe {
    Ma5800,
    Ma5600t,
}

impl ModeloNe {
    /// Converte o argumento de linha de comando (`--model ma5800`).
    pub fn do_arg(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "ma5800" => Ok(Self::Ma5800),
            "ma5600t" => Ok(Self::Ma5600t),
            outro => Err(format!(
                "perfil desconhecido: {outro:?} (aceitos: {})",
                PERFIS.join(", ")
            )),
        }
    }

    pub fn perfil(self) -> &'static str {
        match self {
            Self::Ma5800 => "ma5800",
            Self::Ma5600t => "ma5600t",
        }
    }

    pub fn todos() -> [ModeloNe; 2] {
        [Self::Ma5800, Self::Ma5600t]
    }
}

impl fmt::Display for ModeloNe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.perfil())
    }
}

/// Erro de fixture com **arquivo e linha** — fixture errada tem de ser diagnosticável.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroFixture {
    pub arquivo: String,
    pub linha: usize,
    pub motivo: String,
}

impl fmt::Display for ErroFixture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.arquivo, self.linha, self.motivo)
    }
}

impl std::error::Error for ErroFixture {}

/// Uma fixture carregada: o transporte pronto + o catálogo MML do perfil.
pub struct Fixture {
    pub modelo: ModeloNe,
    pub transporte: InMemoryTransport,
    /// operação canônica → comando MML **desta família** (dicionário do perfil).
    pub catalogo_mml: BTreeMap<String, String>,
}

fn dir_fixture(modelo: ModeloNe) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(modelo.perfil())
}

fn ler_linhas(caminho: &PathBuf) -> Result<Vec<(usize, String)>, ErroFixture> {
    let texto = std::fs::read_to_string(caminho).map_err(|e| ErroFixture {
        arquivo: caminho.display().to_string(),
        linha: 0,
        motivo: format!("não foi possível ler: {e}"),
    })?;
    Ok(texto
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim().to_string()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'))
        .collect())
}

/// `olt|id=x|modelo=y` → ("olt", [("id","x"), ("modelo","y")])
fn separar(linha: &str) -> (&str, Vec<(&str, &str)>) {
    let mut partes = linha.split('|');
    let tipo = partes.next().unwrap_or("").trim();
    let campos = partes
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.trim(), v.trim()))
        .collect();
    (tipo, campos)
}

fn campo<'a>(campos: &'a [(&'a str, &'a str)], chave: &str) -> Option<&'a str> {
    campos.iter().find(|(k, _)| *k == chave).map(|(_, v)| *v)
}

fn exigir<'a>(
    campos: &'a [(&'a str, &'a str)],
    chave: &str,
    arquivo: &str,
    linha: usize,
) -> Result<&'a str, ErroFixture> {
    campo(campos, chave).ok_or_else(|| ErroFixture {
        arquivo: arquivo.to_string(),
        linha,
        motivo: format!("campo obrigatório ausente: {chave}"),
    })
}

fn num<T: std::str::FromStr>(
    campos: &[(&str, &str)],
    chave: &str,
    arquivo: &str,
    linha: usize,
) -> Result<Option<T>, ErroFixture> {
    match campo(campos, chave) {
        None | Some("") => Ok(None),
        Some(v) => v.parse::<T>().map(Some).map_err(|_| ErroFixture {
            arquivo: arquivo.to_string(),
            linha,
            motivo: format!("campo {chave} não é número: {v:?}"),
        }),
    }
}

fn tecnologia(s: &str) -> Result<Tecnologia, String> {
    match s.to_ascii_lowercase().as_str() {
        "gpon" => Ok(Tecnologia::Gpon),
        "epon" => Ok(Tecnologia::Epon),
        "xgs" | "xgs-pon" => Ok(Tecnologia::XgsPon),
        "" => Ok(Tecnologia::Desconhecida),
        outro => Err(format!("tecnologia desconhecida: {outro}")),
    }
}

fn estado(s: &str) -> Result<OnuEstado, String> {
    match s.to_ascii_lowercase().as_str() {
        "online" => Ok(OnuEstado::Online),
        "offline" => Ok(OnuEstado::Offline),
        "los" => Ok(OnuEstado::Los),
        "" => Ok(OnuEstado::Desconhecido),
        outro => Err(format!("estado desconhecido: {outro}")),
    }
}

fn severidade(s: &str) -> Result<Severidade, String> {
    match s.to_ascii_lowercase().as_str() {
        "critical" | "critica" => Ok(Severidade::Critica),
        "major" | "maior" => Ok(Severidade::Maior),
        "minor" | "menor" => Ok(Severidade::Menor),
        "warning" | "aviso" => Ok(Severidade::Aviso),
        "info" => Ok(Severidade::Info),
        outro => Err(format!("severidade desconhecida: {outro}")),
    }
}

fn tipo_perfil(s: &str) -> Result<ProfileKind, String> {
    match s.to_ascii_lowercase().as_str() {
        "line" => Ok(ProfileKind::Line),
        "dba" => Ok(ProfileKind::Dba),
        "bandwidth" => Ok(ProfileKind::Bandwidth),
        "qinq" => Ok(ProfileKind::QinQ),
        "vlan" => Ok(ProfileKind::Vlan),
        "voip" => Ok(ProfileKind::Voip),
        "onu_type" => Ok(ProfileKind::OnuType),
        "capability_set" => Ok(ProfileKind::CapabilitySet),
        outro => Err(format!("tipo de perfil desconhecido: {outro}")),
    }
}

impl Fixture {
    /// Carrega `fixtures/<perfil>/dados.txt` + `catalogo_mml.txt`.
    pub fn carregar(modelo: ModeloNe) -> Result<Self, ErroFixture> {
        let dir = dir_fixture(modelo);
        let dados = dir.join("dados.txt");
        let nome_arq = dados.display().to_string();

        let mut t = InMemoryTransport::new(Module::C);

        for (n, linha) in ler_linhas(&dados)? {
            let (tipo, c) = separar(&linha);
            let erro = |m: String| ErroFixture { arquivo: nome_arq.clone(), linha: n, motivo: m };

            match tipo {
                "fonte" => t.fontes.push(Source {
                    id: SourceId::novo(exigir(&c, "id", &nome_arq, n)?),
                    rotulo: exigir(&c, "rotulo", &nome_arq, n)?.to_string(),
                    host: exigir(&c, "host", &nome_arq, n)?.to_string(),
                    protocolo: exigir(&c, "protocolo", &nome_arq, n)?.to_string(),
                    credencial_ref: exigir(&c, "credencial_ref", &nome_arq, n)?.to_string(),
                }),

                "olt" => t.olts.push(Olt {
                    id: OltId::novo(exigir(&c, "id", &nome_arq, n)?),
                    fonte: SourceId::novo(exigir(&c, "fonte", &nome_arq, n)?),
                    vendor: exigir(&c, "vendor", &nome_arq, n)?.to_string(),
                    modelo: exigir(&c, "modelo", &nome_arq, n)?.to_string(),
                    serial: campo(&c, "serial").unwrap_or("").to_string(),
                    ip: exigir(&c, "ip", &nome_arq, n)?.to_string(),
                    sw_version: exigir(&c, "sw", &nome_arq, n)?.to_string(),
                    uptime_s: num(&c, "uptime", &nome_arq, n)?.unwrap_or(0),
                    slots: num(&c, "slots", &nome_arq, n)?.unwrap_or(0),
                    temperatura_c: num(&c, "temp", &nome_arq, n)?,
                }),

                "pon" => {
                    let slot: u8 = num(&c, "slot", &nome_arq, n)?.unwrap_or(0);
                    let porta: u8 = num(&c, "porta", &nome_arq, n)?.unwrap_or(0);
                    t.pons.push(PonPort {
                        olt: OltId::novo(exigir(&c, "olt", &nome_arq, n)?),
                        pon: PonRef::new(slot, porta),
                        tecnologia: tecnologia(campo(&c, "tech").unwrap_or("")).map_err(erro)?,
                        admin_up: campo(&c, "up").unwrap_or("1") == "1",
                        split_ratio: num(&c, "split", &nome_arq, n)?.unwrap_or(0),
                        onus: num(&c, "onus", &nome_arq, n)?.unwrap_or(0),
                    });
                }

                "onu" => {
                    let slot: u8 = num(&c, "slot", &nome_arq, n)?.unwrap_or(0);
                    let porta: u8 = num(&c, "porta", &nome_arq, n)?.unwrap_or(0);
                    t.onus.push(Onu {
                        id: OnuId::novo(exigir(&c, "id", &nome_arq, n)?),
                        olt: OltId::novo(exigir(&c, "olt", &nome_arq, n)?),
                        pon: PonRef::new(slot, porta),
                        sn: exigir(&c, "sn", &nome_arq, n)?.to_string(),
                        mac: campo(&c, "mac").filter(|s| !s.is_empty()).map(str::to_string),
                        modelo_hw: campo(&c, "hw").unwrap_or("").to_string(),
                        modelo_sw: campo(&c, "sw").unwrap_or("").to_string(),
                        vendor: campo(&c, "vendor").unwrap_or("").to_string(),
                        nome: campo(&c, "nome").unwrap_or("").to_string(),
                        descricao: campo(&c, "desc").unwrap_or("").to_string(),
                        estado: estado(campo(&c, "estado").unwrap_or("")).map_err(erro)?,
                        distancia_m: num(&c, "dist", &nome_arq, n)?,
                        ultimo_registro: campo(&c, "ultimo").filter(|s| !s.is_empty()).map(str::to_string),
                    });
                }

                "perfil" => t.perfis.push(Profile {
                    nome: exigir(&c, "nome", &nome_arq, n)?.to_string(),
                    kind: tipo_perfil(exigir(&c, "kind", &nome_arq, n)?).map_err(erro)?,
                    versao: campo(&c, "versao").unwrap_or("").to_string(),
                }),

                "alarme" => t.alarmes.push(Alarm {
                    id: exigir(&c, "id", &nome_arq, n)?.to_string(),
                    severidade: severidade(exigir(&c, "sev", &nome_arq, n)?).map_err(erro)?,
                    ne: OltId::novo(exigir(&c, "olt", &nome_arq, n)?),
                    objeto: campo(&c, "objeto").unwrap_or("").to_string(),
                    tipo: exigir(&c, "tipo", &nome_arq, n)?.to_string(),
                    primeira: campo(&c, "primeira").unwrap_or("").to_string(),
                    ultima: campo(&c, "ultima").unwrap_or("").to_string(),
                    ack: campo(&c, "ack").unwrap_or("0") == "1",
                }),

                outro => {
                    return Err(erro(format!(
                        "tipo de registro desconhecido: {outro:?} (esperado fonte|olt|pon|onu|perfil|alarme)"
                    )))
                }
            }
        }

        let catalogo_mml = Self::carregar_catalogo(modelo)?;
        Ok(Self { modelo, transporte: t, catalogo_mml })
    }

    fn carregar_catalogo(modelo: ModeloNe) -> Result<BTreeMap<String, String>, ErroFixture> {
        let caminho = dir_fixture(modelo).join("catalogo_mml.txt");
        let nome = caminho.display().to_string();
        let mut mapa = BTreeMap::new();
        for (n, linha) in ler_linhas(&caminho)? {
            let (op, cmd) = linha.split_once('=').ok_or_else(|| ErroFixture {
                arquivo: nome.clone(),
                linha: n,
                motivo: "esperado <operacao>=<comando>".to_string(),
            })?;
            mapa.insert(op.trim().to_string(), cmd.trim().to_string());
        }
        Ok(mapa)
    }

    /// Comando MML do perfil para uma operação canônica (o que muda entre famílias).
    pub fn comando_mml(&self, operacao_canonica: &str) -> Option<&str> {
        self.catalogo_mml.get(operacao_canonica).map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_dois_perfis_carregam() {
        for m in ModeloNe::todos() {
            let f = Fixture::carregar(m).unwrap_or_else(|e| panic!("perfil {m}: {e}"));
            assert!(!f.transporte.olts.is_empty(), "perfil {m} sem OLT");
            assert!(!f.transporte.onus.is_empty(), "perfil {m} sem ONU");
            assert_eq!(f.modelo, m);
        }
    }

    #[test]
    fn args_do_binario_sao_estritos() {
        assert_eq!(ModeloNe::do_arg("ma5800"), Ok(ModeloNe::Ma5800));
        assert_eq!(ModeloNe::do_arg("MA5600T"), Ok(ModeloNe::Ma5600t));
        assert!(ModeloNe::do_arg("ma9999").is_err());
    }

    #[test]
    fn erro_de_fixture_diz_arquivo_e_linha() {
        let e = ErroFixture { arquivo: "x.txt".into(), linha: 7, motivo: "campo ausente".into() };
        let texto = e.to_string();
        assert!(texto.contains("x.txt:7"), "mensagem sem localização: {texto}");
    }

    #[test]
    fn catalogo_cobre_as_16_operacoes_da_fase_1() {
        for m in ModeloNe::todos() {
            let f = Fixture::carregar(m).unwrap();
            let esperado: Vec<&str> = gpon_domain::phase1::PHASE1_READS
                .iter()
                .chain(gpon_domain::phase1::PHASE1_WRITES.iter())
                .copied()
                .collect();
            for op in esperado {
                assert!(
                    f.comando_mml(op).is_some(),
                    "perfil {m}: catálogo MML sem a operação `{op}`"
                );
            }
        }
    }
}
