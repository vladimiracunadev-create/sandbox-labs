$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
corepack enable | Out-Null
pnpm dashboard:build
pnpm dashboard:start
