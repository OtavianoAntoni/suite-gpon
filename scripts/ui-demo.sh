#!/usr/bin/env bash
# ui-demo.sh — mesma coisa que scripts/ui-demo.cmd, para Linux/macOS.
# Abre a UI em modo demonstração (espelho das fixtures, sem app Tauri e sem NE).
set -uo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$RAIZ/ui" || { echo "[erro] pasta ui não encontrada"; exit 1; }

command -v node >/dev/null 2>&1 || { echo "[erro] Node.js não está no PATH (Node 20+)"; exit 1; }

if [ ! -d node_modules ]; then
	printf 'Instalando dependências da UI (uma vez só)...\n'
	npm ci --no-audit --no-fund || { echo "[erro] npm ci falhou"; exit 1; }
fi

printf '\nAbrindo http://localhost:1420 (o vite abre o navegador quando estiver pronto).\n'
printf 'Para encerrar: Ctrl+C.\n\n'
exec npm run dev -- --open
