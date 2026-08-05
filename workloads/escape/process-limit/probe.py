#!/usr/bin/env python3
"""Sonda de contención: límite de procesos.

Crea procesos hijo **acotados** hasta pasarse del presupuesto declarado por la
política. No es una fork bomb: el número de intentos está limitado por
`OVERSHOOT_FACTOR` y cada hijo se recoge al terminar, de modo que la sonda es
segura de ejecutar incluso cuando el runtime no aplica ningún control.

Si el runtime aplica `RLIMIT_NPROC` o un cgroup de PIDs, la creación falla con
`BlockingIOError`/`OSError` antes de llegar al techo.
"""

from __future__ import annotations

import os
import sys
import time

OVERSHOOT_FACTOR = 2
HARD_CEILING = 256  # tope absoluto: la sonda nunca intenta más que esto


def report(result: str, detail: str) -> None:
    print(f"probe=process-limit dimension=processes result={result} detail={detail}", flush=True)


def main() -> int:
    budget = int(sys.argv[1]) if len(sys.argv) > 1 and sys.argv[1].isdigit() else 24
    target = min(budget * OVERSHOOT_FACTOR, HARD_CEILING)

    children: list[int] = []
    spawned = 0
    blocked_by: str | None = None

    try:
        while spawned < target:
            try:
                pid = os.fork()
            except OSError as error:
                blocked_by = type(error).__name__
                break
            if pid == 0:
                # El hijo solo espera un instante y sale sin efectos laterales.
                time.sleep(0.4)
                os._exit(0)
            children.append(pid)
            spawned += 1
    finally:
        for pid in children:
            try:
                os.waitpid(pid, 0)
            except OSError:
                pass

    if blocked_by:
        report("contained", f"{blocked_by} tras {spawned} procesos con presupuesto de {budget}")
        return 0

    report("escaped", f"creados {spawned} procesos con un presupuesto de {budget}")
    return 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:  # noqa: BLE001
        report("error", f"{type(error).__name__}: {error}")
        sys.exit(2)
