/**
 * Ponte única UI ↔ núcleo Rust (`invoke`).
 *
 * Regra: a UI **não** inventa dado. Todo dado canônico vem do núcleo (`gpon-domain`), e fora do
 * Tauri as chamadas devolvem `null` — a tela mostra "fora do Tauri" em vez de mentir um resultado.
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
