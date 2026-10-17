<script lang="ts">
	import '#lib/tokens.css';
	import favicon from '#lib/assets/favicon.svg';
	import { onMount } from 'svelte';
	import { carregarTema } from '#lib/theme.svelte';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	// O tema salvo é aplicado uma única vez, no <html>; a troca não recarrega a janela.
	onMount(() => {
		void carregarTema();
	});
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

{@render children()}
