#!/usr/bin/env python3
"""Instalar una dependencia es ejecutar código de desconocidos, y nadie lo llama así.

Instalar no es copiar ficheros: en la mayoría de ecosistemas **ejecuta scripts**
—`postinstall`, `preinstall`, `build.rs`— con tus permisos y antes de que nadie
mire el código. Y no instalas una biblioteca: instalas su árbol entero, mantenido
por gente que no se conoce entre sí.

Este módulo hace visible ese momento: qué scripts corren, qué buscan en el
entorno y a dónde intentan conectarse.
"""

from __future__ import annotations

# Paquetes populares contra los que se compara para detectar nombres casi
# iguales. Sintético: no se descarga ninguna lista real.
POPULAR = ["requests", "express", "lodash", "numpy", "serde", "pandas", "react", "urllib3"]

# Distancia máxima de edición para considerar un nombre sospechosamente parecido.
# Es una señal, no un veredicto: se revisa a mano.
TYPOSQUAT_DISTANCE = 2


def edit_distance(left: str, right: str) -> int:
    """Distancia de Levenshtein. Sin dependencias: son quince líneas."""
    if left == right:
        return 0
    previous = list(range(len(right) + 1))
    for i, a in enumerate(left, 1):
        current = [i]
        for j, b in enumerate(right, 1):
            current.append(min(previous[j] + 1, current[j - 1] + 1, previous[j - 1] + (a != b)))
        previous = current
    return previous[-1]


def typosquat_suspects(names: list[str]) -> list[dict]:
    """Nombres sospechosamente parecidos a uno popular.

    Un paquete llamado casi igual que el que querías es la forma más barata de
    entrar en miles de proyectos.
    """
    suspects = []
    for name in names:
        if name in POPULAR:
            continue
        for popular in POPULAR:
            distance = edit_distance(name, popular)
            if 0 < distance <= TYPOSQUAT_DISTANCE:
                suspects.append({"name": name, "similarTo": popular, "distance": distance})
                break
    return suspects


def install(manifest: list[dict], lockfile: dict[str, str], allowlist: list[str], environment: dict) -> dict:
    """Instala en jaula, con el entorno vacío, y devuelve el informe.

    El entorno vacío es el control central del caso: convierte «el `postinstall`
    se llevó tu token» en «el `postinstall` buscó un token y no había ninguno».
    """
    direct = [package for package in manifest if package.get("direct", True)]
    transitive = [package for package in manifest if not package.get("direct", True)]

    scripts, environment_reads, network_attempts, mismatches = [], [], [], []

    for package in manifest:
        name = f"{package['name']}@{package['version']}"

        expected = lockfile.get(name)
        if expected is not None and expected != package.get("sha256"):
            mismatches.append({"package": name, "reason": "el checksum no coincide con el fichero de bloqueo"})

        script = package.get("install_script")
        if script:
            scripts.append({"name": package["name"], "version": package["version"], "script": script["hook"], "command": script["command"]})

            for variable in script.get("reads_environment", []):
                environment_reads.append(
                    {
                        "package": package["name"],
                        "variable": variable,
                        # El intento se hizo; lo que no había era nada que leer.
                        "outcome": "entorno vacío: no había nada que leer"
                        if variable not in environment
                        else "LEÍDO: el entorno no estaba vacío",
                    }
                )

            for host in script.get("connects_to", []):
                network_attempts.append(
                    {
                        "package": package["name"],
                        "host": host,
                        "outcome": "permitido" if host in allowlist else "bloqueado por lista de permitidos",
                    }
                )

    leaked = [read for read in environment_reads if read["outcome"].startswith("LEÍDO")]

    return {
        "direct": len(direct),
        "transitive": len(transitive),
        "packagesWithInstallScripts": scripts,
        "environmentReads": environment_reads,
        "networkAttempts": network_attempts,
        "typosquattingSuspects": typosquat_suspects([package["name"] for package in manifest]),
        "checksumMismatches": mismatches,
        # Si esto no está vacío, el entorno no se vació y hay que rotar lo que
        # se leyera.
        "leakedFromEnvironment": leaked,
        "installed": not mismatches,
    }


def handle(payload: dict) -> dict:
    """Punto de entrada: manifiesto, bloqueo, lista de permitidos y entorno."""
    return install(
        payload.get("manifest", []),
        payload.get("lockfile", {}),
        payload.get("allowlist", []),
        payload.get("environment", {}),
    )
