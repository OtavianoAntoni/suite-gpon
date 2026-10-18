//! Cofre de credenciais — **contrato único**, std-only.
//!
//! Regra que dá o tom (SPEC §6/§9): credencial nunca mora em arquivo de configuração nem em log. O
//! que circula é uma **referência** (`cofre://lab-1`); o segredo só existe no cofre do sistema
//! (Windows Credential Manager, Secret Service no Linux, Keychain no macOS) e sai daqui apenas no
//! momento de autenticar.
//!
//! O cofre de verdade é implementado na casca (`src-tauri`), porque a dependência (`keyring`) não
//! pode contaminar o núcleo. Testes usam [`CofreEmMemoria`] — nenhum teste depende de daemon.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Mutex;

/// Uma senha/token embrulhado. `Debug` e `Display` **não** mostram o conteúdo: se alguém logar
/// `{:?}` de um struct que carrega isto, o segredo não vaza. Para usar de fato, [`Segredo::expor`].
#[derive(Clone, PartialEq, Eq)]
pub struct Segredo(String);

impl Segredo {
    pub fn novo(valor: impl Into<String>) -> Self {
        Self(valor.into())
    }

    /// Único caminho pelo qual o valor aparece. Quem chama isto aparece no `grep` da revisão.
    pub fn expor(&self) -> &str {
        &self.0
    }

    pub fn vazio(&self) -> bool {
        self.0.trim().is_empty()
    }
}

impl fmt::Debug for Segredo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Segredo([REDACTED])")
    }
}

impl fmt::Display for Segredo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroCofre {
    /// A referência não segue o formato `cofre://<id>` — erro de configuração, não de rede.
    ReferenciaInvalida { referencia: String },
    NaoEncontrado { referencia: String },
    /// O cofre do sistema recusou (daemon ausente, permissão, política). **Nunca** cai para arquivo.
    FalhaDoSistema { referencia: String, motivo: String },
}

impl fmt::Display for ErroCofre {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReferenciaInvalida { referencia } => {
                write!(f, "referência de cofre inválida: {referencia:?} (esperado cofre://<id>)")
            }
            Self::NaoEncontrado { referencia } => {
                write!(f, "credencial ausente no cofre: {referencia}")
            }
            Self::FalhaDoSistema { referencia, motivo } => {
                write!(f, "cofre do sistema falhou para {referencia}: {motivo}")
            }
        }
    }
}

impl std::error::Error for ErroCofre {}

/// O contrato. Implementações: `CofreDoSistema` (casca) e [`CofreEmMemoria`] (testes).
pub trait Cofre: Send + Sync {
    fn guardar(&self, referencia: &str, segredo: &Segredo) -> Result<(), ErroCofre>;
    fn ler(&self, referencia: &str) -> Result<Segredo, ErroCofre>;
    fn existe(&self, referencia: &str) -> bool;
    fn remover(&self, referencia: &str) -> Result<(), ErroCofre>;
}

/// `cofre://<id>`: prefixo obrigatório, id não vazio, sem espaço. Recusar cedo evita gravar
/// credencial no lugar errado (nome de arquivo, por exemplo).
pub fn referencia_valida(referencia: &str) -> bool {
    let Some(id) = referencia.strip_prefix("cofre://") else {
        return false;
    };
    !id.is_empty()
        && id.len() <= 128
        && !id.chars().any(|c| c.is_whitespace() || c.is_control())
        && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':'))
}

/// Valida antes de tocar no cofre — usado por todas as implementações, para o erro ser o mesmo.
pub fn exigir_referencia(referencia: &str) -> Result<(), ErroCofre> {
    if referencia_valida(referencia) {
        Ok(())
    } else {
        Err(ErroCofre::ReferenciaInvalida { referencia: referencia.to_string() })
    }
}

/// Cofre de teste: guarda em memória do processo. **Não** persiste — e é isso que se quer num teste.
#[derive(Default)]
pub struct CofreEmMemoria {
    itens: Mutex<BTreeMap<String, String>>,
}

impl CofreEmMemoria {
    pub fn novo() -> Self {
        Self::default()
    }

    pub fn quantidade(&self) -> usize {
        self.itens.lock().expect("cofre em memória não envenena").len()
    }
}

