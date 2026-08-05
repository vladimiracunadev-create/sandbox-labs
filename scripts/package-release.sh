#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$(node -p "JSON.parse(require('fs').readFileSync('$ROOT/package.json','utf8')).version")"
OUT="$ROOT/artifacts"
ARCHIVE="$OUT/sandbox-labs-$VERSION.zip"
CHECKSUM="$ARCHIVE.sha256"

node "$ROOT/control-center/scripts/build.mjs"
node "$ROOT/scripts/generate-file-manifest.mjs"

mkdir -p "$OUT"
rm -f "$ARCHIVE" "$CHECKSUM"

cd "$ROOT/.."
zip -X -qr "$ARCHIVE" sandbox-labs \
  -x 'sandbox-labs/.git/*' \
     'sandbox-labs/target/*' \
     'sandbox-labs/node_modules/*' \
     'sandbox-labs/control-center/node_modules/*' \
     'sandbox-labs/.sandbox-data/*' \
     'sandbox-labs/artifacts/*' \
     'sandbox-labs/evidence/runs/*.json'

(cd "$OUT" && sha256sum "$(basename "$ARCHIVE")" > "$(basename "$CHECKSUM")")
unzip -tq "$ARCHIVE" >/dev/null
printf '%s\n' "$ARCHIVE"
