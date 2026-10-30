// Teste da lógica dos filtros da PON — sem navegador e sem Svelte.
//
// Por que existe: os filtros decidem o que o operador VÊ. Um erro aqui esconde ONU (pior tipo de
// falha em campo: "a PON tem 4 clientes, a tela mostrou 3"). As funções são puras em
// `src/lib/filtros.ts` e este script as importa direto — o Node faz type-stripping de `.ts`
// nativamente (22+ e 26 no CI). Roda no `npm run build`, então local e CI usam a mesma régua.

import assert from 'node:assert/strict';

import {
	aplicarFiltroPon,
	ordenarPorQueda,
	contar,
	estadoNormalizado,
	rotuloDoFiltro,
	FILTROS_PON,
	FILTROS_BLOQUEADOS
} from '../src/lib/filtros.ts';

let assercoes = 0;
function confere(nome, fn) {
	fn();
	assercoes++;
	console.log(`  ok  ${nome}`);
}

// ---------- dados de teste ----------
const base = {
	id: '',
	olt: 'olt-ma5800-1',
	rack: 1,
	subrack: 1,
	slot: 1,
	porta: 1,
	sn: 'SN',
	nome: '',
	descricao: '',
	estado: 'online',
	modelo_hw: 'HG8010H',
	modelo_sw: 'V5',
	vendor: 'Huawei',
	distancia_m: null,
	ultima_queda: null,
	ultimo_up: null,
	motivo_queda: null
};
const onu = (p) => ({ ...base, ...p });

const A = onu({ id: 'a', sn: 'SN-A', estado: 'Online', ultima_queda: '2026-10-16T21:40:00-03:00' });
const B = onu({ id: 'b', sn: 'SN-B', estado: 'LOS', ultima_queda: '2026-10-16T22:10:00-03:00' });
const C = onu({ id: 'c', sn: 'SN-C', estado: 'offline', ultima_queda: null });
const D = onu({ id: 'd', sn: 'SN-D', estado: ' los ', ultima_queda: null });
const E = onu({ id: 'e', sn: 'SN-E', estado: 'provisioning', ultima_queda: '2026-10-16T23:00:00-03:00' });
const lista = [A, B, C, D, E];
const ids = (v) => v.map((o) => o.id).join(',');

// ---------- filtros ----------
confere('todas devolve tudo, na ordem de entrada', () => {
	assert.equal(ids(aplicarFiltroPon(lista, 'todas')), 'a,b,c,d,e');
});

confere("'todas' devolve cópia — a lista carregada pela tela não pode ser mexida", () => {
	const r = aplicarFiltroPon(lista, 'todas');
	assert.notEqual(r, lista);
	r.pop();
	assert.equal(lista.length, 5);
});

confere('com_queda = só quem tem hora de queda', () => {
	assert.equal(ids(aplicarFiltroPon(lista, 'com_queda')), 'a,b,e');
});

confere('sem_hora = só quem o NE não deu hora (ausência não é "agora")', () => {
	assert.equal(ids(aplicarFiltroPon(lista, 'sem_hora')), 'c,d');
});

confere('online normaliza "Online" do NE', () => {
	assert.equal(ids(aplicarFiltroPon(lista, 'online')), 'a');
});

confere('offline normaliza "offline"', () => {
	assert.equal(ids(aplicarFiltroPon(lista, 'offline')), 'c');
});

confere('LOS pega "LOS" e " los " (caixa e espaço não importam)', () => {
	assert.equal(ids(aplicarFiltroPon(lista, 'los')), 'b,d');
});

confere('estado desconhecido não some de "todas" nem invade filtro de estado', () => {
	assert.equal(estadoNormalizado(E), 'provisioning');
	assert.ok(ids(aplicarFiltroPon(lista, 'todas')).includes('e'));
	assert.equal(contar(lista, 'online') + contar(lista, 'offline') + contar(lista, 'los'), 4);
});

confere('com_queda + sem_hora = todas (invariante de contagem)', () => {
	assert.equal(contar(lista, 'com_queda') + contar(lista, 'sem_hora'), lista.length);
});

confere('filtro desconhecido devolve tudo (falha segura: não esconde ONU)', () => {
	assert.equal(ids(aplicarFiltroPon(lista, 'inexistente')), 'a,b,c,d,e');
});

confere('lista vazia devolve vazio em qualquer filtro', () => {
	for (const f of FILTROS_PON.map((x) => x.id)) assert.deepEqual(aplicarFiltroPon([], f), []);
});

// ---------- ordenação ----------
confere('ordem por queda: mais recente primeiro, sem hora no fim por SN', () => {
	assert.equal(ids(ordenarPorQueda(lista)), 'e,b,a,c,d');
});

confere('ordenar não muta a lista original', () => {
	const antes = ids(lista);
	ordenarPorQueda(lista);
	assert.equal(ids(lista), antes);
});

confere('ordem estável entre duas ONUs sem hora', () => {
	const z = onu({ id: 'z', sn: 'SN-Z', estado: 'los' });
	assert.equal(ids(ordenarPorQueda([z, C])), 'c,z');
});

// ---------- catálogo de filtros ----------
confere('seis filtros ativos, com ids únicos', () => {
	assert.equal(FILTROS_PON.length, 6);
	assert.equal(new Set(FILTROS_PON.map((f) => f.id)).size, 6);
	assert.equal(rotuloDoFiltro('los'), 'LOS');
});

confere('quatro pastas do AMS bloqueadas, todas com motivo escrito', () => {
	assert.equal(FILTROS_BLOQUEADOS.length, 4);
	for (const b of FILTROS_BLOQUEADOS) assert.ok(b.motivo.trim().length > 10, b.rotulo);
});

confere("'alarmes' não aparece como bloqueado — alarme de ONU é o chip LOS (decisão do dono, 2026-10-30)", () => {
	assert.ok(!FILTROS_BLOQUEADOS.some((b) => b.rotulo.toLowerCase() === 'alarmes'));
});

confere('bloqueado não colide com filtro ativo (nada de chip clicável e travado ao mesmo tempo)', () => {
	const ativos = FILTROS_PON.map((f) => f.id.toLowerCase());
	for (const b of FILTROS_BLOQUEADOS) assert.ok(!ativos.includes(b.rotulo.toLowerCase()), b.rotulo);
});

console.log(`\nfiltros da PON: ${assercoes} verificações, nenhuma falha.`);
