<script lang="ts">
	/**
	 * Casca da suíte (SPEC §8): barra superior (título + módulo A/B/C/D + tema),
	 * navegação FIXA à esquerda e conteúdo à direita.
	 *
	 * Decisão de produto desta versão: a navegação é fixa e as FOLHAS variam por seção
	 * (SPEC §3.1, regra 6) — nunca um botão morto. Seção sem dado mostra o motivo, não uma
	 * tabela vazia fingindo que funcionou.
	 */
	import { onMount } from 'svelte';
	import { Select } from 'bits-ui';
	import {
		dentroDoTauri,
		escopoFase1,
		listarModulos,
		listarOperacoes,
		type EscopoFase1,
		type GlifoEvidencia,
		type Modulo,
		type ModuloInfo,
		type Operacao
	} from '#lib/ipc';
	import { alternarTema, tema } from '#lib/theme.svelte';

	type SecaoId =
		| 'fontes' | 'olts' | 'pon' | 'onus' | 'perfis'
		| 'alarmes' | 'tarefas' | 'auditoria' | 'matriz' | 'f1';

	interface Secao {
		id: SecaoId;
		rotulo: string;
		atalho?: string;
		resume: string;
		ops: string[];
	}

	const SECOES: Secao[] = [
		{ id: 'fontes', rotulo: 'Fontes', atalho: 'Ctrl+K', resume: 'fontes de gestão cadastradas — a credencial fica no cofre do SO, nunca em arquivo', ops: ['list_sources', 'test_connectivity', 'discover_ne', 'set_source_credentials'] },
		{ id: 'olts', rotulo: 'OLTs', resume: 'OLT/NE por fonte, com modelo, versão de software e uptime', ops: ['list_olts', 'get_olt_info'] },
		{ id: 'pon', rotulo: 'Portas PON', resume: 'portas GPON/EPON/XGS por OLT, estado e nº de ONUs', ops: ['list_pon_ports', 'get_performance'] },
		{ id: 'onus', rotulo: 'ONUs', resume: 'ONUs por porta; a busca por serial é o caminho canônico Ctrl+Shift+S', ops: ['list_onus', 'find_onu_by_sn', 'get_onu', 'get_onu_auth_mode', 'authorize_onu', 'deauthorize_onu', 'reset_onu', 'set_onu_description'] },
		{ id: 'perfis', rotulo: 'Perfis', resume: 'perfis/templates por tipo (line, DBA, bandwidth, ONU-type)', ops: ['list_profiles', 'get_profile', 'create_profile/delete_profile/apply_profile'] },
		{ id: 'alarmes', rotulo: 'Alarmes', resume: 'alarmes ativos e histórico por NE', ops: ['list_alarms', 'ack_alarm/clear_alarm'] },
		{ id: 'tarefas', rotulo: 'Tarefas', resume: 'tarefas em execução, progresso e histórico', ops: ['list_tasks/get_task', 'cancel_task'] },
		{ id: 'auditoria', rotulo: 'Auditoria', resume: 'toda execução grava módulo, nível de evidência, autor e resultado', ops: [] },
		{ id: 'matriz', rotulo: 'Matriz de capacidades', atalho: 'Ctrl+Shift+C', resume: 'as 33 operações canônicas × os 4 módulos, com o nível de evidência', ops: [] },
		{ id: 'f1', rotulo: 'F1 — o que mudou para você', atalho: 'F1', resume: 'painel de onboarding; nasce em branco por decisão: nada herdado de exemplo', ops: [] }
	];

	const LEGENDA: [GlifoEvidencia, string][] = [
		['E', 'evidência direta em disco (comando/artefato nomeado)'],
		['P', 'provável (caminho plausível, sem comando nomeado)'],
		['?', 'indício insuficiente'],
		['—', 'sem evidência no cliente']
	];

	let modulos = $state<ModuloInfo[]>([]);
	let operacoes = $state<Operacao[]>([]);
	let escopo = $state<EscopoFase1 | null>(null);
	let moduloAtivo = $state<Modulo>('C');
	let secao = $state<SecaoId>('matriz');
	let carregando = $state(true);

	const secaoAtual = $derived(SECOES.find((s) => s.id === secao) ?? SECOES[8]);

	function opsDaSecao(): Operacao[] {
		return secaoAtual.ops
			.map((id) => operacoes.find((o) => o.id === id))
			.filter((o): o is Operacao => o !== undefined);
	}

	function rotuloAtual(): string {
		const m = modulos.find((x) => x.tag === moduloAtivo);
		return m ? `${m.tag} · ${m.rotulo}` : moduloAtivo;
	}

	function naFase1(id: string): boolean {
		if (!escopo) return false;
		return escopo.leitura.includes(id) || escopo.escrita.includes(id);
	}

	function atalhos(e: KeyboardEvent) {
		if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 'c') {
			e.preventDefault();
			secao = 'matriz';
		} else if (e.ctrlKey && !e.shiftKey && e.key.toLowerCase() === 'k') {
			e.preventDefault();
			secao = 'fontes';
		} else if (e.altKey && ['1', '2', '3', '4'].includes(e.key)) {
			e.preventDefault();
			moduloAtivo = (['A', 'B', 'C', 'D'] as Modulo[])[Number(e.key) - 1];
		} else if (e.key === 'F1') {
			e.preventDefault();
			secao = 'f1';
		}
	}

	onMount(() => {
		window.addEventListener('keydown', atalhos);
		void (async () => {
			modulos = (await listarModulos()) ?? [];
			operacoes = (await listarOperacoes()) ?? [];
			escopo = await escopoFase1();
			carregando = false;
		})();
		return () => window.removeEventListener('keydown', atalhos);
	});
