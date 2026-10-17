#!/usr/bin/env bash
# Gate local — os MESMOS comandos do CI, para não descobrir no push o que já falhava aqui.
# Uso:  bash scripts/verificar.sh
# Sai com código != 0 no primeiro problema; imprime um resumo por etapa.
set -uo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FALHAS=0
etapa() { printf '\n== %s ==\n' "$1"; }
ok()    { printf 'OK   %s\n' "$1"; }
falha() { printf 'FAIL %s\n' "$1"; FALHAS=$((FALHAS + 1)); }

etapa "1/4 Rust: domínio + transporte + contrato com a UI"
if cargo test --workspace -j 4 --manifest-path "$RAIZ/Cargo.toml" >/tmp/suite-gpon-cargo.log 2>&1; then
	ok "$(grep -c '^test result: ok' /tmp/suite-gpon-cargo.log 2>/dev/null || echo n/a) suítes de teste passaram"
else
	falha "cargo test — ver /tmp/suite-gpon-cargo.log"
	tail -20 /tmp/suite-gpon-cargo.log
fi

etapa "2/4 UI: lint anti-animação"
if (cd "$RAIZ/ui" && npm run --silent lint:no-anim); then
	ok "nenhuma animação na UI"
else
	falha "lint anti-animação (SPEC §8 proíbe animação)"
fi

etapa "3/4 UI: build estático (frontendDist do Tauri)"
if (cd "$RAIZ/ui" && npm run --silent build >/tmp/suite-gpon-ui.log 2>&1); then
	ok "ui/build gerado ($(du -sh "$RAIZ/ui/build" 2>/dev/null | cut -f1))"
else
	falha "build da UI — ver /tmp/suite-gpon-ui.log"
	tail -20 /tmp/suite-gpon-ui.log
fi

etapa "4/4 SPEC: régua estrutural (só onde a spec existe)"
# Pega a régua MAIS NOVA por versão (sort -V), em vez de fixar um número que envelhece:
REGUA=$(ls -1 "$HOME/.hermes/profiles/netdev/cache/scratch"/v[0-9]*_verify.sh 2>/dev/null | sort -V | tail -1)
SPEC="${GPON_SPEC:-$HOME/.hermes/profiles/netdev/artifacts/SPEC-suite-gpon-rust-tauri.md}"
if [ -f "$REGUA" ] && [ -f "$SPEC" ]; then
	if bash "$REGUA" >/dev/null 2>&1; then ok "régua da spec (exit 0)"; else falha "régua da spec"; fi
else
	printf 'SKIP régua da spec (sem spec/régua neste ambiente — normal em CI)\n'
fi

printf '\n'
if [ "$FALHAS" -eq 0 ]; then
	echo "RESULTADO: tudo verde"
	exit 0
fi
echo "RESULTADO: $FALHAS etapa(s) com falha"
exit 1
