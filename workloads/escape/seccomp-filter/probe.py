#!/usr/bin/env python3
"""Sonda de contención: filtro de llamadas al sistema.

Un perfil seccomp en un fichero no protege de nada. Esta sonda **ejecuta** una
llamada que la política deniega y mira qué responde el kernel.

# Por qué `getcpu` y no una llamada peligrosa

El instinto dice medir con `mount` o `ptrace`. Es una trampa: esas ya fallan con
`EPERM` para cualquier usuario sin privilegios, así que la sonda aprobaría con
filtro y sin él — mediría el privilegio del usuario, no el sandbox.

El segundo intento usó `perf_event_open(NULL, …)`, que devuelve `EFAULT` en una
máquina normal. También falló: en el runner de CI devuelve `EACCES` por
`perf_event_paranoid`, y en un host con ese sysctl en 3 devolvería `EPERM` sin
que hubiera filtro alguno. Un discriminador que depende de la configuración del
host no discrimina.

`getcpu(NULL, NULL, NULL)` **tiene éxito siempre**, para cualquiera y en
cualquier host. Así que solo hay dos respuestas posibles y significan una cosa
cada una:

- éxito → ningún filtro la bloqueó
- `EPERM` → el filtro la denegó

Por eso `containment-audit` —la política cuya única razón de ser es medir— la
incluye en su lista de denegación.
"""

from __future__ import annotations

import ctypes
import ctypes.util
import errno
import platform
import sys

# Número de `getcpu` por arquitectura. No coinciden entre ellas, y usar el de
# x86_64 en aarch64 llamaría a otra cosa distinta.
GETCPU = {"x86_64": 309, "aarch64": 168}

# Llamada que la política también deniega. Solo se informa: su error sin filtro
# depende del host, así que no puede decidir el veredicto.
#
# `ptrace` NO está aquí, y el motivo importa: llamarla con `request=0` es
# `PTRACE_TRACEME`, que **tiene éxito** y deja el proceso detenido esperando a su
# padre. Con filtro no se nota —devuelve EPERM antes de nada— pero sin filtro la
# sonda se colgaba y no llegaba a imprimir su veredicto. Una sonda que se cuelga
# justo en el caso que tiene que detectar es peor que no tenerla.
EXTRAS = {
    "x86_64": {"perf_event_open": 298},
    "aarch64": {"perf_event_open": 241},
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


def name_of(code: int) -> str:
    return "éxito" if code == 0 else errno.errorcode.get(code, str(code))


def main() -> int:
    machine = platform.machine()
    number = GETCPU.get(machine)
    if number is None:
        report("seccomp-filter", "syscalls", "error", f"arquitectura sin número de getcpu: {machine}")
        return 2

    # Contexto informativo, nunca decisorio.
    extras = ", ".join(
        f"{name}={name_of(call(value, (0, 0, -1, -1, 0)))}" for name, value in sorted(EXTRAS.get(machine, {}).items())
    )

    code = call(number, (0, 0, 0))
    if code == errno.EPERM:
        report("seccomp-filter", "syscalls", "contained", f"getcpu devolvió EPERM: el filtro la deniega · {extras}")
        return 0
    if code == 0:
        report(
            "seccomp-filter",
            "syscalls",
            "escaped",
            f"getcpu tuvo éxito: ningún filtro la bloqueó · {extras}",
        )
        return 1
    report(
        "seccomp-filter",
        "syscalls",
        "inconclusive",
        f"getcpu devolvió {name_of(code)}, que no es ni éxito ni EPERM · {extras}",
    )
    return 2


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:  # noqa: BLE001
        report("seccomp-filter", "syscalls", "error", f"{type(error).__name__}: {error}")
        sys.exit(2)
