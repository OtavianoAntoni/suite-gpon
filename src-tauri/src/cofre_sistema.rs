//! Cofre real do sistema operacional — **só aqui** a dependência `keyring` aparece.
//!
//! Windows Credential Manager, Secret Service (Linux), Keychain (macOS). O núcleo (`gpon-cofre`)
//! não conhece nada disso: conhece o trait.
//!
//! Regra que este arquivo faz valer: **se o cofre do sistema não estiver disponível, a suíte
//! recusa — nunca cai para arquivo em texto**. Credencial em arquivo é o que os legados faziam.

use gpon_cofre::{exigir_referencia, Cofre, ErroCofre, Segredo};

/// Prefixo do "serviço" no cofre do SO. Fica visível para quem abrir o gerenciador de credenciais.
pub const SERVICO: &str = "suite-gpon";

pub struct CofreDoSistema {
    servico: String,
}

impl Default for CofreDoSistema {
    fn default() -> Self {
        Self::novo(SERVICO)
    }
}

impl CofreDoSistema {
    pub fn novo(servico: &str) -> Self {
        Self { servico: servico.to_string() }
    }

    fn entrada(&self, referencia: &str) -> Result<keyring::Entry, ErroCofre> {
        keyring::Entry::new(&self.servico, referencia).map_err(|e| ErroCofre::FalhaDoSistema {
            referencia: referencia.to_string(),
            motivo: e.to_string(),
        })
    }
}

impl Cofre for CofreDoSistema {
    fn guardar(&self, referencia: &str, segredo: &Segredo) -> Result<(), ErroCofre> {
        exigir_referencia(referencia)?;
        if segredo.vazio() {
            return Err(ErroCofre::FalhaDoSistema {
                referencia: referencia.to_string(),
                motivo: "segredo vazio: nada a guardar".into(),
            });
        }
        self.entrada(referencia)?.set_password(segredo.expor()).map_err(|e| {
            ErroCofre::FalhaDoSistema { referencia: referencia.to_string(), motivo: e.to_string() }
        })
    }

    fn ler(&self, referencia: &str) -> Result<Segredo, ErroCofre> {
        exigir_referencia(referencia)?;
        match self.entrada(referencia)?.get_password() {
            Ok(s) => Ok(Segredo::novo(s)),
            Err(keyring::Error::NoEntry) => {
                Err(ErroCofre::NaoEncontrado { referencia: referencia.to_string() })
            }
            Err(e) => Err(ErroCofre::FalhaDoSistema {
                referencia: referencia.to_string(),
                motivo: e.to_string(),
            }),
        }
    }

    fn existe(&self, referencia: &str) -> bool {
        matches!(self.ler(referencia), Ok(_))
    }

    fn remover(&self, referencia: &str) -> Result<(), ErroCofre> {
        exigir_referencia(referencia)?;
        match self.entrada(referencia)?.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => {
                Err(ErroCofre::NaoEncontrado { referencia: referencia.to_string() })
            }
            Err(e) => Err(ErroCofre::FalhaDoSistema {
                referencia: referencia.to_string(),
                motivo: e.to_string(),
            }),
        }
    }
}

/// Cofre que existe para **dizer que não existe**. Usado quando o sistema não oferece cofre (build
/// sem backend, container sem Secret Service): a suíte avisa e recusa, em vez de degradar para texto.
#[allow(dead_code)] // fallback explícito para ambiente sem cofre; exercitado nos testes
pub struct CofreIndisponivel {
    pub motivo: String,
}

#[allow(dead_code)]
impl CofreIndisponivel {
    pub fn novo(motivo: impl Into<String>) -> Self {
        Self { motivo: motivo.into() }
    }

    fn recusa(&self, referencia: &str) -> ErroCofre {
        ErroCofre::FalhaDoSistema {
            referencia: referencia.to_string(),
            motivo: self.motivo.clone(),
        }
    }
}

impl Cofre for CofreIndisponivel {
    fn guardar(&self, referencia: &str, _segredo: &Segredo) -> Result<(), ErroCofre> {
        exigir_referencia(referencia)?;
        Err(self.recusa(referencia))
    }

    fn ler(&self, referencia: &str) -> Result<Segredo, ErroCofre> {
        exigir_referencia(referencia)?;
        Err(self.recusa(referencia))
    }

    fn existe(&self, referencia: &str) -> bool {
        let _ = referencia;
        false
    }

    fn remover(&self, referencia: &str) -> Result<(), ErroCofre> {
        exigir_referencia(referencia)?;
        Err(self.recusa(referencia))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Teste que roda em qualquer máquina: o cofre indisponível **recusa** — nunca finge que
    /// guardou, nunca escreve arquivo.
    #[test]
    fn cofre_indisponivel_recusa_nada_de_fallback() {
        let c = CofreIndisponivel::novo("Secret Service ausente neste ambiente");
        let e = c.guardar("cofre://lab-1", &Segredo::novo("x")).expect_err("tem de recusar");
        assert!(matches!(e, ErroCofre::FalhaDoSistema { .. }));
        assert!(e.to_string().contains("Secret Service ausente"));
        assert!(!c.existe("cofre://lab-1"));
        assert!(matches!(c.ler("cofre://lab-1"), Err(ErroCofre::FalhaDoSistema { .. })));
    }

    #[test]
    fn referencia_invalida_e_recusada_antes_de_falar_do_sistema() {
        let c = CofreIndisponivel::novo("sem cofre");
        assert!(matches!(
            c.ler("lab-1"),
            Err(ErroCofre::ReferenciaInvalida { .. })
        ));
    }
}
