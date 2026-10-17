import adapter from '@sveltejs/adapter-static';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

// SvelteKit 3 guarda a configuração do kit DENTRO do plugin (não há mais svelte.config.js).
// Duas decisões existem por causa do Tauri:
//  - adapter-static: o app é servido do disco pelo WebView, não existe servidor Node;
//  - porta 1420 fixa: é a que o tauri.conf.json aponta em devUrl.
export default defineConfig({
	plugins: [
		sveltekit({
			compilerOptions: {
				// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
				runes: ({ filename }) =>
					filename.split(/[/\\]/).includes('node_modules') ? undefined : true
			},
			adapter: adapter({ fallback: 'index.html', strict: false })
		})
	],
	clearScreen: false,
	server: { port: 1420, strictPort: true, host: '127.0.0.1' },
	build: { target: 'esnext' }
});
