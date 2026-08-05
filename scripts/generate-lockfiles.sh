#!/usr/bin/env bash
set -euo pipefail
cargo generate-lockfile
corepack enable
pnpm install --lockfile-only
printf 'Lockfiles actualizados. Revisa el diff antes del commit.\n'
