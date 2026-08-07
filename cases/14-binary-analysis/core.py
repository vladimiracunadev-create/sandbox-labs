#!/usr/bin/env python3
"""Caracterizar un binario de terceros: qué hace, no si es peligroso.

Con código fuente puedes leer antes de ejecutar. Con un binario no: es un fichero
de instrucciones de máquina, y quien lo escribió pudo haber previsto que lo
mirases.

Comparte frontera con el caso 06, pero la pregunta es otra: allí se observa una
**muestra sospechosa**; aquí se caracteriza un **programa que probablemente
quieras usar**. El resultado no es «peligroso o no», es «esto es lo que hace».

Los tres datos que más valen: **a qué se conecta**, **qué escribe fuera de su
carpeta** y **qué otros programas lanza**.
"""

from __future__ import annotations

import os
import re

# Formatos reconocibles por sus primeros bytes.
FORMATS = [
    (b"\x7fELF", "ELF"),
    (b"MZ", "PE"),
    (b"\xca\xfe\xba\xbe", "Mach-O universal"),
    (b"\xcf\xfa\xed\xfe", "Mach-O 64"),
    (b"#!", "script con intérprete"),
]

# Cadenas que merecen mirarse. Ninguna prueba nada por sí sola: son el punto de
# partida de la fase dinámica, no una conclusión.
INTERESTING = [
    (re.compile(rb"/etc/shadow"), "lee la base de contraseñas"),
    (re.compile(rb"/etc/passwd"), "lee la lista de usuarios"),
    (re.compile(rb"\.ssh/id_"), "busca claves SSH"),
    (re.compile(rb"\.aws/credentials"), "busca credenciales de nube"),
    (re.compile(rb"169\.254\.169\.254"), "busca el servicio de metadatos de la nube"),
    (re.compile(rb"(?:curl|wget)\s"), "descarga cosas"),
    (re.compile(rb"crontab|systemctl\s+enable"), "intenta persistir"),
]


def preflight() -> dict:
    """La fase dinámica necesita KVM. La estática no."""
    if os.path.exists("/dev/kvm"):
        return {"staticOnly": False, "canRunDynamic": True}
    return {
        "staticOnly": True,
        "canRunDynamic": False,
        "detail": "sin /dev/kvm solo se puede hacer análisis estático, y así se declara",
        "alternatives": ["activar la virtualización anidada en WSL2", "ejecutar el caso en una máquina con KVM"],
    }


def static_analysis(data: bytes) -> dict:
    """Primera pasada: **sin ejecutar nada**.

    Todo lo que se pueda averiguar sin arrancar el binario reduce lo que hay que
    arriesgar después.
    """
    fmt = next((name for signature, name in FORMATS if data.startswith(signature)), "desconocido")

    libraries = sorted({match.decode("ascii", "replace") for match in re.findall(rb"lib[a-z0-9_+\-]+\.so(?:\.\d+)*", data)})
    interesting = [{"pattern": pattern.pattern.decode("ascii", "replace"), "why": why} for pattern, why in INTERESTING if pattern.search(data)]

    return {"format": fmt, "bytes": len(data), "linkedLibraries": libraries, "interestingStrings": interesting}


def profile(static: dict, observed: dict, home: str = "/home/analista") -> dict:
    """Junta lo estático y lo dinámico en un perfil comparable entre versiones.

    Comparable importa: un binario que se comporta distinto entre dos
    ejecuciones —o entre dos versiones— es exactamente lo que interesa detectar.
    """
    written = observed.get("filesWritten", [])
    outside = [path for path in written if not path.startswith(home) and not path.startswith("/tmp")]

    return {
        "static": static,
        "dynamic": {
            "syscalls": observed.get("syscalls", {}),
            "filesRead": observed.get("filesRead", []),
            "filesWritten": written,
            "filesWrittenOutsideHome": outside,
            "networkAttempts": [
                {"host": host, "outcome": "simulada, no salió"} for host in observed.get("networkAttempts", [])
            ],
            "processesSpawned": observed.get("processesSpawned", []),
        },
        "summary": {
            "connects": bool(observed.get("networkAttempts")),
            "writesOutsideItsFolder": bool(outside),
            "spawnsPrograms": bool(observed.get("processesSpawned")),
        },
        "vmDestroyed": observed.get("vmDestroyed", False),
    }


def differs(first: dict, second: dict) -> list[str]:
    """En qué se diferencian dos perfiles del mismo binario.

    Un binario que se comporta distinto cuando cree que lo observan es el
    hallazgo más valioso del caso.
    """
    changes = []
    for key in ("connects", "writesOutsideItsFolder", "spawnsPrograms"):
        if first["summary"][key] != second["summary"][key]:
            changes.append(f"{key}: {first['summary'][key]} → {second['summary'][key]}")
    return changes


def handle(payload: dict) -> dict:
    """Punto de entrada: el binario en base64 y lo observado, si lo hubo."""
    import base64

    raw = payload.get("binaryBase64", "")
    try:
        data = base64.b64decode(raw, validate=True)
    except Exception as error:  # noqa: BLE001
        raise ValueError(f"binaryBase64 no es base64 válido: {error}") from error

    static = static_analysis(data)
    return {"preflight": preflight(), **profile(static, payload.get("observed", {}))}
