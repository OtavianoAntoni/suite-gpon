/**
 * Modo demonstração — **só** quando a UI roda fora do Tauri (navegador, teste de layout).
 *
 * É um espelho mínimo das fixtures do `mock-ne-huawei`, com o mesmo formato de campo. A tela
 * anuncia "modo demonstração" no topo: dado de demo nunca se passa por dado de OLT.
 */
import type {
	Alarme,
	AuditoriaItem,
	Busca,
	Cluster,
	Confirmacao,
	Fonte,
	Massiva,
	Olt,
	Onu,
	PaginaOnus,
	Pon,
	Ponte,
	Previa
} from './ipc';

const FONTE: Fonte = {
	id: 'demo-lab',
	rotulo: 'demo-lab',
	perfil: 'ma5800',
	modelo: 'MA5800-X7',
	host: '10.0.0.10',
	protocolo: 'ssh',
	modulo: 'C'
};

const OLT: Olt = {
	id: 'olt-ma5800-1',
	modelo: 'MA5800-X7',
	vendor: 'Huawei',
	ip: '10.0.0.11',
	sw_version: 'MA5800V100R019C10SPC300',
	slots: 17,
	uptime_dias: 15,
	temperatura_c: 43
};

const PONS: Pon[] = [
	{ olt: OLT.id, slot: 0, porta: 1, rotulo: '0/1', tecnologia: 'GPON', admin_up: true, split_ratio: 128, onus: 4 },
	{ olt: OLT.id, slot: 0, porta: 2, rotulo: '0/2', tecnologia: 'GPON', admin_up: true, split_ratio: 128, onus: 0 }
];

function onu(i: number, nome: string, estado: string, ultimo: string | null): Onu {
	return {
		id: `onu-${i}`,
		olt: OLT.id,
		slot: 0,
		porta: 1,
		sn: `48575443A1B2C3D${i}`,
		nome,
		descricao: '',
		estado,
		modelo_hw: 'HG8245H',
		modelo_sw: 'V5R019C10S270',
		vendor: 'Huawei',
		distancia_m: 1200 + i * 10,
		ultimo_registro: ultimo
	};
}

const ONUS: Onu[] = [
	onu(1, 'cliente-0001', 'online', '2026-10-17T14:02:10-03:00'),
	onu(2, 'cliente-0002', 'offline', '2026-10-17T14:04:00-03:00'),
	onu(3, 'cliente-0003', 'offline', '2026-10-17T14:06:30-03:00'),
	onu(4, 'cliente-0004', 'online', '2026-10-17T09:15:00-03:00')
];

const JANELA_S = (min: number) => min * 60;
const seg = (texto: string) => Math.floor(new Date(texto).getTime() / 1000);

function clusters(min: number): Cluster[] {
	const grupos: Cluster[] = [];
	const comHora = ONUS.filter((o) => o.ultimo_registro).sort(
		(a, b) => seg(b.ultimo_registro!) - seg(a.ultimo_registro!)
	);
	let atual: Onu[] = [];
	const fechar = () => {
		if (!atual.length) return;
		grupos.push({
			inicio: atual[atual.length - 1].ultimo_registro!,
			fim: atual[0].ultimo_registro!,
			tamanho: atual.length,
			duracao_min: Math.round((seg(atual[0].ultimo_registro!) - seg(atual[atual.length - 1].ultimo_registro!)) / 60),
			quedas: atual.map((o) => ({ sn: o.sn, nome: o.nome, quando: o.ultimo_registro! }))
		});
		atual = [];
	};
	for (const o of comHora) {
		const anterior = atual.length ? atual[atual.length - 1] : null;
		if (anterior && seg(anterior.ultimo_registro!) - seg(o.ultimo_registro!) <= JANELA_S(min)) {
			atual.push(o);
		} else {
			fechar();
			atual.push(o);
		}
	}
	fechar();
	return grupos;
}

export const ponteDemo: Ponte = {
	async fontes() {
		return [FONTE];
	},
	async olts() {
		return [OLT];
	},
	async pons() {
		return PONS;
	},
	async onus(_fonte, _olt, pon): Promise<PaginaOnus> {
		const itens = pon ? ONUS.filter((o) => o.slot === pon.slot && o.porta === pon.porta) : ONUS;
		const sem = itens.filter((o) => !o.ultimo_registro).length;
		return {
			total: itens.length,
			itens,
			aviso: sem ? `${sem} ONU(s) sem hora de registro informada pelo NE` : null,
			modulo: 'C'
		};
	},
	async buscar(_fonte, criterio, valor): Promise<Busca> {
		if (criterio === 'vlan') {
			throw new Error('indisponível nesta fase: busca por VLAN do cliente (depende do serviço da ONU — Fase 2)');
		}
		const alvo = valor.trim().toLowerCase();
		const itens = ONUS.filter((o) =>
			criterio === 'alias'
				? o.nome.toLowerCase().includes(alvo)
				: o.sn.toLowerCase() === alvo
		);
		if (!itens.length) throw new Error(`não encontrado: ONU com ${criterio.toUpperCase()}`);
		return {
			criterio,
			valor,
			itens,
			aviso: itens.length > 1 ? `ALIAS não é único: ${itens.length} ONUs atendem a este texto` : null,
			modulo: 'C'
		};
	},
	async alarmes(): Promise<Alarme[]> {
		return [
			{
				id: 'alarme-1',
				severidade: 'critical',
				ne: OLT.id,
				objeto: '0/1',
				tipo: 'LOS no ONT',
				primeira: '2026-10-17T14:02:00-03:00',
				ultima: '2026-10-17T14:07:00-03:00',
				ack: false
			}
		];
	},
	// A demonstração **não finge** escrita: mostra a prévia e diz que nada foi enviado.
	async previaEscrita(pedido): Promise<Previa> {
		const alvo = pedido.nome ? `${pedido.sn} — ${pedido.nome} (${pedido.onu})` : `${pedido.sn} (${pedido.onu})`;
		const resumo =
			pedido.tipo === 'reset'
				? `reiniciar a ONU ${alvo} na PON ${pedido.slot}/${pedido.porta}`
				: `mudar a descrição de ${alvo}: "${pedido.atual}" → "${pedido.novo}"`;
		return {
			token: 'demo0000demo0000',
			token_curto: 'demo0000',
			validade_s: 120,
			operacao: pedido.tipo === 'reset' ? 'reset_onu' : 'set_onu_description',
			alvo: pedido.onu,
			resumo
		};
	},
	async confirmarEscrita(): Promise<Confirmacao> {
		return {
			ok: false,
			mensagem: '(demonstração) nenhuma escrita foi enviada a um NE',
			id_operacao: null,
			operacao: 'demo',
			aviso_auditoria: null
		};
	},
	async auditoria(): Promise<AuditoriaItem[]> {
		return [];
	},
	async massiva(_fonte, _olt, _pon, janelaMin): Promise<Massiva> {
		const c = clusters(janelaMin);
		const comHora = c.reduce((n, g) => n + g.tamanho, 0);
		return {
			janela_min: janelaMin,
			com_hora: comHora,
			sem_hora: ONUS.length - comHora,
			maior_cluster: c.reduce((n, g) => Math.max(n, g.tamanho), 0),
			clusters: c,
			modulo: 'C'
		};
	}
};
