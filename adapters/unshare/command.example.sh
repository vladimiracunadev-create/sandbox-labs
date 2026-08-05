#!/usr/bin/env bash
set -euo pipefail
exec unshare \
  --user --map-root-user \
  --mount --pid --fork --mount-proc \
  --uts --ipc \
  /bin/sh -lc 'hostname sandbox-lab 2>/dev/null || true; printf "pid=%s host=%s\n" "$$" "$(hostname)"'
