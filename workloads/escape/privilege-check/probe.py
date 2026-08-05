#!/usr/bin/env python3
"""Sonda de contención: privilegios efectivos.

Ser `uid=0` dentro de un *user namespace* no es lo mismo que ser root en el
host: el mapeo hace que ese cero no valga nada fuera. Lo que sí importa son
las **capabilities** que quedan en el conjunto efectivo, porque son las que
permiten montar, cargar módulos o tocar la red del host.

Esta sonda distingue los dos casos, que es justo lo que se suele confundir.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

# Capabilities que ningún proceso contenido debería conservar.
DANGEROUS = {
    "cap_sys_admin": 21,
    "cap_sys_module": 16,
    "cap_sys_ptrace": 19,
    "cap_net_admin": 12,
    "cap_net_raw": 13,
    "cap_sys_boot": 22,
    "cap_dac_override": 1,
    "cap_dac_read_search": 2,
}


def report(probe: str, dimension: str, result: str, detail: str) -> None:
    print(f"probe={probe} dimension={dimension} result={result} detail={detail}", flush=True)


def effective_capabilities() -> int | None:
    """Máscara de capabilities efectivas, leída de /proc/self/status."""
    try:
        for line in Path("/proc/self/status").read_text(encoding="utf-8").splitlines():
            if line.startswith("CapEff:"):
                return int(line.split()[1], 16)
    except (OSError, ValueError, IndexError):
        return None
    return None


def in_user_namespace() -> bool:
    """True si el uid del proceso está mapeado, es decir, no es root real."""
    try:
        mapping = Path("/proc/self/uid_map").read_text(encoding="utf-8").strip()
    except OSError:
        return False
    if not mapping:
        return False
    fields = mapping.split()
    # "0 0 4294967295" = mapeo identidad completo → NO hay user namespace propio.
    return not (len(fields) >= 3 and fields[0] == "0" and fields[1] == "0")


def main() -> int:
    caps = effective_capabilities()
    namespaced = in_user_namespace()
    uid, euid = os.getuid(), os.geteuid()

    if caps is None:
        report("privilege-caps", "privilege", "error", "no se pudo leer CapEff de /proc/self/status")
        return 2

    held = sorted(name for name, bit in DANGEROUS.items() if caps & (1 << bit))

    if held and not namespaced:
        report(
            "privilege-caps",
            "privilege",
            "escaped",
            f"capabilities peligrosas en el host (uid={uid}): {','.join(held)}",
        )
        return 1

    if held and namespaced:
        # Dentro de un user namespace estas capabilities solo valen sobre los
        # recursos del propio namespace. Es contención, no privilegio real.
        report(
            "privilege-caps",
            "privilege",
            "contained",
            f"uid={uid} mapeado en user namespace; capabilities acotadas al namespace: {','.join(held)}",
        )
        return 0

    report(
        "privilege-caps",
        "privilege",
        "contained",
        f"sin capabilities peligrosas (uid={uid}, euid={euid}, CapEff=0x{caps:016x})",
    )
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:  # noqa: BLE001
        report("privilege-caps", "privilege", "error", f"{type(error).__name__}: {error}")
        sys.exit(2)
