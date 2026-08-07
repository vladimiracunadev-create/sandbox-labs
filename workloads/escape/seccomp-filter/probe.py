#!/usr/bin/env python3
"""Sonda de contención: filtro de llamadas al sistema.

Un perfil seccomp en un fichero no protege de nada. Esta sonda **ejecuta** las
llamadas que la política dice denegar y mira el error que devuelve el kernel.

El truco está en elegir llamadas cuyo error sin filtro sea **distinto** de
`EPERM`, que es lo que devuelve el filtro. Si se eligiera una que ya falla con
`EPERM` por falta de privilegios —`mount`, `kexec_load`— la sonda aprobaría con
filtro y sin él: mediría el privilegio del usuario, no el sandbox.
"""

from __future__ import annotations

import ctypes
import ctypes.util
import errno
import sys

# Números de llamada por arquitectura. No coinciden entre ellas, y usar los de
# x86_64 en aarch64 filtraría llamadas al azar.
SYSCALLS = {
    "x86_64": {"perf_event_open": 298, "ptrace": 101},
    "aarch64": {"perf_event_open": 241, "ptrace": 117},
}

# Qué devuelve cada llamada cuando NO hay filtro, con los argumentos de abajo.
# Es la mitad que hace medible a la sonda.
#
# - perf_event_open(NULL, ...) → EFAULT: el kernel no puede leer la estructura.
# - ptrace(PTRACE_PEEKDATA, pid 0) → ESRCH: no existe ese proceso.
#
# Ninguno es EPERM, así que un EPERM solo puede venir del filtro.
WITHOUT_FILTER = {
    "perf_event_open": (errno.EFAULT,),
    # Algunos kernels responden EIO o EPERM a un PEEKDATA imposible según la
    # política de ptrace del host, así que ptrace se usa solo como refuerzo y
    # nunca decide por sí sola. El veredicto lo fija perf_event_open.
    "ptrace": (errno.ESRCH, errno.EIO),
}

ARGUMENTS = {
    "perf_event_open": (0, 0, -1, -1, 0),
    "ptrace": (2, 0, 0, 0),
}


def report(probe: str, dimension: str, result: str, detail: str) -> None:
    print(f"probe={probe} dimension={dimension} result={result} detail={detail}", flush=True)


def call(number: int, arguments: tuple[int, ...]) -> int:
    """Ejecuta la llamada y devuelve su errno, o 0 si tuvo éxito."""
    libc = ctypes.CDLL(ctypes.util.find_library("c"), use_errno=True)
    libc.syscall.restype = ctypes.c_long
    libc.syscall.argtypes = [ctypes.c_long] + [ctypes.c_long] * len(arguments)
    ctypes.set_errno(0)
    result = libc.syscall(number, *arguments)
    return 0 if result >= 0 else ctypes.get_errno()


def main() -> int:
    table = SYSCALLS.get(_machine())
    if table is None:
        report("seccomp-filter", "syscalls", "error", f"arquitectura sin tabla de llamadas: {_machine()}")
        return 2

    escaped = False
    for name, number in table.items():
        code = call(number, ARGUMENTS[name])
        expected = WITHOUT_FILTER[name]
        if code == errno.EPERM:
            report("seccomp-filter", "syscalls", "contained", f"{name} devolvió EPERM: el filtro la deniega")
            continue
        if code in expected:
            # Llegó al kernel y falló por sus propios motivos: no hay filtro.
            if name == "perf_event_open":
                escaped = True
                report(
                    "seccomp-filter",
                    "syscalls",
                    "escaped",
                    f"{name} llegó al kernel y devolvió {errno.errorcode.get(code, code)}: ningún filtro la bloqueó",
                )
            continue
        report(
            "seccomp-filter",
            "syscalls",
            "inconclusive",
            f"{name} devolvió {errno.errorcode.get(code, code)}, que no distingue filtro de ausencia de filtro",
        )

    return 1 if escaped else 0


def _machine() -> str:
    import platform

    return platform.machine()


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:  # noqa: BLE001
        report("seccomp-filter", "syscalls", "error", f"{type(error).__name__}: {error}")
        sys.exit(2)
