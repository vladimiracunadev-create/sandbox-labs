#!/usr/bin/env bash
set -euo pipefail
# Plantilla educativa. Ajusta las rutas desde una política validada.
exec bwrap \
  --unshare-all \
  --new-session \
  --die-with-parent \
  --ro-bind /usr /usr \
  --ro-bind /bin /bin \
  --proc /proc \
  --dev /dev \
  --tmpfs /tmp \
  --dir /workspace \
  --chdir /workspace \
  /bin/sh -lc 'printf "bubblewrap sandbox ready\n"'
