/**
 * Tema claro/escuro — decidido na SPEC §8: persistência via `tauri-plugin-store`
 * (e `localStorage` quando rodando no navegador, fora do Tauri).
 *
 * Detalhe que importa: a troca aplica o atributo no `<html>` e NÃO recarrega a janela.
 */
import { dentroDoTauri } from './ipc';

/** Não dependemos de `$app/environment` (API do Kit mudou na v3): checagem direta do host. */
const browser = typeof window !== 'undefined';

export type Tema = 'claro' | 'escuro';

const CHAVE = 'suite-gpon.tema';
const ARQUIVO = 'config.json';

let atual = $state<Tema>('claro');

export function tema(): Tema {
	return atual;
}

function aplicarNoDocumento(t: Tema) {
	if (!browser) return;
	document.documentElement.dataset.tema = t;
}

async function persistir(t: Tema) {
	if (!browser) return;
	try {
		if (dentroDoTauri) {
			const { load } = await import('@tauri-apps/plugin-store');
			const store = await load(ARQUIVO, { autoSave: true });
			await store.set(CHAVE, t);
		} else {
			localStorage.setItem(CHAVE, t);
		}
	} catch (e) {
		// Persistir é conveniência: se o cofre/arquivo falhar, a UI continua funcionando.
		console.warn('tema: não foi possível persistir', e);
	}
}

export function definirTema(t: Tema) {
	atual = t;
	aplicarNoDocumento(t);
	void persistir(t);
}

export function alternarTema() {
	definirTema(atual === 'claro' ? 'escuro' : 'claro');
}

/** Lê o tema salvo e aplica. Chamado uma vez, no `onMount` do layout. */
export async function carregarTema() {
	if (!browser) return;
	let salvo: string | null = null;
	try {
		if (dentroDoTauri) {
			const { load } = await import('@tauri-apps/plugin-store');
			const store = await load(ARQUIVO, { autoSave: true });
			salvo = (await store.get<string>(CHAVE)) ?? null;
		} else {
			salvo = localStorage.getItem(CHAVE);
		}
	} catch (e) {
		console.warn('tema: não foi possível ler o valor salvo', e);
	}
	definirTema(salvo === 'escuro' ? 'escuro' : 'claro');
}