</script>

<div class="app">
	<header class="topo">
		<span class="titulo">Suíte GPON/FTTH</span>

		<div class="modulo">
			{#if modulos.length > 0}
				<Select.Root
					type="single"
					value={moduloAtivo}
					onValueChange={(v) => (moduloAtivo = v as Modulo)}
				>
					<Select.Trigger class="sel-trigger" aria-label="Módulo de origem">
						{rotuloAtual()}
						<span class="seta">▾</span>
					</Select.Trigger>
					<Select.Content class="sel-conteudo">
						{#each modulos as m (m.tag)}
							<Select.Item value={m.tag} label={m.rotulo} class="sel-item">
								{m.tag} · {m.rotulo}
							</Select.Item>
						{/each}
					</Select.Content>
				</Select.Root>
			{:else}
				<span class="aviso">nenhum módulo carregado</span>
			{/if}
		</div>

		<button class="tema" onclick={alternarTema} aria-label="Alternar tema claro/escuro">
			tema: {tema()}
		</button>
	</header>

	<div class="corpo">
		<nav class="nav" aria-label="Navegação principal">
			<span class="nav-rotulo">navegação (fixa)</span>
			{#each SECOES as s (s.id)}
				<button
					class="nav-item"
					class:selecionado={secao === s.id}
					aria-current={secao === s.id ? 'page' : undefined}
					onclick={() => (secao = s.id)}
				>
					{s.rotulo}{#if s.atalho}<span class="atalho">{s.atalho}</span>{/if}
				</button>
			{/each}
		</nav>

		<main class="conteudo">
			{#if !dentroDoTauri}
				<p class="faixa">
					Fora do Tauri: os dados canônicos só existem no núcleo Rust. Rode
					<code>./ui/node_modules/.bin/tauri dev</code> para ver as 33 operações.
				</p>
			{/if}

			<div class="barra-resultado">
				<span class="badge">via {rotuloAtual()}</span>
				<span class="nucleo">
					{operacoes.length} operações canônicas{escopo
						? ` · Fase 1 (módulo ${escopo.modulo}): ${escopo.leitura.length} leituras + ${escopo.escrita.length} escritas`
						: ''}
				</span>
			</div>

			<h1>{secaoAtual.rotulo}</h1>
			<p class="sub">{secaoAtual.resume}</p>

			{#if secao === 'matriz'}
				<p class="sub">
					Cada linha traz o nível de <strong>evidência</strong> do módulo (SPEC §1.3). A coluna do
					módulo ativo está destacada; <span class="marca">⚠</span> marca operação destrutiva
					(exige confirmação dupla).
				</p>
				{#if carregando}
					<p>lendo o núcleo…</p>
				{:else if operacoes.length === 0}
					<p class="vazio">sem dados do núcleo (fora do Tauri).</p>
				{:else}
					<table>
						<thead>
							<tr>
								<th>#</th>
								<th>Operação</th>
								<th>Pré-condições</th>
								<th class:ativa={moduloAtivo === 'A'}>A</th>
								<th class:ativa={moduloAtivo === 'B'}>B</th>
								<th class:ativa={moduloAtivo === 'C'}>C</th>
								<th class:ativa={moduloAtivo === 'D'}>D</th>
								<th>Fase 1</th>
							</tr>
						</thead>
						<tbody>
							{#each operacoes as op (op.n)}
								<tr class:mudo={op.evidencias[moduloAtivo] === '—'}>
									<td>{op.n}</td>
									<td><code>{op.assinatura}</code>{#if op.destrutiva}<span class="marca"> ⚠</span>{/if}</td>
									<td>{op.precondicoes}</td>
									{#each ['A', 'B', 'C', 'D'] as const as m (m)}
										<td class:ativa={moduloAtivo === m}>{op.evidencias[m]}</td>
									{/each}
									<td>{naFase1(op.id) ? 'sim' : ''}</td>
								</tr>
							{/each}
						</tbody>
					</table>

					<h2>Legenda do nível de evidência</h2>
					<ul class="legenda">
						{#each LEGENDA as [g, texto] (g)}
							<li><code>{g}</code> — {texto}</li>
						{/each}
					</ul>
				{/if}

			{:else if secao === 'f1'}
				<p class="faixa">
					Os caminhos legados são preenchidos por observação real. Nada aqui é herdado de exemplo.
				</p>
				<h2>1. O caminho antigo → o caminho novo</h2>
				<p class="vazio">Vazio por padrão — preenchido só por observação na Fase 1 (SPEC §4.1/P1).</p>
				<h2>2. O que muda no seu dia</h2>
				<p class="vazio">a confirmar (depende do bloco 1).</p>
				<h2>3. O que NÃO mudou</h2>
				<p>Um caminho canônico por operação; nada de decorar três atalhos por fornecedor.</p>
				<h2>4. O que ainda não está pronto</h2>
				<p>Fases 0–1 rodam 100% contra mock, sem equipamento real. Nenhum dado de campo aqui é real.</p>
				<h2>5. Como confirmar</h2>
				<p>última atualização: — · 0 de 0 linhas confirmadas</p>

			{:else}
				{#if secaoAtual.ops.length === 0}
					<p class="vazio">
						Seção sem tabela por decisão de escopo: {secaoAtual.id === 'auditoria'
							? 'o registro de auditoria é gravado pelo núcleo a cada execução, e ainda não há execução para mostrar.'
							: 'esta seção será preenchida quando a fonte estiver cadastrada.'}
					</p>
				{:else}
					<div class="barra-resultado">
						<span class="badge">nível em {moduloAtivo}</span>
						<span class="nucleo">operações que servem esta tela</span>
					</div>
					<table>
						<thead>
							<tr><th>Operação</th><th>Pré-condições</th><th>{moduloAtivo}</th><th>Fase 1</th></tr>
						</thead>
						<tbody>
							{#each opsDaSecao() as op (op.n)}
								<tr class:mudo={op.evidencias[moduloAtivo] === '—'}>
									<td><code>{op.assinatura}</code>{#if op.destrutiva}<span class="marca"> ⚠</span>{/if}</td>
									<td>{op.precondicoes}</td>
									<td>{op.evidencias[moduloAtivo]}</td>
									<td>{naFase1(op.id) ? 'sim' : ''}</td>
								</tr>
							{/each}
						</tbody>
					</table>
					<p class="vazio">
						Sem linhas de dado: nenhuma fonte cadastrada. A Fase 0 entrega o cadastro + teste de
						conexão contra o mock; a listagem real vem na Fase 1.
					</p>
				{/if}
			{/if}
		</main>
	</div>
</div>

<style>
	.app { display: flex; flex-direction: column; height: 100vh; }

	.topo {
		display: flex; align-items: center; gap: var(--espaco);
		height: 40px; padding: 0 var(--espaco);
		background: var(--cor-superficie); border-bottom: 1px solid var(--cor-borda);
	}

	.titulo { font-weight: 600; margin-right: auto; }

	:global(.sel-trigger) { display: inline-flex; align-items: center; gap: 6px; min-width: 210px; }
	.seta { margin-left: auto; color: var(--cor-texto-suave); }
	:global(.sel-conteudo) {
		background: var(--cor-superficie); border: 1px solid var(--cor-borda);
		border-radius: 2px; padding: 2px; min-width: 210px;
	}
	:global(.sel-item) { padding: 4px 8px; cursor: default; }
	:global(.sel-item[data-highlighted]) { background: var(--cor-fundo); }

	.corpo { display: flex; flex: 1; min-height: 0; }

	.nav {
		width: 264px; flex: 0 0 264px;
		background: var(--cor-superficie); border-right: 1px solid var(--cor-borda);
		padding: var(--espaco); display: flex; flex-direction: column; gap: 2px;
	}

	.nav-rotulo {
		color: var(--cor-texto-suave); font-size: 11px;
		text-transform: uppercase; letter-spacing: 0.04em; margin-bottom: 4px;
	}

	.nav-item {
		display: flex; align-items: center; gap: 6px; white-space: nowrap;
		height: var(--altura-linha); padding: 0 8px;
		background: transparent; border: 1px solid transparent; border-radius: 2px;
		text-align: left; cursor: pointer;
	}

	.nav-item:hover { background: var(--cor-fundo); border-color: var(--cor-borda); }

	.nav-item.selecionado {
		background: var(--cor-fundo); border-color: var(--cor-acento); font-weight: 600;
	}

	.atalho { margin-left: auto; color: var(--cor-texto-suave); font-size: 11px; }

	.conteudo { flex: 1; min-width: 0; overflow: auto; padding: var(--espaco) calc(var(--espaco) * 1.5); }

	h1 { font-size: 15px; margin: 12px 0 4px; }
	h2 { font-size: 13.5px; margin: 18px 0 4px; }
	.sub { color: var(--cor-texto-suave); margin: 0 0 10px; }

	.barra-resultado { display: flex; align-items: center; gap: var(--espaco); padding: 6px 0; }

	.badge, .nucleo {
		font-size: 12px; border: 1px solid var(--cor-borda);
		background: transparent; padding: 2px 6px;
	}
	.nucleo { color: var(--cor-texto-suave); }

	.faixa {
		border: 1px solid var(--cor-borda); border-left: 3px solid var(--cor-acento);
		padding: 6px 8px; background: var(--cor-superficie);
	}

	.aviso { color: var(--cor-texto-suave); }
	.marca { color: var(--cor-acento); }
	.vazio { color: var(--cor-texto-suave); border-left: 3px solid var(--cor-borda); padding-left: 8px; }

	th.ativa, td.ativa {
		background: var(--cor-superficie); font-weight: 700;
		border-bottom: 2px solid var(--cor-acento);
	}

	tr.mudo td { color: var(--cor-texto-suave); }

	.legenda { margin: 0; padding-left: 18px; }

	code { font-family: ui-monospace, 'Cascadia Mono', Menlo, Consolas, monospace; font-size: 12.5px; }
</style>
