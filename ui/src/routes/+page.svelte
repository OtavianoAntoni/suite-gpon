<script lang="ts">
	/**
	 * Casca da suíte — estrutura espelha a U2000 (SPEC §8.1), visual flat (§8).
	 *
	 * Árvore de inventário à esquerda (Fonte > OLT > PON > ONU, carregada sob demanda), painel de
	 * tarefas no rodapé, "Find by..." na barra superior. Todo dado vem do Rust por `ponte`; fora do
	 * Tauri a tela entra em modo demonstração e **avisa**.
	 */
	import { onMount } from 'svelte';
	import { alternarTema, tema } from '#lib/theme.svelte';
	import {
		csvDasQuedas,
		horaCurta,
		listarOperacoes,
		ponteDaTela,
		type Alarme,
		type Busca,
		type Fonte,
		type Massiva,
		type Onu,
		type Operacao,
		type Olt,
		type Pon,
		type Ponte
	} from '#lib/ipc';

	let ponte: Ponte | null = null;
	let demo = $state(false);
	let erro = $state<string | null>(null);

	let fontes = $state<Fonte[]>([]);
	let oltas = $state<Record<string, Olt[]>>({});
	let pons = $state<Record<string, Pon[]>>({});
	let onus = $state<Record<string, Onu[]>>({});
	let aberto = $state<Record<string, boolean>>({});

	let fonteSel = $state<Fonte | null>(null);
	let oltSel = $state<Olt | null>(null);
	let ponSel = $state<Pon | null>(null);
	let onuSel = $state<Onu | null>(null);

	let area = $state<'inventario' | 'alarmes' | 'matriz' | 'f1'>('inventario');
	let carregando = $state(true);

	let janela = $state(5);
	let massiva = $state<Massiva | null>(null);
	let analisando = $state(false);

	let criterio = $state<'sn' | 'alias' | 'vlan'>('sn');
	let termo = $state('');
	let busca = $state<Busca | null>(null);
	let erroBusca = $state<string | null>(null);

	let alarmes = $state<Alarme[]>([]);
	let operacoes = $state<Operacao[]>([]);

	type Tarefa = { hora: string; texto: string; tipo: 'ok' | 'erro' };
	let tarefas = $state<Tarefa[]>([]);
	let painelAberto = $state(true);

	function registrar(texto: string, tipo: 'ok' | 'erro' = 'ok') {
		const hora = new Date().toLocaleTimeString('pt-BR', {
			hour: '2-digit',
			minute: '2-digit',
			second: '2-digit'
		});
		tarefas = [{ hora, texto, tipo }, ...tarefas].slice(0, 20);
	}

	onMount(async () => {
		const { ponte: p, demo: ehDemo } = await ponteDaTela();
		ponte = p;
		demo = ehDemo;
		try {
			fontes = await p.fontes();
			if (fontes.length) {
				await abrirFonte(fontes[0]);
				// Abre o primeiro ramo até a PON: a tela inicial já mostra o que importa (quedas).
				const olts = oltas[fontes[0].id] ?? [];
				if (olts.length) {
					await abrirOlt(olts[0]);
					const lista = pons[olts[0].id] ?? [];
					if (lista.length) await abrirPon(lista[0]);
				}
			}
			operacoes = (await listarOperacoes()) ?? [];
		} catch (e) {
			erro = String(e);
		} finally {
			carregando = false;
		}
	});

	async function abrirFonte(f: Fonte) {
		fonteSel = f;
		aberto[f.id] = true;
		if (oltas[f.id]) return;
		try {
			oltas[f.id] = await ponte!.olts(f.id);
			registrar(`OLTs de ${f.rotulo}: ${oltas[f.id].length}`);
		} catch (e) {
			registrar(`falha ao listar OLTs: ${e}`, 'erro');
		}
	}

	async function abrirOlt(o: Olt) {
		oltSel = o;
		aberto[o.id] = true;
		ponSel = null;
		massiva = null;
		if (pons[o.id]) return;
		try {
			pons[o.id] = await ponte!.pons(fonteSel!.id, o.id);
			registrar(`${o.id}: ${pons[o.id].length} PON(s)`);
		} catch (e) {
			registrar(`falha ao listar PONs: ${e}`, 'erro');
		}
	}

	async function abrirPon(p: Pon) {
		ponSel = p;
		onuSel = null;
		area = 'inventario';
		const chave = `${p.olt}-${p.slot}/${p.porta}`;
		if (!onus[chave]) {
			try {
				const pagina = await ponte!.onus(fonteSel!.id, p.olt, { slot: p.slot, porta: p.porta });
				onus[chave] = pagina.itens;
			} catch (e) {
				registrar(`falha ao listar ONUs: ${e}`, 'erro');
			}
		}
		await analisar();
	}

	async function analisar() {
		if (!ponte || !fonteSel || !ponSel) return;
		analisando = true;
		try {
			massiva = await ponte.massiva(fonteSel.id, ponSel.olt, { slot: ponSel.slot, porta: ponSel.porta }, janela);
			registrar(
				`PON ${ponSel.rotulo}: ${massiva.com_hora} queda(s) em ${massiva.clusters.length} janela(s) de ${janela} min`,
				massiva.maior_cluster > 1 ? 'ok' : 'ok'
			);
		} catch (e) {
			registrar(`análise falhou: ${e}`, 'erro');
			massiva = null;
		} finally {
			analisando = false;
		}
	}

	function verOnu(o: Onu) {
		onuSel = o;
		area = 'inventario';
	}

	async function procurar() {
		if (!ponte || !fonteSel) return;
		erroBusca = null;
		busca = null;
		try {
			busca = await ponte.buscar(fonteSel.id, criterio, termo);
			registrar(`busca por ${criterio}: ${busca.itens.length} resultado(s)`);
		} catch (e) {
			erroBusca = String(e).replace(/^Error:\s*/, '');
			registrar(`busca por ${criterio}: ${erroBusca}`, 'erro');
		}
	}

	async function carregarAlarmes() {
		area = 'alarmes';
		if (!ponte || !fonteSel || alarmes.length) return;
		try {
			alarmes = await ponte.alarmes(fonteSel.id);
			registrar(`alarmes ativos: ${alarmes.length}`);
		} catch (e) {
			registrar(`falha ao listar alarmes: ${e}`, 'erro');
		}
	}

	function exportarCsv() {
		if (!massiva) return;
		const total = massiva.clusters.reduce((n, c) => n + c.tamanho, 0);
		const blob = new Blob([csvDasQuedas(massiva.clusters)], { type: 'text/csv;charset=utf-8' });
		const url = URL.createObjectURL(blob);
		const a = document.createElement('a');
		a.href = url;
		a.download = `massiva-pon-${ponSel?.slot}-${ponSel?.porta}.csv`;
		a.click();
		URL.revokeObjectURL(url);
		registrar(`CSV exportado: ${total} ONU(s)`);
	}

	let maiorIndice = $derived(
		massiva ? massiva.clusters.reduce((m, c, i) => (c.tamanho > massiva!.clusters[m].tamanho ? i : m), 0) : -1
	);

	function atalhos(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			busca = null;
			erroBusca = null;
			termo = '';
		}
	}