impl Cofre for CofreEmMemoria {
    fn guardar(&self, referencia: &str, segredo: &Segredo) -> Result<(), ErroCofre> {
        exigir_referencia(referencia)?;
        self.itens
            .lock()
            .expect("cofre em memória não envenena")
            .insert(referencia.to_string(), segredo.expor().to_string());
        Ok(())
    }

    fn ler(&self, referencia: &str) -> Result<Segredo, ErroCofre> {
        exigir_referencia(referencia)?;
        self.itens
            .lock()
            .expect("cofre em memória não envenena")
            .get(referencia)
            .map(|s| Segredo::novo(s.clone()))
            .ok_or_else(|| ErroCofre::NaoEncontrado { referencia: referencia.to_string() })
    }

    fn existe(&self, referencia: &str) -> bool {
        if !referencia_valida(referencia) {
            return false;
        }
        self.itens
            .lock()
            .expect("cofre em memória não envenena")
            .contains_key(referencia)
    }

    fn remover(&self, referencia: &str) -> Result<(), ErroCofre> {
        exigir_referencia(referencia)?;
        self.itens
            .lock()
            .expect("cofre em memória não envenena")
            .remove(referencia)
            .map(|_| ())
            .ok_or_else(|| ErroCofre::NaoEncontrado { referencia: referencia.to_string() })
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn segredo_nao_vaza_em_debug_nem_em_display() {
        let s = Segredo::novo("senha-super-secreta");
        assert_eq!(format!("{s:?}"), "Segredo([REDACTED])");
        assert_eq!(format!("{s}"), "[REDACTED]");
        assert_eq!(s.expor(), "senha-super-secreta");

        // o caso real: logar uma struct que carrega o segredo
        #[derive(Debug)]
        struct Conexao {
            host: String,
            _segredo: Segredo,
        }
        let c = Conexao { host: "10.0.0.10".into(), _segredo: s };
        let log = format!("{c:?}");
        assert!(log.contains("10.0.0.10"));
        assert!(!log.contains("super-secreta"), "o log vazou a senha: {log}");
    }

    #[test]
    fn referencia_precisa_do_prefixo_e_de_id_util() {
        for boa in ["cofre://lab-1", "cofre://olt/ma5800-1", "cofre://a"] {
            assert!(referencia_valida(boa), "deveria aceitar {boa}");
        }
        for ruim in ["", "lab-1", "cofre://", "cofre://com espaço", "cofre://a\nb", "arquivo://x"] {
            assert!(!referencia_valida(ruim), "deveria recusar {ruim:?}");
        }
    }

    #[test]
    fn guardar_e_ler_devolvem_o_mesmo_segredo() {
        let c = CofreEmMemoria::novo();
        c.guardar("cofre://lab-1", &Segredo::novo("s3nh4")).unwrap();
        assert_eq!(c.ler("cofre://lab-1").unwrap().expor(), "s3nh4");
        assert!(c.existe("cofre://lab-1"));
        assert_eq!(c.quantidade(), 1);
    }

    #[test]
    fn ler_inexistente_e_erro_proprio_nao_string_vazia() {
        let c = CofreEmMemoria::novo();
        let e = c.ler("cofre://nunca-gravado").expect_err("não existe");
        assert_eq!(
            e,
            ErroCofre::NaoEncontrado { referencia: "cofre://nunca-gravado".into() }
        );
        assert!(e.to_string().contains("cofre://nunca-gravado"));
    }

    #[test]
    fn referencia_invalida_nao_grava_nada() {
        let c = CofreEmMemoria::novo();
        let e = c.guardar("lab-1", &Segredo::novo("x")).expect_err("sem prefixo");
        assert!(matches!(e, ErroCofre::ReferenciaInvalida { .. }));
        assert_eq!(c.quantidade(), 0, "nada pode ter sido gravado");
        assert!(!c.existe("lab-1"));
    }

    #[test]
    fn remover_duas_vezes_falha_na_segunda() {
        let c = CofreEmMemoria::novo();
        c.guardar("cofre://lab-1", &Segredo::novo("x")).unwrap();
        c.remover("cofre://lab-1").unwrap();
        assert!(matches!(
            c.remover("cofre://lab-1"),
            Err(ErroCofre::NaoEncontrado { .. })
        ));
        assert_eq!(c.quantidade(), 0);
    }

    #[test]
    fn segredo_vazio_e_detectavel_para_nao_autenticar_com_nada() {
        assert!(Segredo::novo("   ").vazio());
        assert!(!Segredo::novo("x").vazio());
    }
}
