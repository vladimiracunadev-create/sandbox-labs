#!/usr/bin/env python3
"""Construcción de paquetes en dos fases: se resuelve con red, se compila sin ella.

Construir software tiene dos momentos que se suelen confundir en uno. Resolver
dependencias necesita red. **Compilar no la necesita**, y sin embargo casi
siempre la tiene — y esa segunda fase ejecuta código arbitrario de cada
dependencia, con tus permisos.

Cerrar la red a mitad del proceso es un control barato: el momento del cambio
está perfectamente definido, justo después de verificar los checksums. Casi nadie
lo aplica.
"""

from __future__ import annotations


class ResolveError(RuntimeError):
    """Lo descargado no coincide con el fichero de bloqueo."""


def resolve(manifest: list[dict], lockfile: dict[str, str], allowlist: list[str], registry: str) -> dict:
    """Fase 1: descargar y **verificar antes de cerrar la red**.

    Si un checksum no cuadra no se sigue. Puede ser una caché sucia o un paquete
    alterado bajo la misma versión; en los dos casos, construir con él sería
    construir con algo que nadie ha visto.
    """
    if registry not in allowlist:
        raise ResolveError(f"el registro {registry} no está en la lista de permitidos")

    resolved, mismatches = [], []
    for package in manifest:
        name, version, digest = package["name"], package["version"], package["sha256"]
        expected = lockfile.get(f"{name}@{version}")
        if expected is None:
            mismatches.append({"package": f"{name}@{version}", "reason": "no está en el fichero de bloqueo"})
        elif expected != digest:
            mismatches.append({"package": f"{name}@{version}", "reason": "el checksum no coincide"})
        else:
            resolved.append(package)

    if mismatches:
        raise ResolveError("; ".join(f"{item['package']}: {item['reason']}" for item in mismatches))

    return {"resolved": resolved, "sbom": [{"name": p["name"], "version": p["version"], "sha256": p["sha256"]} for p in resolved]}


def build(resolved: list[dict], network_attempts: dict[str, list[str]]) -> dict:
    """Fase 2: compilar **sin red**.

    `network_attempts` es lo que cada paquete intentó durante su `postinstall`.
    Aquí no sale nada: la fase 2 no tiene pila de red. Lo que se hace con esos
    intentos es anotarlos, porque una dependencia que quiere salir a internet
    mientras compila es una señal por sí sola, funcione la construcción o no.
    """
    attempts = []
    for package, hosts in network_attempts.items():
        for host in hosts:
            attempts.append({"from": f"postinstall de {package}", "host": host, "outcome": "sin red: fallo de resolución"})

    scripted = [package["name"] for package in resolved if package.get("has_install_script")]

    return {
        "outcome": "built",
        "buildNetwork": "none",
        "packagesWithInstallScripts": scripted,
        "buildNetworkAttempts": attempts,
        # Que la fase 2 no tenga red es la afirmación del caso, y va explícita
        # para que un cambio que la rompa se vea en el acta.
        "networkClosedBeforeBuild": True,
    }


def pipeline(manifest: list[dict], lockfile: dict[str, str], allowlist: list[str], registry: str, network_attempts: dict[str, list[str]]) -> dict:
    """Las dos fases seguidas, en jaulas distintas.

    Dos jaulas y no una que cambia: es más simple de razonar y no deja ninguna
    ventana en la que la red siga abierta por descuido.
    """
    try:
        first = resolve(manifest, lockfile, allowlist, registry)
    except ResolveError as error:
        return {"outcome": "not-built", "phase": "resolve", "reason": str(error), "networkClosedBeforeBuild": False}

    second = build(first["resolved"], network_attempts)
    return {**second, "sbom": first["sbom"], "phase": "build"}


def handle(payload: dict) -> dict:
    """Punto de entrada: manifiesto, bloqueo y lo que intentó cada postinstall."""
    return pipeline(
        payload.get("manifest", []),
        payload.get("lockfile", {}),
        payload.get("allowlist", []),
        payload.get("registry", ""),
        payload.get("networkAttempts", {}),
    )
