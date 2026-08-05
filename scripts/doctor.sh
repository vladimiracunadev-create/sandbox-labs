#!/usr/bin/env bash
set -euo pipefail
printf 'Sandbox Labs preflight\n'
for cmd in node pnpm cargo bwrap unshare prlimit wasmtime runsc firecracker; do
  if command -v "$cmd" >/dev/null 2>&1; then printf '✅ %-12s %s\n' "$cmd" "$($cmd --version 2>&1 | head -n1)"; else printf '⚪ %-12s no disponible\n' "$cmd"; fi
done
if [[ -e /dev/kvm ]]; then printf '✅ /dev/kvm\n'; else printf '⚪ /dev/kvm no disponible\n'; fi
