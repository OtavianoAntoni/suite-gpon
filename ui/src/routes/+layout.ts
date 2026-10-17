// O WebView do Tauri não tem servidor: a página é um SPA servido de arquivo.
// `ssr = false` desliga renderização no servidor; o adapter-static gera o shell (fallback).
export const ssr = false;
export const prerender = false;
