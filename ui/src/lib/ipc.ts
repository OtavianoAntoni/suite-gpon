/**
 * Ponte única UI ↔ núcleo Rust (`invoke`).
 *
 * Regra: a UI **não** inventa dado. Todo dado canônico vem do núcleo (`gpon-domain` /
 * `gpon-transport`). Fora do Tauri, os comandos de inventário caem em **modo demo** — um conjunto
 * mínimo embutido, sempre anunciado na tela com faixa própria; nada de demo silencioso.
 */
import { invoke } from '@tauri-apps/api/core';

export type Modulo = 'A' | 'B' | 'C' | 'D';

export const GLIFOS_EVIDENCIA = ['E', 'P', '?', '—'] as const;
export type GlifoEvidencia = (typeof GLIFOS_EVIDENCIA)[number];

export interface ModuloInfo {
	tag: Modulo;
	rotulo: string;
}

export interface Operacao {
	n: number;
	id: string;
	assinatura: string;
	precondicoes: string;
	destrutiva: boolean;
	evidencias: Record<Modulo, GlifoEvidencia>;
}

export interface EscopoFase1 {
	modulo: Modulo;
	leitura: string[];
	escrita: string[];
	/** Nível de evidência esperado em C, por operação (SPEC §7.2). */
	esperado_em_c: [string, GlifoEvidencia][];
}

export const dentroDoTauri: boolean =
	typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function chamar<T>(cmd: string, args?: Record<string, unknown>): Promise<T | null> {
	if (!dentroDoTauri) return null;
	try {
		return await invoke<T>(cmd, args);
	} catch (e) {
		console.error(`invoke ${cmd} falhou`, e);
		return null;
	}
}

export const listarModulos = () => chamar<ModuloInfo[]>('modulos');
export const listarOperacoes = () => chamar<Operacao[]>('operacoes_canonicas');
export const escopoFase1 = () => chamar<EscopoFase1>('escopo_fase1');

/** Marca na tela se um módulo pode servir a operação (SPEC §3.1: nunca mostrar botão morto). */
export function serve(g: GlifoEvidencia): boolean {
	return g !== '—';
}

// ---------------------------------------------------------------------------------------------
// Inventário (Fase 1): tudo abaixo passa por `CanonicalCmd` no Rust.
// ---------------------------------------------------------------------------------------------

export type Fonte = {
	id: string;
	rotulo: string;
	perfil: string;
	modelo: string;
	host: string;
	protocolo: string;
	modulo: string;
};

export type Olt = {
	id: string;
	modelo: string;
	vendor: string;
	ip: string;
	sw_version: string;
	slots: number;
	uptime_dias: number;
	temperatura_c: number | null;
};

/** Resposta de `get_olt_info` — o que a lista de OLTs não traz (serial, uptime cru). */
export type OltDetalhe = {
	id: string;
	fonte: string;
	modelo: string;
	vendor: string;
	serial: string;
	ip: string;
	sw_version: string;
	slots: number;
	uptime_dias: number;
	uptime_horas: number;
	temperatura_c: number | null;
	/** `null` = a contagem de PONs não pôde ser lida agora (não é "0 PONs"). */
	pons: number | null;
};

export type Pon = {
	olt: string;
	slot: number;
	porta: number;
	rotulo: string;
	tecnologia: string;
	admin_up: boolean;
	split_ratio: number;
	onus: number;
};

export type Onu = {
	id: string;
	olt: string;
	slot: number;
	porta: number;
	sn: string;
	nome: string;
	descricao: string;
	estado: string;
	modelo_hw: string;
	modelo_sw: string;
	vendor: string;
	distancia_m: number | null;
	ultimo_registro: string | null;
};

export type PaginaOnus = {
	total: number;
	itens: Onu[];
	aviso: string | null;
	modulo: string;
};

export type Busca = {
	criterio: string;
	valor: string;
	itens: Onu[];
	aviso: string | null;
	modulo: string;
};

export type Queda = { sn: string; nome: string; quando: string };

export type Cluster = {
	inicio: string;
	fim: string;
	tamanho: number;
	duracao_min: number;
	quedas: Queda[];
};

export type Massiva = {
	janela_min: number;
	com_hora: number;
	sem_hora: number;
	maior_cluster: number;
	clusters: Cluster[];
	modulo: string;
};

export type Alarme = {
	id: string;
	severidade: string;
	ne: string;
	objeto: string;
	tipo: string;
	primeira: string;
	ultima: string;
	ack: boolean;
};

