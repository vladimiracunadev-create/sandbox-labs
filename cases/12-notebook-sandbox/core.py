#!/usr/bin/env python3
"""Notebooks: datos de entrada de solo lectura, salida en otro sitio.

En los demás casos lo valioso es el equipo y lo desconocido es el programa. Aquí
lo valioso son **los datos**, y protegerlos consiste en que el análisis no pueda
modificar aquello que analiza.

Tiene un efecto secundario que la gente agradece más que la seguridad: se vuelve
imposible arruinar el dataset sin querer.
"""

from __future__ import annotations


class QuotaExceeded(RuntimeError):
    """Se alcanzó un techo. Es contención, no avería."""


def plan_mounts(datasets: list[dict], output: dict) -> dict:
    """Traduce la configuración de la sesión a montajes concretos.

    Los datasets van **siempre** de solo lectura. No es configurable: un dataset
    escribible «solo esta vez» es cómo se corrompen los datos de todo el mundo.
    """
    mounts = [{"path": dataset["path"], "mode": "ro"} for dataset in datasets]
    mounts.append({"path": output["path"], "mode": "rw"})
    return {"mounts": mounts, "readOnlyDatasets": [dataset["path"] for dataset in datasets], "outputPath": output["path"]}


def check_write(path: str, mounts: dict) -> dict:
    """¿Qué pasa cuando una celda escribe en `path`?"""
    for dataset in mounts["readOnlyDatasets"]:
        if path.startswith(dataset):
            return {
                "path": path,
                "outcome": "solo lectura",
                "detail": "el montaje es de solo lectura: falla en el sistema de ficheros, no en una comprobación",
            }
    if path.startswith(mounts["outputPath"]):
        return {"path": path, "outcome": "permitido", "detail": "dentro de la carpeta de salida"}
    return {"path": path, "outcome": "fuera de la jaula", "detail": "esa ruta no existe dentro del sandbox"}


def run_session(cells: list[dict], mounts: dict, limits: dict) -> dict:
    """Ejecuta una sesión simulada y devuelve el acta.

    Cada celda declara lo que consume y lo que toca. No se ejecuta Python de
    verdad aquí: lo que este módulo demuestra son **las cuotas y los montajes**,
    que es lo que decide si el caso contiene o no.
    """
    memory_limit = limits.get("memoryMb", 0)
    pids_limit = limits.get("pids", 0)
    output_limit = limits.get("outputMaxBytes", 0)

    executed = 0
    peak_memory = 0
    peak_pids = 0
    output_bytes = 0
    write_attempts = []
    network_attempts = []
    outputs = []

    for cell in cells:
        peak_memory = max(peak_memory, cell.get("memoryMb", 0))
        if memory_limit and peak_memory > memory_limit:
            return {
                "outcome": "killed",
                "reason": f"se alcanzó memory.max ({memory_limit} MB)",
                "cellsExecuted": executed,
                "peakMemoryMb": peak_memory,
                "writeAttempts": write_attempts,
                "networkAttempts": network_attempts,
                "outputsProduced": outputs,
            }

        peak_pids = max(peak_pids, cell.get("pids", 1))
        if pids_limit and peak_pids > pids_limit:
            return {
                "outcome": "killed",
                "reason": f"se alcanzó pids.max ({pids_limit})",
                "cellsExecuted": executed,
                "peakPids": peak_pids,
                "writeAttempts": write_attempts,
                "networkAttempts": network_attempts,
                "outputsProduced": outputs,
            }

        for path in cell.get("writes", []):
            attempt = check_write(path, mounts)
            write_attempts.append(attempt)
            if attempt["outcome"] == "permitido":
                output_bytes += cell.get("outputBytes", 0)
                outputs.append(path)

        if output_limit and output_bytes > output_limit:
            return {
                "outcome": "quota-exceeded",
                "reason": "la carpeta de salida superó su cuota",
                "cellsExecuted": executed,
                "writeAttempts": write_attempts,
                "networkAttempts": network_attempts,
                "outputsProduced": outputs,
            }

        for host in cell.get("network", []):
            allowed = host in limits.get("allowlist", [])
            network_attempts.append({"host": host, "outcome": "permitido" if allowed else "sin red"})

        executed += 1

    return {
        "outcome": "completed",
        "cellsExecuted": executed,
        "peakMemoryMb": peak_memory,
        "peakPids": peak_pids,
        "writeAttempts": write_attempts,
        "networkAttempts": network_attempts,
        "outputsProduced": outputs,
        # La limpieza es parte del caso, no un efecto secundario: lo que no está
        # en la carpeta de salida desaparece con la sesión.
        "cleanedUp": True,
    }


def handle(payload: dict) -> dict:
    """Punto de entrada: datasets, salida, celdas y cuotas."""
    mounts = plan_mounts(payload.get("datasets", []), payload.get("output", {"path": "salida/"}))
    return {"mounts": mounts, **run_session(payload.get("cells", []), mounts, payload.get("limits", {}))}
