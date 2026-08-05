$ErrorActionPreference = "Stop"
$Root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
Set-Location $Root
corepack enable | Out-Null
pnpm dashboard:build
Start-Process "http://127.0.0.1:9093"
pnpm dashboard:start
