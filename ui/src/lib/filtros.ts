//! Lógica dos filtros da tela da PON — pura, sem Svelte e sem DOM.
//!
//! Saiu do `+page.svelte` por um motivo prático: dentro do componente não havia como provar
//! semântica (ausência de hora não é "agora", estado vem do NE em caixa alta, etc.). Aqui ela é
//! testável por `scripts/filtros.test.mjs`, que roda no `npm run build` — local e no CI.

import type { Onu } from './ipc';

/** Filtros que a Fase 1 consegue responder com dado de contrato. */
export type FiltroPon = 'todas' | 'com_queda' | 'online' | 'offline' | 'los' | 'sem_hora';

export const FILTROS_PON: { id: FiltroPon; rotulo: string; dica: string }[] = [
	{
		id: 'todas',
		rotulo: 'todas',
		dica: 'todas as ONUs autorizadas nesta PON — é o ONT (Provisioned) do AMS'
	},
	{
		id: 'com_queda',
		rotulo: 'com queda',
		dica: 'ONUs com hora de queda informada: é a base da análise de massiva'
	},
	{ id: 'online', rotulo: 'online', dica: 'estado devolvido pelo NE: online' },
	{ id: 'offline', rotulo: 'offline', dica: 'estado devolvido pelo NE: offline' },
	{ id: 'los', rotulo: 'LOS', dica: 'estado devolvido pelo NE: LOS (perda de sinal)' },
	{
		id: 'sem_hora',
		rotulo: 'sem hora',
		dica: 'o NE não informou hora de queda — estas ONUs ficam fora da análise de massiva'
	}
];

/** Pastas do AMS que a Fase 1 NÃO consegue filtrar, com o motivo em uma linha. */
export const FILTROS_BLOQUEADOS: { rotulo: string; motivo: string }[] = [
	{
		rotulo: 'novas',
		motivo:
			'ONT (New) = ONU descoberta e ainda não autorizada (autofind). Não existe no contrato da Fase 1 — a suíte só lista ONU já autorizada.'
	},
	{
		rotulo: 'multicast',
		motivo: 'Multicast Sources depende do serviço da ONU (list_services) — Fase 2.'
	},
	{
		rotulo: 'alarmes',
		motivo:
			'o alarme do NE não traz o caminho da PON no contrato atual (objeto é texto livre, ex.: "0/1/13"). Filtrar por prefixo seria adivinhar — Fase 2, com o objeto estruturado.'
	},
	{ rotulo: 'TCA', motivo: 'TCA é limiar de desempenho (get_performance) — Fase 2.' },
	{ rotulo: 'PM', motivo: 'PM é medição de desempenho (get_performance) — Fase 2.' }
];

/** Estado como o NE manda, comparável: `LOS`, `Los` e ` los ` são o mesmo estado. */
export function estadoNormalizado(o: Onu): string {
	return (o.estado ?? '').trim().toLowerCase();
}

export function aplicarFiltroPon(lista: readonly Onu[], f: FiltroPon): Onu[] {
	switch (f) {
		case 'com_queda':
			return lista.filter((o) => !!o.ultima_queda);
		case 'sem_hora':
			return lista.filter((o) => !o.ultima_queda);
		case 'online':
			return lista.filter((o) => estadoNormalizado(o) === 'online');
		case 'offline':
			return lista.filter((o) => estadoNormalizado(o) === 'offline');
		case 'los':
			return lista.filter((o) => estadoNormalizado(o) === 'los');
		case 'todas':
			return [...lista];
		default:
			// Filtro desconhecido não pode esconder ONU: devolver tudo é a falha segura.
			return [...lista];
	}
}

/**
 * Queda mais recente primeiro. Quem não tem hora vai para o fim — ausência de hora **não** é
 * "agora", é "o NE não informou". Empate sem hora desempata por SN, para a ordem ser estável.
 */
export function ordenarPorQueda(lista: readonly Onu[]): Onu[] {
	return [...lista].sort((a, b) => {
		if (!a.ultima_queda && !b.ultima_queda) return a.sn.localeCompare(b.sn);
		if (!a.ultima_queda) return 1;
		if (!b.ultima_queda) return -1;
		return b.ultima_queda.localeCompare(a.ultima_queda);
	});
}

export function contar(lista: readonly Onu[], f: FiltroPon): number {
	return aplicarFiltroPon(lista, f).length;
}

export function rotuloDoFiltro(f: FiltroPon): string {
	return FILTROS_PON.find((x) => x.id === f)?.rotulo ?? 'todas';
}
