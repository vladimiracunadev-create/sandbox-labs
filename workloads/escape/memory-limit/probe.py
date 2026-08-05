#!/usr/bin/env python3
"""Sonda de contención: límite de memoria.

Pide memoria en tramos crecientes hasta pasarse del límite declarado por la
política. Un runtime que aplica el control corta la asignación (`MemoryError`)
o mata el proceso; uno que no lo aplica deja que la carga se coma la RAM del
host y arrastre consigo a todo lo demás.

El presupuesto llega por argumento en MB para que la sonda mida contra lo que
la política pidió, no contra un número inventado aquí.
"""

from __future__ import annotations

import sys

CHUNK_MB = 16
# Se intenta el doble del presupuesto: suficiente para pasarse sin pedir una
# cantidad absurda que ahogue al host si el control no está.
OVERSHOOT_FACTOR = 2


def report(result: str, detail: str) -> None:
    print(f"probe=memory-limit dimension=memory result={result} detail={detail}", flush=True)


def main() -> int:
    budget_mb = int(sys.argv[1]) if len(sys.argv) > 1 and sys.argv[1].isdigit() else 256
    target_mb = budget_mb * OVERSHOOT_FACTOR

    blocks: list[bytearray] = []
    allocated_mb = 0
    try:
        while allocated_mb < target_mb:
            # `bytearray` se toca al crearse, así que la memoria se compromete
            # de verdad y no se queda en una reserva perezosa.
            blocks.append(bytearray(CHUNK_MB * 1024 * 1024))
            allocated_mb += CHUNK_MB
    except MemoryError:
        report("contained", f"MemoryError tras {allocated_mb} MB con presupuesto de {budget_mb} MB")
        return 0

    report("escaped", f"asignados {allocated_mb} MB con un presupuesto de {budget_mb} MB")
    return 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:  # noqa: BLE001
        report("error", f"{type(error).__name__}: {error}")
        sys.exit(2)
