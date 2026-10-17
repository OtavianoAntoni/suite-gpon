#!/usr/bin/env node
/**
 * Lint de UI flat (SPEC §8): a suíte NÃO tem animação.
 *
 * Roda antes do `vite build` e FALHA o build se encontrar qualquer coisa que anime:
 * a decisão é "transições instantâneas", e a única forma de garantir isso ao longo do tempo
 * é o build quebrar — componente do Bits UI expõe `data-state`, e animar é opt-in por CSS.
 */
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { extname, join } from 'node:path';

const RAIZES = ['src'];
const EXTS = new Set(['.svelte', '.css', '.js', '.ts', '.html']);

const REGRAS = [
	[/\btransition\s*:/, 'propriedade/diretiva `transition`'],
	[/\btransition-(property|duration|delay|timing-function)\b/, '`transition-*`'],
	[/\banimation\s*:/, 'propriedade CSS `animation`'],
	[/\banimation-(name|duration|delay|iteration-count|timing-function|fill-mode|play-state)\b/, '`animation-*`'],
	[/@keyframes/, '`@keyframes`'],
	[/\bwill-change\s*:/, '`will-change`'],
	[/\banimate-(in|out)\b/, 'diretiva `animate:` do Svelte'],
];

// Diretivas de entrada/saída do Svelte (`in:fade`, `out:fly`, `both:`), só valem em .svelte.
const REGRAS_SVELTE = [
	[/(^|\s)(in|out|both)\s*:\s*[a-zA-Z]/, 'diretiva `in:`/`out:`/`both:` do Svelte']
];

function arquivos(dir) {
	const achados = [];
	for (const nome of readdirSync(dir)) {
		const p = join(dir, nome);
		if (statSync(p).isDirectory()) achados.push(...arquivos(p));
		else if (EXTS.has(extname(p))) achados.push(p);
	}
	return achados;
}

const problemas = [];
let examinados = 0;

for (const raiz of RAIZES) {
	let lista;
	try {
		lista = arquivos(raiz);
	} catch {
		continue;
	}
	for (const arquivo of lista) {
		examinados++;
		const linhas = readFileSync(arquivo, 'utf8').split('\n');
		linhas.forEach((linha, i) => {
			const regras = arquivo.endsWith('.svelte') ? [...REGRAS, ...REGRAS_SVELTE] : REGRAS;
			for (const [re, descricao] of regras) {
				if (re.test(linha)) {
					problemas.push(`${arquivo}:${i + 1}: ${descricao} → ${linha.trim().slice(0, 90)}`);
					break;
				}
			}
		});
	}
}

if (problemas.length > 0) {
	console.error(`\nFALHA — ${problemas.length} ocorrência(s) de animação na UI (SPEC §8 proíbe):\n`);
	for (const p of problemas) console.error('  ' + p);
	console.error('\nTroque por separação de borda 1px / mudança instantânea de cor de fundo.\n');
	process.exit(1);
}

console.log(`lint UI-flat: OK — ${examinados} arquivos, nenhuma animação.`);
