#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
corepack enable 2>/dev/null || true
pnpm dashboard:build
exec pnpm dashboard:start
