#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
if [[ -n "${SANDBOXCTL_BIN:-}" ]]; then
  ctl=("$SANDBOXCTL_BIN")
else
  ctl=(cargo run --locked -q -p sandboxctl --)
fi

cleanup() {
  "${ctl[@]}" --root "$root" service down --all >/dev/null 2>&1 || true
}
trap cleanup EXIT

cleanup
"${ctl[@]}" --root "$root" service up file-detonation

health="$(curl -fsS http://127.0.0.1:8803/health)"
node -e '
  const value = JSON.parse(process.argv[1]);
  if (value.status !== "ok" || value.runtime !== "bwrap") {
    throw new Error(`health inesperado: ${process.argv[1]}`);
  }
' "$health"

state="$("${ctl[@]}" --root "$root" service list --json)"
node -e '
  const list = JSON.parse(process.argv[1]);
  const service = list.find((value) => value.id === "file-detonation");
  if (!service || service.state !== "running" || service.record?.runtime !== "bwrap") {
    throw new Error("file-detonation no está corriendo con bubblewrap");
  }
  const required = ["capabilities", "cpu", "devices", "environment", "filesystem", "memory", "network", "processes", "syscalls"];
  const effective = new Set(service.record.effectiveControls ?? []);
  const missing = required.filter((control) => !effective.has(control));
  if (missing.length) throw new Error(`controles efectivos ausentes: ${missing.join(", ")}`);
' "$state"

"${ctl[@]}" --root "$root" service down file-detonation
if curl -fsS --max-time 1 http://127.0.0.1:8803/health >/dev/null 2>&1; then
  echo "el puerto 8803 sigue respondiendo después de service down" >&2
  exit 1
fi
if ps -eo args= | grep -F "$root/cases/03-file-detonation" | grep -F "/workspace/app" | grep -v -F "grep -F" | grep -q .; then
  echo "bubblewrap sigue vivo después de service down" >&2
  exit 1
fi

trap - EXIT
echo "✅ ciclo real: service up → bubblewrap → localhost:8803 → service down"
