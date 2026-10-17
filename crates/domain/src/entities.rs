//! Entidades canônicas — transcrição da SPEC §1.1.
//!
//! A coluna "evidência que a justifica" é o que o recon mediu em disco. Ela vive no código
//! de propósito: é o que responde "de onde você tirou esse campo?" sem reabrir o recon, e é
//! o insumo da matriz de capacidades (§3.1) e do badge de rastreabilidade (§3.4).

/// Uma entidade do modelo canônico.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entity {
    /// Nome canônico usado na UI e no domínio.
    pub name: &'static str,
    /// Campos essenciais (resumo fiel da §1.1).
    pub fields: &'static str,
    /// Evidência de disco que justifica a entidade (arquivo/comando por módulo).
    pub evidence: &'static str,
    /// `true` quando a entidade **não** faz parte do escopo da Fase 1.
    pub out_of_phase1: bool,
}

/// As **18** entidades da §1.1, na ordem do documento.
pub const ENTITIES: [Entity; 18] = [
    Entity {
        name: "Fonte / Alvo de gestão (Source)",
        fields: "id, rótulo, host/ip, protocolo preferido, credencial_ref, capacidades",
        evidence: "AMS amsclient.ini + configuration/config.ini; ZTE deploy-*.xml por NE; U2000 ipusage.cfg; UNM2000 unm2000access.conf",
        out_of_phase1: false,
    },
    Entity {
        name: "OLT / NE",
        fields: "vendor, modelo, serial, ip, sw_version, uptime, slots/telas, temperatura",
        evidence: "ZTE pacotes por modelo (c220, c300, f820, sseries); U2000 CommandCodesTrace.xml; AMS nemodel; D: cm.action.NEViewShelfAction, NePropertyBean, OpticalNePropertyPanel, LineInfo",
        out_of_phase1: false,
    },
    Entity {
        name: "Card / Slot / Placa",
        fields: "ne, slot, tipo, estado, nº portas",
        evidence: "ZTE ADD_CARD_INFO, SYN_CARD_INFO_FROM_NE; U2000 família de tasks de export; D: cm.action.BoardPropertyAction",
        out_of_phase1: false,
    },
    Entity {
        name: "Porta PON",
        fields: "olt, slot/porta, tecnologia (GPON/EPON/XGS), estado, split ratio, nº ONUs",
        evidence: "ZTE show-pon, ADD_XPONONUIF_PROFILE; U2000 \"10G GPON ONU PON Port\"",
        out_of_phase1: false,
    },
    Entity {
        name: "ONU / ONT",
        fields: "sn, mac, modelo hw/sw, vendor, nome, descrição, estado (online/offline/LOS), distância (ranging), último registro",
        evidence: "ZTE GET_ONT_INFO, GET_ONT_LIST, GET_ONT_REGISTER_INFO; U2000 \"Add a GPON ONU\"; D: AuthedOnuInfoHandler, OnuInformationExtraHandle, AbnormalOnuInfoPanel",
        out_of_phase1: false,
    },
    Entity {
        name: "Autorização / Whitelist (AuthEntry)",
        fields: "olt, pon, sn, modo (SN/LOID/password/none), loid, senha_ref, profile, faixa de autorização",
        evidence: "ZTE GET_ONT_AUTH_MODE; U2000 \"Activate a GPON ONU\", \"ONU PnP\"; D: extrahandle.onu.grant.*, UriRegisterStatusAction, OnuAuthWayHandler (⚠ \"authorization number\" é conceito próprio do D, ≠ SN)",
        out_of_phase1: false,
    },
    Entity {
        name: "Perfil / Template",
        fields: "nome, tipo (line/DBA/bandwidth/QinQ/VLAN/VoIP/ONU-type/capability set), versão, corpo",
        evidence: "ZTE ADD_ONUTYPE_PROFILE, ADDXPONONUTEMPLET, AddBandwidthTemplet; U2000 \"Add ONT configure profile\"; D: \"Add ONU Capability Set Template\"",
        out_of_phase1: false,
    },
    Entity {
        name: "Serviço (Service)",
        fields: "onu, porta UNI, VLAN(s), GEM, T-CONT, DBA, QoS",
        evidence: "ZTE CONF_ONU_SERVICE, ADD_GEM_PORT, ADD_TCONT_BW; U2000 \"Add ONU and Service\"",
        out_of_phase1: false,
    },
    Entity {
        name: "VLAN / QinQ",
        fields: "id, domínio, modo tag/untag, tradução",
        evidence: "ZTE ADDEXTENDVLANINFO, ADDONTVLANTAGINFO; U2000 \"Add a User VLAN for a GPON ONT\"",
        out_of_phase1: false,
    },
    Entity {
        name: "Alarme",
        fields: "id, severidade, ne, objeto, tipo, first/last, ack/clear",
        evidence: "AMS AlarmImages*, AlarmCallBackHandler; ZTE an-pon-tl1statistics/alarm; U2000 fm.jar; D event-anm.jar + am.actions.OpenAlmNotifyRulesAction",
        out_of_phase1: false,
    },
    Entity {
        name: "Medição / PM",
        fields: "objeto, contador, período, valor",
        evidence: "ZTE an-a10-*-perf, an-is-gpon-current-perf.csv; U2000 \"Add a performance monitoring object for an ONT\"; AMS PmSortVO/PmTaskVO (VO, não comando)",
        out_of_phase1: false,
    },
    Entity {
        name: "Tarefa / Job",
        fields: "id, alvo(s), tipo (upgrade, backup, batch, cutover, config-sync), progresso, resultado",
        evidence: "ZTE GETONTSOFTWAREUPDATESTATUS, ABORTONTSOFTWAREUPDATE; AMS JobDetailsDialog; D task-anm.jar",
        out_of_phase1: false,
    },
    Entity {
        name: "Imagem / Firmware",
        fields: "vendor, modelo alvo, versão, tamanho, hash",
        evidence: "ZTE GETONTSOFTWARENAME, UPDATEONTSOFTWARE, BATCH_ONT_VERSION; U2000 plugin_MML_SSL_client; AMS UpgradeDialogDriverImpl + gui.tftpserver",
        out_of_phase1: false,
    },
    Entity {
        name: "Backup / Restore",
        fields: "escopo, timestamp, arquivo, versão de sw",
        evidence: "AMS axs-backrest-api.jar, BackupCheckServerSyncNeToolsJob/Action, IBackupFileStoreServerHandler; ZTE backup_restore.properties; U2000 CheckFtprootAndBackupFileNums.txt",
        out_of_phase1: false,
    },
    Entity {
        name: "Topologia / Rota",
        fields: "A-Z, tipo (fibra/cabo), uso, capacidade",
        evidence: "presente nos 4 (mais declarado que usado); D expõe SODN_NORTHINTERFACE.OpticalRouteTypeIce (rota óptica)",
        out_of_phase1: true,
    },
    Entity {
        name: "Usuário / Papel",
        fields: "login, papel, escopo, permissões",
        evidence: "ZTE uca-osf-role*-commandlist.xml; U2000 CheckTelnetUserPwd.sql; AMS container.security.SubjectActions$Add*",
        out_of_phase1: false,
    },
    Entity {
        name: "Execução / Auditoria",
        fields: "comando canônico, alvo, protocolo, módulo que executou, início/fim, resultado, autor",
        evidence: "necessária para rastreabilidade (nenhum legado expõe isso de forma simples); o módulo entra aqui por causa do badge §3.4",
        out_of_phase1: false,
    },
    Entity {
        name: "Credencial (referência)",
        fields: "id, tipo, usuário, ref ao cofre, nunca o segredo",
        evidence: "AMS amsclient.ini (tem senha em texto no legado — mascarada no recon com [REDACTED])",
        out_of_phase1: false,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sao_18_entidades_da_secao_1_1() {
        // Medido na §1.1: 18 linhas de entidade (a lista vai de "Fonte" a "Credencial (referência)").
        assert_eq!(ENTITIES.len(), 18);
    }

    #[test]
    fn nenhuma_entidade_vem_sem_evidencia() {
        for e in ENTITIES.iter() {
            assert!(!e.evidence.trim().is_empty(), "entidade sem evidência: {}", e.name);
            assert!(!e.fields.trim().is_empty(), "entidade sem campos: {}", e.name);
        }
    }

    #[test]
    fn topologia_esta_fora_da_fase_1() {
        let t = ENTITIES.iter().find(|e| e.name.starts_with("Topologia")).unwrap();
        assert!(t.out_of_phase1, "§1.1 declara topologia fora do escopo da Fase 1");
    }

    #[test]
    fn nenhuma_credencial_real_entrou_no_codigo() {
        // §9: segredo nunca em arquivo. O campo é sempre referência ao cofre.
        for e in ENTITIES.iter() {
            let low = e.evidence.to_lowercase();
            assert!(
                !low.contains("password=") && !low.contains("senha=") && !low.contains("pwd="),
                "possível segredo em texto na entidade {}",
                e.name
            );
        }
    }
}
