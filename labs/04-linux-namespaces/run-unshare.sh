#!/usr/bin/env bash
set -euo pipefail
command -v unshare >/dev/null || { echo 'unshare no está disponible'; exit 2; }
unshare --user --map-root-user --mount --pid --fork --mount-proc --uts --ipc \
  /bin/sh -lc 'hostname sandbox-lab 2>/dev/null || true; echo "inside uid=$(id -u) pid=$$ hostname=$(hostname)"; ps -o pid,ppid,comm'