export type Previa = {
	token: string;
	token_curto: string;
	validade_s: number;
	operacao: string;
	alvo: string;
	resumo: string;
};

export type Confirmacao = {
	ok: boolean;
	mensagem: string;
	id_operacao: string | null;
	operacao: string;
	aviso_auditoria: string | null;
};

export type AuditoriaItem = {
	quando: string;
	operacao: string;
	alvo: string;
	resumo: string;
	resultado: string;
	mensagem: string;
};

export type PedidoEscrita = {
	tipo: 'reset' | 'descricao' | 'autorizar' | 'desautorizar';
	olt: string;
	slot: number;
	porta: number;
	onu: string;
	sn: string;
	nome: string;
	atual: string;
	novo: string;
	/** Só em `autorizar`: modo do contrato (sn | loid | password | none). */
	modo?: string;
	/** Só em `autorizar`: perfil a aplicar (opcional — o NE usa o padrão). */
	perfil?: string;
};

export type Ponte = {
	fontes(): Promise<Fonte[]>;
	olts(fonte: string): Promise<Olt[]>;
	detalheOlt(fonte: string, olt: string): Promise<OltDetalhe>;
	pons(fonte: string, olt: string): Promise<Pon[]>;
	onus(fonte: string, olt: string, pon?: { slot: number; porta: number }, ordem?: string): Promise<PaginaOnus>;
	buscar(fonte: string, criterio: string, valor: string): Promise<Busca>;
	massiva(fonte: string, olt: string, pon: { slot: number; porta: number }, janelaMin: number): Promise<Massiva>;
	alarmes(fonte: string, filtro?: string): Promise<Alarme[]>;
	previaEscrita(pedido: PedidoEscrita): Promise<Previa>;
	confirmarEscrita(token: string): Promise<Confirmacao>;
	auditoria(n?: number): Promise<AuditoriaItem[]>;
};

async function real<T>(cmd: string, args: Record<string, unknown>): Promise<T> {
	return invoke<T>(cmd, args);
}

export const ponte: Ponte = {
	fontes: () => real<Fonte[]>('fontes', {}),
	olts: (fonte) => real<Olt[]>('listar_olts', { fonte }),
	detalheOlt: (fonte, olt) => real<OltDetalhe>('detalhe_olt', { fonte, olt }),
	pons: (fonte, olt) => real<Pon[]>('listar_pons', { fonte, olt }),
	onus: (fonte, olt, pon, ordem = 'sn') =>
		real<PaginaOnus>('listar_onus', {
			fonte,
			olt,
			slot: pon?.slot ?? null,
			porta: pon?.porta ?? null,
			filtro: null,
			ordem
		}),
	buscar: (fonte, criterio, valor) => real<Busca>('buscar_onu', { fonte, criterio, valor }),
	massiva: (fonte, olt, pon, janelaMin) =>
		real<Massiva>('massiva', { fonte, olt, slot: pon.slot, porta: pon.porta, janelaMin }),
	alarmes: (fonte, filtro = undefined) => real<Alarme[]>('listar_alarmes', { fonte, filtro: filtro ?? null }),
	previaEscrita: (pedido) => real<Previa>('previa_escrita', { pedido }),
	confirmarEscrita: (token) => real<Confirmacao>('confirmar_escrita', { token }),
	auditoria: (n = 50) => real<AuditoriaItem[]>('auditoria_recente', { n })
};

/** Ponte usada pela tela: a real dentro do app, a de demonstração no navegador. */
export async function ponteDaTela(): Promise<{ ponte: Ponte; demo: boolean }> {
	if (dentroDoTauri) return { ponte, demo: false };
	const { ponteDemo } = await import('./demo');
	return { ponte: ponteDemo, demo: true };
}

/** `HH:MM` a partir do texto que o NE mandou — sem reinterpretar fuso. */
export function horaCurta(texto: string | null): string {
	if (!texto) return '—';
	const m = texto.match(/[T ](\d{2}:\d{2})/);
	return m ? m[1] : texto;
}

export function csvDasQuedas(clusters: Cluster[]): string {
	const linhas = ['cluster_inicio,cluster_fim,sn,nome,quando'];
	for (const c of clusters) {
		for (const q of c.quedas) {
			linhas.push([c.inicio, c.fim, q.sn, `"${q.nome.replace(/"/g, '""')}"`, q.quando].join(','));
		}
	}
	return linhas.join('\n');
}
