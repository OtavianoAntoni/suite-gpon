@echo off
rem ---------------------------------------------------------------------------
rem  ui-demo.cmd — abre a UI da suíte no navegador, em MODO DEMONSTRAÇÃO.
rem
rem  Por que existe: fora do app Tauri a tela usa um espelho das fixtures
rem  (`ui/src/lib/demo.ts`) e mostra a faixa "modo demonstração". Serve para ver
rem  layout e navegação sem precisar compilar o Rust — que nesta máquina não
rem  linka (alvo GNU) e no Windows pede MSVC.
rem
rem  O que NÃO é: não abre OLT, não conecta em NE, não substitui o app.
rem  Duplo-clique neste arquivo. Ctrl+C na janela encerra.
rem ---------------------------------------------------------------------------
setlocal
cd /d "%~dp0..\ui" || (echo Nao encontrei a pasta ui. & pause & exit /b 1)

where node >nul 2>&1
if errorlevel 1 (
  echo [erro] Node.js nao esta no PATH. Instale o Node 20+ e rode de novo.
  pause
  exit /b 1
)

if not exist node_modules (
  echo Instalando dependencias da UI ^(uma vez so, ~90 MB^)...
  call npm ci --no-audit --no-fund
  if errorlevel 1 (
    echo [erro] npm ci falhou. Veja a mensagem acima.
    pause
    exit /b 1
  )
)

echo.
echo  Abrindo http://localhost:1420 no navegador ^(vite abre sozinho quando estiver pronto^).
echo  Para encerrar: Ctrl+C nesta janela.
echo.
call npm run dev -- --open
