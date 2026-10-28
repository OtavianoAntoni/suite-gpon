//! Análise de quedas: **agrupar ONUs que caíram na mesma janela de tempo**.
//!
//! É o caso de uso âncora da §7.4: o operador olhava ONU por ONU para descobrir se vários clientes
//! da mesma PON caíram na mesma hora. Aqui isso vira uma função pura sobre a lista que já existe —
//! **nenhuma operação nova no NE** e nenhuma dependência externa (o crate é std-only de propósito).
//!
//! Limite honesto: se o NE não devolver a hora da queda (`ultima_queda` — em C a evidência é `P`),
//! a lista de clusters volta **vazia** e o chamador tem de dizer isso na tela — nunca inventar
//! horário. A hora do **retorno** (`ultimo_up`) não é usada para agrupar: quem voltou junto não
//! necessariamente caiu junto, e agrupar por ela produziria cluster falso.

use crate::result::Onu;

/// Quanto tempo separa duas quedas que ainda contam como "o mesmo evento".
pub const JANELA_PADRAO_MIN: i64 = 5;

/// Uma ONU que caiu, com o instante já convertido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queda {
    /// Id da ONU: é o que permite a tela abrir o cliente a partir da linha do cluster
    /// (o SN identifica, mas o id é a chave que o contrato usa em `get_onu`).
    pub id: String,
    pub sn: String,
    pub nome: String,
    /// Instante da **queda** em segundos desde a época (UTC).
    pub quando: i64,
    /// O texto original do NE, preservado para a tela (não reformatamos o que veio de lá).
    pub quando_texto: String,
    /// Hora do retorno, como veio do NE (eixo "voltou?" da mesma linha da tabela).
    pub ultimo_up: Option<String>,
    /// Motivo da queda no vocabulário do NE ("LOS", "DyingGasp").
    pub motivo: Option<String>,
}

/// Grupo de quedas próximas no tempo — o "suspeita de massiva".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cluster {
    /// Mais antiga do grupo.
    pub inicio: i64,
    /// Mais recente do grupo.
    pub fim: i64,
    /// Ordenadas da mais recente para a mais antiga.
    pub quedas: Vec<Queda>,
}

impl Cluster {
    pub fn tamanho(&self) -> usize {
        self.quedas.len()
    }

    /// Duração do evento em segundos (0 quando é uma ONU só).
    pub fn duracao_s(&self) -> i64 {
        self.fim - self.inicio
    }
}

/// Converte `YYYY-MM-DDTHH:MM:SS[.fração][Z|±HH:MM]` em segundos desde a época.
///
/// Feito à mão para manter o crate sem dependências. Aceita o que OLT manda na prática; devolve
/// `None` em qualquer coisa fora do formato — e o chamador trata `None` como "não sei a hora",
/// jamais como "agora".
pub fn para_epoch(texto: &str) -> Option<i64> {
    let b = texto.trim().as_bytes();
    if b.len() < 19 {
        return None;
    }
    let num = |de: usize, ate: usize| -> Option<i64> {
        let fatia = b.get(de..ate)?;
        if !fatia.iter().all(u8::is_ascii_digit) {
            return None;
        }
        std::str::from_utf8(fatia).ok()?.parse().ok()
    };
    if b[4] != b'-' || b[7] != b'-' || (b[10] != b'T' && b[10] != b' ') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let (ano, mes, dia) = (num(0, 4)?, num(5, 7)?, num(8, 10)?);
    let (hora, min, seg) = (num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&mes) || !(1..=31).contains(&dia) || hora > 23 || min > 59 || seg > 60 {
        return None;
    }

    // Deslocamento: `Z`, `+HH:MM`, `-HH:MM` ou nada (assume UTC).
    let mut pos = 19;
    while b.get(pos) == Some(&b'.') || b.get(pos).is_some_and(u8::is_ascii_digit) {
        pos += 1;
    }
    let deslocamento_s = match b.get(pos) {
        None => 0,
        Some(b'Z') | Some(b'z') => 0,
        Some(sinal @ (b'+' | b'-')) => {
            let h = num(pos + 1, pos + 3)?;
            let m = num(pos + 4, pos + 6)?;
            let total = h * 3600 + m * 60;
            if *sinal == b'-' { -total } else { total }
        }
        Some(_) => return None,
    };

    Some(dias_desde_epoca(ano, mes, dia) * 86_400 + hora * 3600 + min * 60 + seg - deslocamento_s)
}

