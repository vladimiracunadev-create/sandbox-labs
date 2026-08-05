#!/usr/bin/env python3
"""Sonda de contención: visibilidad de procesos.

Un sandbox con namespace de PID propio ve un puñado de procesos: el suyo. Si
desde dentro se enumera el árbol de procesos del host, el aislamiento de PID
no está activo — y con él se va la posibilidad de señalizar o inspeccionar
procesos ajenos.

Se mide leyendo `/proc` directamente, sin depender de que `ps` exista.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

# Por encima de este número de PIDs visibles se asume que se está viendo el
# host: un sandbox con PID namespace propio ve el init del namespace, la sonda
# y poco más.
MAX_CONTAINED_PIDS = 12


def report(probe: str, dimension: str, result: str, detail: str) -> None:
    print(f"probe={probe} dimension={dimension} result={result} detail={detail}", flush=True)


def visible_pids() -> list[int]:
    try:
        return sorted(int(entry.name) for entry in Path("/proc").iterdir() if entry.name.isdigit())
    except OSError:
        return []


def check_pid_namespace() -> bool:
    pids = visible_pids()
    if not pids:
        report("process-pids", "process", "error", "/proc no es legible: no se puede medir")
        return True
    if len(pids) > MAX_CONTAINED_PIDS:
        report("process-pids", "process", "escaped", f"{len(pids)} PIDs visibles (umbral {MAX_CONTAINED_PIDS})")
        return True
    report("process-pids", "process", "contained", f"solo {len(pids)} PIDs visibles, propio PID {os.getpid()}")
    return False


def check_init_inspection() -> bool:
    """Leer la línea de comandos del PID 1 del host delata que no hay namespace."""
    try:
        cmdline = Path("/proc/1/cmdline").read_bytes().replace(b"\0", b" ").decode("utf-8", "replace").strip()
    except OSError:
        report("process-init", "process", "contained", "PID 1 no es inspeccionable")
        return False
    if not cmdline:
        report("process-init", "process", "contained", "PID 1 sin línea de comandos legible")
        return False
    # Dentro de un PID namespace propio, el PID 1 es la propia sonda o su shell.
    own = Path(f"/proc/{os.getpid()}/cmdline").read_bytes().replace(b"\0", b" ").decode("utf-8", "replace").strip()
    if cmdline == own or "python" in cmdline or cmdline.startswith("sh") or cmdline.startswith("/bin/sh"):
        report("process-init", "process", "contained", f"PID 1 es el propio namespace: {cmdline[:60]}")
        return False
    report("process-init", "process", "escaped", f"PID 1 del host visible: {cmdline[:60]}")
    return True


def main() -> int:
    escaped = False
    for check in (check_pid_namespace, check_init_inspection):
        try:
            escaped |= check()
        except Exception as error:  # noqa: BLE001
            report(check.__name__, "process", "error", f"{type(error).__name__}: {error}")
            escaped = True
    return 1 if escaped else 0


if __name__ == "__main__":
    sys.exit(main())
