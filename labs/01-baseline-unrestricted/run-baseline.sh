#!/usr/bin/env bash
set -euo pipefail
printf 'user=%s uid=%s gid=%s pid=%s\n' "$(id -un)" "$(id -u)" "$(id -g)" "$$"
printf 'cwd=%s hostname=%s\n' "$PWD" "$(hostname)"
printf 'network-interfaces:\n'
if command -v ip >/dev/null; then ip -brief address || true; else printf 'ip command unavailable\n'; fi
printf 'filesystem probes:\n'
for path in /etc/hostname /tmp "$HOME"; do
  if [ -r "$path" ]; then printf 'readable %s\n' "$path"; else printf 'blocked %s\n' "$path"; fi
done