/// Dias civis desde 1970-01-01 (algoritmo de Howard Hinnant, sem tabela e sem depender do SO).
fn dias_desde_epoca(ano: i64, mes: i64, dia: i64) -> i64 {
    let y = if mes <= 2 { ano - 1 } else { ano };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let ano_na_era = y - era * 400;
    let mes_deslocado = (mes + 9) % 12;
    let dia_do_ano = (153 * mes_deslocado + 2) / 5 + dia - 1;
    let dia_na_era = ano_na_era * 365 + ano_na_era / 4 - ano_na_era / 100 + dia_do_ano;
    era * 146_097 + dia_na_era - 719_468
}

/// Agrupa as quedas em janelas de `janela_min` minutos.
///
/// Regras (todas testadas):
/// - ONU **sem** `ultima_queda` ou com texto ilegível é ignorada — não vira "queda agora";
/// - ordena da mais recente para a mais antiga;
/// - duas quedas consecutivas ficam no mesmo grupo quando a distância entre elas é **≤ janela**
///   (inclusive: 5 min exatos é a mesma janela, não duas);
/// - `janela_min <= 0` é tratado como 1 minuto (nunca "tudo junto");
/// - devolve `[]` quando nenhuma ONU tem hora de queda.
pub fn agrupar_massiva(onus: &[Onu], janela_min: i64) -> Vec<Cluster> {
    let janela_s = janela_min.max(1) * 60;

    let mut quedas: Vec<Queda> = onus
        .iter()
        .filter_map(|o| {
            let texto = o.ultima_queda.as_deref()?;
            let quando = para_epoch(texto)?;
            Some(Queda {
                id: o.id.as_str().to_string(),
                sn: o.sn.clone(),
                nome: o.nome.clone(),
                quando,
                quando_texto: texto.to_string(),
                ultimo_up: o.ultimo_up.clone(),
                motivo: o.motivo_queda.clone(),
            })
        })
        .collect();

    if quedas.is_empty() {
        return Vec::new();
    }

    // Mais recente primeiro; desempate por SN para a saída ser determinística.
    quedas.sort_by(|a, b| b.quando.cmp(&a.quando).then_with(|| a.sn.cmp(&b.sn)));

    let mut clusters: Vec<Cluster> = Vec::new();
    let mut atual: Vec<Queda> = vec![quedas[0].clone()];
    for queda in quedas.into_iter().skip(1) {
        let anterior = atual.last().expect("atual nunca está vazia").quando;
        if anterior - queda.quando <= janela_s {
            atual.push(queda);
        } else {
            let mut grupo = Cluster {
                inicio: atual.last().expect("não vazia").quando,
                fim: atual[0].quando,
                quedas: std::mem::take(&mut atual),
            };
            grupo.quedas.shrink_to_fit();
            clusters.push(grupo);
            atual = vec![queda];
        }
    }
    clusters.push(Cluster {
        inicio: atual.last().expect("não vazia").quando,
        fim: atual[0].quando,
        quedas: atual,
    });
    clusters
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::cmd::{OltId, OnuId, PonRef};
    use crate::result::OnuEstado;

    fn onu(sn: &str, ultimo: Option<&str>) -> Onu {
        Onu {
            id: OnuId::novo(&format!("id-{sn}")),
            olt: OltId::novo("olt-1"),
            pon: PonRef::new(1, 1, 1, 1),
            sn: sn.into(),
            mac: None,
            modelo_hw: "HG8245H".into(),
            modelo_sw: "V5".into(),
            vendor: "Huawei".into(),
            nome: format!("cliente-{sn}"),
            descricao: String::new(),
            estado: OnuEstado::Offline,
            distancia_m: None,
            ultima_queda: ultimo.map(str::to_string),
            ultimo_up: None,
            motivo_queda: None,
        }
    }

    #[test]
    fn epoch_aceita_os_formatos_que_a_olt_manda() {
        assert_eq!(para_epoch("1970-01-01T00:00:00Z"), Some(0));
        // 2026-10-17T14:02:00-03:00 = 17:02 UTC
        let base = para_epoch("2026-10-17T17:02:00Z").unwrap();
        assert_eq!(para_epoch("2026-10-17T14:02:00-03:00"), Some(base));
        assert_eq!(para_epoch("2026-10-17T14:02:00.123-03:00"), Some(base));
        assert_eq!(para_epoch("2026-10-17 14:02:00-03:00"), Some(base));
        assert_eq!(para_epoch("2026-10-17T11:02:00-06:00"), Some(base));
    }

    #[test]
    fn epoch_recusa_texto_que_nao_e_data() {
        for ruim in ["", "agora", "2026-13-01T00:00:00Z", "2026-10-32T00:00:00Z", "17/10/2026 14:02"] {
            assert_eq!(para_epoch(ruim), None, "deveria recusar {ruim:?}");
        }
    }

    #[test]
    fn tres_quedas_proximas_e_uma_isolada_formam_dois_grupos() {
        let onus = vec![
            onu("A", Some("2026-10-17T14:02:00-03:00")),
            onu("B", Some("2026-10-17T14:04:00-03:00")),
            onu("C", Some("2026-10-17T14:06:00-03:00")),
            onu("D", Some("2026-10-17T09:00:00-03:00")),
        ];
        let grupos = agrupar_massiva(&onus, JANELA_PADRAO_MIN);
        assert_eq!(grupos.len(), 2);
        assert_eq!(grupos[0].tamanho(), 3, "14:02→14:06 dentro de ±5 min");
        assert_eq!(grupos[0].quedas[0].sn, "C", "mais recente primeiro");
        assert_eq!(grupos[0].duracao_s(), 4 * 60);
        assert_eq!(grupos[1].tamanho(), 1);
        assert_eq!(grupos[1].quedas[0].sn, "D");
    }

    #[test]
    fn distancia_exatamente_igual_a_janela_fica_no_mesmo_grupo() {
        let onus = vec![
            onu("A", Some("2026-10-17T14:00:00-03:00")),
            onu("B", Some("2026-10-17T14:05:00-03:00")),
        ];
        assert_eq!(agrupar_massiva(&onus, 5).len(), 1, "5 min exatos = mesma janela");
        assert_eq!(agrupar_massiva(&onus, 4).len(), 2, "4 min = grupos separados");
    }

    #[test]
    fn onu_sem_hora_de_queda_nao_entra_nem_vira_agora() {
        let onus = vec![
            onu("A", Some("2026-10-17T14:02:00-03:00")),
            onu("B", None),
            onu("C", Some("registro ilegível")),
        ];
        let grupos = agrupar_massiva(&onus, 5);
        assert_eq!(grupos.len(), 1);
        assert_eq!(grupos[0].tamanho(), 1, "só a ONU com hora válida");
        assert_eq!(grupos[0].quedas[0].sn, "A");
    }

    #[test]
    fn nenhuma_onu_com_hora_devolve_lista_vazia() {
        let onus = vec![onu("A", None), onu("B", None)];
        assert!(agrupar_massiva(&onus, 5).is_empty(), "sem hora => sem clusters, nunca chute");
        assert!(agrupar_massiva(&[], 5).is_empty());
    }

    #[test]
    fn janela_zero_nao_junta_tudo() {
        let onus = vec![
            onu("A", Some("2026-10-17T14:00:00-03:00")),
            onu("B", Some("2026-10-17T14:30:00-03:00")),
            onu("C", Some("2026-10-17T15:00:00-03:00")),
        ];
        assert_eq!(agrupar_massiva(&onus, 0).len(), 3, "janela 0 vira 1 min, não infinito");
    }

    #[test]
    fn mesmo_instante_agrupa_e_ordena_por_sn() {
        let onus = vec![
            onu("Z", Some("2026-10-17T14:02:00-03:00")),
            onu("A", Some("2026-10-17T14:02:00-03:00")),
        ];
        let g = agrupar_massiva(&onus, 5);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].quedas.iter().map(|q| q.sn.as_str()).collect::<Vec<_>>(), vec!["A", "Z"]);
    }

    #[test]
    fn agrupa_pela_queda_e_nao_pelo_retorno() {
        // A caiu às 09:00 e voltou às 14:59; B caiu às 14:58 e não voltou.
        // Se a análise usasse o retorno, A entraria no cluster de 14:5x — cluster falso.
        let mut a = onu("A", Some("2026-10-17T09:00:00-03:00"));
        a.ultimo_up = Some("2026-10-17T14:59:00-03:00".to_string());
        let b = onu("B", Some("2026-10-17T14:58:00-03:00"));

        let grupos = agrupar_massiva(&[a, b], JANELA_PADRAO_MIN);
        assert_eq!(grupos.len(), 2, "quedas a 6h de distância não são a mesma janela");
        assert_eq!(grupos[0].quedas[0].sn, "B", "a queda mais recente primeiro");
        assert_eq!(grupos[1].quedas[0].sn, "A");
        assert_eq!(
            grupos[1].quedas[0].ultimo_up.as_deref(),
            Some("2026-10-17T14:59:00-03:00"),
            "o retorno acompanha a linha sem influenciar o agrupamento"
        );
    }

    #[test]
    fn motivo_e_id_atravessam_para_a_linha_da_tabela() {
        let mut a = onu("A", Some("2026-10-17T14:02:00-03:00"));
        a.motivo_queda = Some("DyingGasp".into());
        let b = onu("B", Some("2026-10-17T14:03:00-03:00"));

        let grupos = agrupar_massiva(&[a, b], JANELA_PADRAO_MIN);
        assert_eq!(grupos.len(), 1);
        let com_motivo = grupos[0].quedas.iter().find(|q| q.sn == "A").expect("A no cluster");
        assert_eq!(com_motivo.motivo.as_deref(), Some("DyingGasp"));
        // O id é o que a tela usa para abrir o cliente ao clicar na linha.
        assert_eq!(com_motivo.id, "id-A");
        let sem_motivo = grupos[0].quedas.iter().find(|q| q.sn == "B").expect("B no cluster");
        assert_eq!(sem_motivo.motivo, None, "sem motivo informado não vira texto inventado");
    }

    #[test]
    fn volume_de_uma_pon_densa_agrupa_certo() {
        // 128 ONUs caindo em 3 eventos distintos ao longo do dia.
        let mut onus = Vec::new();
        for i in 0..60u32 {
            onus.push(onu(&format!("A{i:02}"), Some(&format!("2026-10-17T14:{:02}:00-03:00", i % 4))));
        }
        for i in 0..40u32 {
            onus.push(onu(&format!("B{i:02}"), Some(&format!("2026-10-17T18:{:02}:00-03:00", i % 3))));
        }
        for i in 0..28u32 {
            onus.push(onu(&format!("C{i:02}"), Some("2026-10-17T23:59:00-03:00")));
        }
        let g = agrupar_massiva(&onus, 5);
        assert_eq!(g.len(), 3, "três eventos separados por horas");
        assert_eq!(g.iter().map(Cluster::tamanho).sum::<usize>(), 128);
        assert_eq!(g[0].tamanho(), 28, "23:59 é o mais recente");
    }
}