</script>

<svelte:window onkeydown={atalhos} />

<div class="app">
	{#if demo}
		<div class="faixa-demo" role="status">
			modo demonstração — a UI está fora do app (navegador). Os números abaixo são de exemplo.
		</div>
	{/if}

	<header class="topo">
		<span class="titulo">Suíte GPON/FTTH</span>

		<form class="find" onsubmit={(e) => (e.preventDefault(), procurar())}>
			<label class="oculto" for="find-criterio">Critério de busca</label>
			<select id="find-criterio" bind:value={criterio}>
				<option value="sn">SN</option>
				<option value="alias">ALIAS</option>
				<option value="vlan">VLAN</option>
			</select>
			<label class="oculto" for="find-termo">Valor</label>
			<input id="find-termo" bind:value={termo} placeholder="Find by…" autocomplete="off" />
			<button type="submit" disabled={!termo.trim()}>buscar</button>
		</form>

		<div class="modulos" aria-label="Origem dos comandos">
			{#each ['A', 'B', 'C', 'D'] as m}
				<span class="modulo" class:ativo={m === 'C'} title={m === 'C' ? 'mock-ne-huawei (Fase 1)' : 'sem fonte configurada'}>
					{m}
				</span>
			{/each}
		</div>

		<button class="tema" onclick={alternarTema} aria-label="Alternar tema claro/escuro">
			{tema() === 'claro' ? 'escuro' : 'claro'}
		</button>
	</header>

	{#if erro}
		<div class="erro" role="alert">núcleo indisponível: {erro}</div>
	{/if}

	<div class="corpo">
		<aside class="lateral">
			<nav class="areas" aria-label="Áreas">
				<button class:ativo={area === 'inventario'} onclick={() => (area = 'inventario')}>Inventário</button>
				<button class:ativo={area === 'alarmes'} onclick={carregarAlarmes}>Alarmes</button>
				<button class:ativo={area === 'matriz'} onclick={() => (area = 'matriz')}>Matriz</button>
				<button class:ativo={area === 'f1'} onclick={() => (area = 'f1')}>F1</button>
			</nav>

			<div class="arvore-caixa">
				<span class="nav-rotulo">inventário</span>
				{#if carregando}
					<p class="vazio">carregando…</p>
				{:else if !fontes.length}
					<p class="vazio">nenhuma fonte configurada</p>
				{/if}

				<ul class="arvore">
					{#each fontes as f (f.id)}
						<li>
							<button class="no n1" class:sel={fonteSel?.id === f.id} onclick={() => abrirFonte(f)} aria-expanded={!!aberto[f.id]}>
								<span class="seta">{aberto[f.id] ? '▾' : '▸'}</span>
								<span class="rotulo">{f.rotulo}</span>
								<span class="meta">{f.perfil}</span>
							</button>
							{#if aberto[f.id]}
								<ul>
									{#each oltas[f.id] ?? [] as o (o.id)}
										<li>
											<button class="no n2" class:sel={oltSel?.id === o.id} onclick={() => abrirOlt(o)} aria-expanded={!!aberto[o.id]}>
												<span class="seta">{aberto[o.id] ? '▾' : '▸'}</span>
												<span class="rotulo">{o.modelo}</span>
												<span class="meta">{o.ip}</span>
											</button>
											{#if aberto[o.id]}
												<ul>
													{#each pons[o.id] ?? [] as p (p.rotulo)}
														<li>
															<button class="no n3" class:sel={ponSel?.rotulo === p.rotulo && ponSel?.olt === p.olt} onclick={() => abrirPon(p)}>
																<span class="seta">·</span>
																<span class="rotulo">PON {p.rotulo}</span>
																<span class="meta">{p.onus} ONU</span>
															</button>
															{#if ponSel?.rotulo === p.rotulo && ponSel?.olt === p.olt}
																<ul>
																	{#each (onus[`${p.olt}-${p.slot}/${p.porta}`] ?? []).slice(0, 8) as u (u.id)}
																		<li>
																			<button class="no n4" class:sel={onuSel?.id === u.id} onclick={() => verOnu(u)}>
																				<span class="seta">·</span>
																				<span class="rotulo">{u.nome || u.sn}</span>
																				<span class="meta">{u.estado}</span>
																			</button>
																		</li>
																	{/each}
																</ul>
															{/if}
														</li>
													{/each}
												</ul>
											{/if}
										</li>
									{/each}
								</ul>
							{/if}
						</li>
					{/each}
				</ul>
			</div>
		</aside>

		<main class="conteudo">
			{#if busca || erroBusca}
				<section class="busca-caixa">
					<header class="linha">
						<strong>busca por {criterio}</strong>
						<span class="badge">via C</span>
						<button class="link" onclick={() => ((busca = null), (erroBusca = null))}>fechar</button>
					</header>
					{#if erroBusca}
						<p class="erro-linha">{erroBusca}</p>
					{:else if busca}
						{#if busca.aviso}<p class="aviso-linha">{busca.aviso}</p>{/if}
						<table class="tabela">
							<thead>
								<tr><th>SN</th><th>nome</th><th>ONU</th><th>estado</th><th>último registro</th></tr>
							</thead>
							<tbody>
								{#each busca.itens as u (u.id)}
									<tr class="clicavel" onclick={() => verOnu(u)}>
										<td class="mono">{u.sn}</td>
										<td>{u.nome}</td>
										<td>{u.slot}/{u.porta}</td>
										<td>{u.estado}</td>
										<td>{horaCurta(u.ultimo_registro)}</td>
									</tr>
								{/each}
							</tbody>
						</table>
					{/if}
				</section>
			{/if}

			{#if area === 'inventario'}
				{#if ponSel}
					<header class="cabecalho">
						<h1>PON {ponSel.rotulo} · {oltSel?.modelo ?? ponSel.olt}</h1>
						<div class="controles">
							<label>
								janela
								<select bind:value={janela} onchange={analisar}>
									{#each [1, 3, 5, 10, 15] as j}<option value={j}>{j} min</option>{/each}
								</select>
							</label>
							<button onclick={analisar} disabled={analisando}>{analisando ? 'analisando…' : 'reanalisar'}</button>
							<button onclick={exportarCsv} disabled={!massiva?.clusters.length}>exportar CSV</button>
						</div>
					</header>

					{#if massiva}
						<div class="resumo">
							<div><span class="num">{massiva.com_hora}</span><span class="rot">ONUs com hora de queda</span></div>
							<div><span class="num">{massiva.sem_hora}</span><span class="rot">sem hora informada</span></div>
							<div><span class="num">{massiva.clusters.length}</span><span class="rot">janelas de {massiva.janela_min} min</span></div>
							<div class:alerta={massiva.maior_cluster > 2}>
								<span class="num">{massiva.maior_cluster}</span><span class="rot">maior agrupamento</span>
							</div>
						</div>

						{#if !massiva.clusters.length}
							<p class="vazio">
								este NE não informou hora de queda para nenhuma ONU desta PON — não há como agrupar.
							</p>
						{/if}

						{#each massiva.clusters as c, i (c.inicio + c.fim)}
							<section class="cluster" class:destaque={i === maiorIndice && c.tamanho > 2}>
								<header class="linha">
									<strong>{c.tamanho} ONU{c.tamanho > 1 ? 's' : ''}</strong>
									<span class="mono">{horaCurta(c.inicio)} → {horaCurta(c.fim)}</span>
									<span class="meta">{c.duracao_min} min de duração</span>
									{#if i === maiorIndice && c.tamanho > 2}
										<span class="selo">possível massiva</span>
									{/if}
								</header>
								<table class="tabela">
									<thead><tr><th>SN</th><th>cliente</th><th>quando</th></tr></thead>
									<tbody>
										{#each c.quedas as q (q.sn)}
											<tr><td class="mono">{q.sn}</td><td>{q.nome}</td><td>{horaCurta(q.quando)}</td></tr>
										{/each}
									</tbody>
								</table>
							</section>
						{/each}
					{:else if !analisando}
						<p class="vazio">sem análise ainda.</p>
					{/if}
				{:else if onuSel}
					<section class="detalhe">
						<header class="cabecalho">
							<h1>{onuSel.nome || onuSel.sn}</h1>
							<span class="badge">via C · E</span>
						</header>
						<dl class="campos">
							<div><dt>SN</dt><dd class="mono">{onuSel.sn}</dd></div>
							<div><dt>PON</dt><dd>{onuSel.slot}/{onuSel.porta}</dd></div>
							<div><dt>estado</dt><dd>{onuSel.estado}</dd></div>
							<div><dt>modelo</dt><dd>{onuSel.modelo_hw} · {onuSel.modelo_sw}</dd></div>
							<div><dt>distância</dt><dd>{onuSel.distancia_m ?? 'não informada'}</dd></div>
							<div><dt>último registro</dt><dd>{onuSel.ultimo_registro ?? 'não informado'}</dd></div>
						</dl>
						<p class="nota">
							nível óptico não está na Fase 1: é o primeiro item da Fase 2 (em C a evidência é <code>P</code>).
						</p>
					</section>
				{:else}
					<p class="vazio">selecione uma PON na árvore para ver as quedas agrupadas.</p>
				{/if}
			{:else if area === 'alarmes'}
				<header class="cabecalho"><h1>Alarmes</h1><span class="badge">via C</span></header>
				{#if !alarmes.length}
					<p class="vazio">nenhum alarme ativo na fonte selecionada.</p>
				{:else}
					<table class="tabela">
						<thead><tr><th>severidade</th><th>NE</th><th>objeto</th><th>tipo</th><th>primeira</th><th>última</th><th>ack</th></tr></thead>
						<tbody>
							{#each alarmes as a (a.id)}
								<tr>
									<td class="sev {a.severidade}">{a.severidade}</td>
									<td>{a.ne}</td><td>{a.objeto}</td><td>{a.tipo}</td>
									<td>{horaCurta(a.primeira)}</td><td>{horaCurta(a.ultima)}</td>
									<td>{a.ack ? 'sim' : 'não'}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				{/if}
			{:else if area === 'matriz'}
				<header class="cabecalho"><h1>Matriz de capacidades</h1><span class="meta">{operacoes.length} operações canônicas</span></header>
				<p class="nota">
					E = comando nomeado em disco · P = caminho plausível · ? = insuficiente · — = nenhuma evidência no cliente
				</p>
				<table class="tabela compacta">
					<thead><tr><th>#</th><th>operação</th><th>A</th><th>B</th><th>C</th><th>D</th></tr></thead>
					<tbody>
						{#each operacoes as op (op.id)}
							<tr>
								<td class="mono">{op.n}</td>
								<td><code>{op.id}</code>{#if op.destrutiva}<span class="marca"> ⚠</span>{/if}</td>
								<td class:celula-ativa={op.evidencias.A !== '—'}>{op.evidencias.A}</td>
								<td class:celula-ativa={op.evidencias.B !== '—'}>{op.evidencias.B}</td>
								<td class:celula-ativa={op.evidencias.C !== '—'}>{op.evidencias.C}</td>
								<td class:celula-ativa={op.evidencias.D !== '—'}>{op.evidencias.D}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			{:else}
				<header class="cabecalho"><h1>F1 — o que esta suíte faz</h1></header>
				<ul class="f1">
					<li><strong>Fase 1</strong> conversa só com o <em>mock</em> em memória: nenhuma OLT real é tocada.</li>
					<li>16 operações canônicas (12 leituras + 4 escritas); o módulo de referência é <code>C</code>.</li>
					<li>Busca por <strong>SN</strong> e <strong>ALIAS</strong> agora; <strong>VLAN</strong> chega na Fase 2.</li>
					<li>Ordenar a PON pela hora da queda é o que permite ver massiva sem clicar em cada cliente.</li>
					<li>Operações destrutivas aparecem com <span class="marca">⚠</span> e pedem confirmação dupla.</li>
				</ul>
			{/if}
		</main>
	</div>

	<footer class="tarefas" class:fechado={!painelAberto}>
		<header class="linha">
			<strong>tarefas</strong>
			<span class="meta">{tarefas.length} evento(s)</span>
			<button class="link" onclick={() => (painelAberto = !painelAberto)}>
				{painelAberto ? 'recolher' : 'expandir'}
			</button>
		</header>
		{#if painelAberto}
			<ul>
				{#each tarefas as t, i (i)}
					<li class:falha={t.tipo === 'erro'}><span class="mono">{t.hora}</span> {t.texto}</li>
				{/each}
				{#if !tarefas.length}<li class="vazio">nenhuma operação nesta sessão.</li>{/if}
			</ul>
		{/if}
	</footer>
</div>

<style>
	.app {
		display: grid;
		grid-template-rows: auto auto 1fr auto;
		height: 100vh;
		background: var(--cor-fundo);
		color: var(--cor-texto);
		font-size: var(--fonte-tamanho);
	}

	.oculto {
		position: absolute;
		width: 1px;
		height: 1px;
		overflow: hidden;
		clip-path: inset(50%);
	}

	.faixa-demo {
		background: #fef3c7;
		color: #7c2d12;
		padding: 4px 12px;
		border-bottom: 1px solid var(--cor-borda);
		font-size: 12px;
	}

	.topo {
		display: flex;
		align-items: center;
		gap: 12px;
		height: 44px;
		padding: 0 12px;
		background: var(--cor-superficie);
		border-bottom: 1px solid var(--cor-borda);
	}

	.titulo {
		font-weight: 600;
		margin-right: 8px;
	}

	.find {
		display: flex;
		gap: 4px;
		flex: 1;
		max-width: 460px;
	}
	.find select,
	.find input,
	.find button {
		height: 28px;
		border: 1px solid var(--cor-borda);
		background: var(--cor-fundo);
		color: var(--cor-texto);
		border-radius: 2px;
		padding: 0 6px;
		font: inherit;
	}
	.find input {
		flex: 1;
	}
	.find button {
		cursor: pointer;
	}
	.find button:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.modulos {
		display: flex;
		gap: 2px;
		margin-left: auto;
	}
	.modulo {
		width: 24px;
		height: 24px;
		display: grid;
		place-items: center;
		border: 1px solid var(--cor-borda);
		border-radius: 2px;
		color: var(--cor-texto-suave);
		font-size: 12px;
	}
	.modulo.ativo {
		background: var(--cor-acento);
		color: #fff;
		border-color: var(--cor-acento);
	}

	.tema {
		height: 24px;
		border: 1px solid var(--cor-borda);
		background: var(--cor-fundo);
		color: var(--cor-texto);
		border-radius: 2px;
		cursor: pointer;
		font: inherit;
		padding: 0 8px;
	}

	.erro {
		background: #fee2e2;
		color: #7f1d1d;
		padding: 6px 12px;
		border-bottom: 1px solid var(--cor-borda);
	}

	.corpo {
		display: grid;
		grid-template-columns: 264px 1fr;
		min-height: 0;
	}

	.lateral {
		border-right: 1px solid var(--cor-borda);
		background: var(--cor-superficie);
		display: flex;
		flex-direction: column;
		min-height: 0;
	}

	.areas {
		display: flex;
		gap: 0;
		border-bottom: 1px solid var(--cor-borda);
	}
	.areas button {
		flex: 1;
		height: 30px;
		border: 0;
		border-right: 1px solid var(--cor-borda);
		background: transparent;
		color: var(--cor-texto-suave);
		cursor: pointer;
		font: inherit;
		font-size: 12px;
	}
	.areas button.ativo {
		color: var(--cor-texto);
		box-shadow: inset 0 -2px 0 var(--cor-acento);
	}

	.nav-rotulo {
		display: block;
		padding: 6px 10px;
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		color: var(--cor-texto-suave);
	}

	.arvore-caixa {
		overflow: auto;
		min-height: 0;
	}

	.arvore,
	.arvore ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.arvore ul {
		padding-left: 10px;
		border-left: 1px solid var(--cor-borda);
		margin-left: 14px;
	}

	.no {
		display: flex;
		align-items: center;
		gap: 6px;
		width: 100%;
		height: var(--altura-linha);
		border: 0;
		background: transparent;
		color: var(--cor-texto);
		cursor: pointer;
		font: inherit;
		text-align: left;
		padding: 0 8px;
	}
	.no:hover {
		background: var(--cor-fundo);
	}
	.no.sel {
		background: var(--cor-fundo);
		box-shadow: inset 2px 0 0 var(--cor-acento);
	}
	.no .seta {
		color: var(--cor-texto-suave);
		width: 10px;
	}
	.no .rotulo {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.no .meta {
		color: var(--cor-texto-suave);
		font-size: 11px;
	}

	.conteudo {
		overflow: auto;
		padding: 12px 16px;
		min-height: 0;
	}

	.cabecalho {
		display: flex;
		align-items: center;
		gap: 12px;
		margin-bottom: 10px;
	}
	h1 {
		font-size: 15px;
		font-weight: 600;
		margin: 0;
	}
	.controles {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-left: auto;
	}
	.controles select,
	.controles button {
		height: 26px;
		border: 1px solid var(--cor-borda);
		background: var(--cor-superficie);
		color: var(--cor-texto);
		border-radius: 2px;
		font: inherit;
		padding: 0 6px;
		cursor: pointer;
	}

	.resumo {
		display: flex;
		gap: 1px;
		background: var(--cor-borda);
		border: 1px solid var(--cor-borda);
		margin-bottom: 12px;
	}
	.resumo > div {
		flex: 1;
		background: var(--cor-superficie);
		padding: 8px 10px;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.resumo .num {
		font-size: 18px;
		font-weight: 600;
	}
	.resumo .rot {
		font-size: 11px;
		color: var(--cor-texto-suave);
	}
	.resumo .alerta .num {
		color: #b45309;
	}

	.cluster {
		border: 1px solid var(--cor-borda);
		background: var(--cor-superficie);
		margin-bottom: 10px;
	}
	.cluster.destaque {
		border-color: #b45309;
	}
	.linha {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 6px 10px;
		border-bottom: 1px solid var(--cor-borda);
	}
	.selo {
		margin-left: auto;
		background: #fef3c7;
		color: #7c2d12;
		border: 1px solid #f59e0b;
		padding: 1px 8px;
		font-size: 11px;
	}

	.tabela {
		width: 100%;
		border-collapse: collapse;
	}
	.tabela th,
	.tabela td {
		text-align: left;
		padding: 4px 10px;
		height: var(--altura-linha);
		border-bottom: 1px solid var(--cor-borda);
	}
	.tabela th {
		font-weight: 500;
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		color: var(--cor-texto-suave);
	}
	.tabela tbody tr:last-child td {
		border-bottom: 0;
	}
	.clicavel {
		cursor: pointer;
	}
	.clicavel:hover {
		background: var(--cor-fundo);
	}
	.compacta td {
		height: 26px;
	}
	.celula-ativa {
		color: var(--cor-acento);
		font-weight: 600;
	}

	.mono {
		font-family: ui-monospace, 'SF Mono', Menlo, Consolas, monospace;
		font-size: 12px;
	}
	.meta {
		color: var(--cor-texto-suave);
		font-size: 11px;
	}
	.badge {
		border: 1px solid var(--cor-borda);
		padding: 1px 6px;
		font-size: 11px;
		color: var(--cor-texto-suave);
	}
	.marca {
		color: #b45309;
	}
	.nota {
		color: var(--cor-texto-suave);
		font-size: 12px;
		margin: 10px 0 0;
	}
	.vazio {
		color: var(--cor-texto-suave);
		padding: 10px;
	}

	.busca-caixa {
		border: 1px solid var(--cor-borda);
		background: var(--cor-superficie);
		margin-bottom: 14px;
	}
	.erro-linha {
		color: #b91c1c;
		padding: 8px 10px;
		margin: 0;
	}
	.aviso-linha {
		color: #92400e;
		padding: 6px 10px;
		margin: 0;
		background: #fffbeb;
	}

	.campos {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 1px;
		background: var(--cor-borda);
		border: 1px solid var(--cor-borda);
		margin: 0;
	}
	.campos > div {
		background: var(--cor-superficie);
		padding: 8px 10px;
	}
	.campos dt {
		font-size: 11px;
		color: var(--cor-texto-suave);
		text-transform: uppercase;
		letter-spacing: 0.03em;
	}
	.campos dd {
		margin: 2px 0 0;
	}

	.f1 {
		line-height: 1.7;
		max-width: 70ch;
	}

	.tarefas {
		border-top: 1px solid var(--cor-borda);
		background: var(--cor-superficie);
		max-height: 160px;
		overflow: auto;
	}
	.tarefas.fechado {
		max-height: none;
	}
	.tarefas ul {
		list-style: none;
		margin: 0;
		padding: 4px 10px 8px;
	}
	.tarefas li {
		height: 22px;
		display: flex;
		gap: 8px;
		align-items: center;
	}
	.tarefas li.falha {
		color: #b91c1c;
	}

	.link {
		margin-left: auto;
		border: 0;
		background: transparent;
		color: var(--cor-acento);
		cursor: pointer;
		font: inherit;
		font-size: 12px;
	}

	.sev.critical {
		color: #b91c1c;
		font-weight: 600;
	}
	.sev.major {
		color: #b45309;
	}
</style>
