#!/usr/bin/env python3
"""Detonación en microVM: cuándo el namespace ya no basta.

Los demás casos técnicos usan namespaces: la muestra corre en **el mismo núcleo**
que todo lo demás, con una vista recortada. Eso basta para código desconocido y
no para código que intenta activamente escapar — un núcleo compartido tiene
cientos de puntos de entrada, y si la muestra encuentra un fallo en uno solo, la
vista recortada deja de existir.

Y hay una segunda razón, distinta: para **observar comportamiento** hay que dejar
que la muestra actúe. No se trata de impedirle escribir ficheros, se trata de
dejarla escribirlos y anotar cuáles.

> Las muestras del repositorio son **sintéticas e inofensivas**: imitan el
> comportamiento sin hacer daño. Nunca malware real, ni aquí ni en el equipo
> anfitrión.

Este módulo hace dos cosas que **no necesitan una máquina virtual**: comprobar si
este equipo puede ejecutar el caso, y clasificar una línea de tiempo observada.
La detonación en sí necesita KVM, y cuando no está, se dice.
"""

from __future__ import annotations

import os

# Lo que hace falta para detonar de verdad. Sin esto, el caso **no se ejecuta
# aquí** y lo dice en vez de fingir que sí.
REQUIREMENTS = [
    ("/dev/kvm", "virtualización por hardware"),
]


def preflight() -> dict:
    """¿Puede este equipo detonar una muestra?

    Se responde antes de intentar nada. Un caso que arranca y falla a mitad deja
    una máquina virtual a medias con una muestra dentro, que es el peor
    resultado posible.
    """
    missing = [(path, why) for path, why in REQUIREMENTS if not os.path.exists(path)]
    if missing:
        return {
            "canRun": False,
            "missing": [{"path": path, "why": why} for path, why in missing],
            "alternatives": [
                "en WSL2, activar la virtualización anidada en .wslconfig (nestedVirtualization=true)",
                "comprobar que el BIOS tiene VT-x o AMD-V",
                "ejecutar este caso en una máquina Linux con KVM",
            ],
            "detail": "sin virtualización por hardware no hay microVM, y un namespace no sustituye a una máquina",
        }
    return {"canRun": True, "missing": []}


# Cómo se clasifica cada cosa observada. El nombre no es decorativo: es lo que
# convierte una lista de eventos en algo que alguien puede leer y decidir.
BEHAVIOURS = {
    "process": "lanzó otro programa",
    "file": "escribió un fichero",
    "persistence": "intentó sobrevivir a un reinicio",
    "network": "intentó conectarse",
    "privilege": "intentó subir de privilegios",
}

# Rutas cuya escritura significa persistencia. Escribir en una carpeta temporal
# es normal; escribir aquí es querer estar mañana.
PERSISTENCE_PATHS = ("/etc/cron", "/etc/systemd", "/autostart/", "/.config/autostart", "/etc/rc", "/.bashrc", "/Startup/")


def classify(event: dict) -> dict:
    """Clasifica un evento observado dentro de la máquina."""
    kind = event.get("kind", "")
    detail = event.get("detail", "")

    if kind == "file" and any(marker in detail for marker in PERSISTENCE_PATHS):
        return {**event, "kind": "persistence", "meaning": BEHAVIOURS["persistence"]}
    return {**event, "meaning": BEHAVIOURS.get(kind, "desconocido")}


def timeline(events: list[dict], vm_destroyed: bool) -> dict:
    """Convierte los eventos observados en un informe con veredicto.

    «La muestra no hizo nada» es un resultado, no un fallo: se anota igual. Lo
    que no puede pasar es que la máquina siga viva al terminar.
    """
    classified = [classify(event) for event in sorted(events, key=lambda item: item.get("t", 0))]
    kinds = {event["kind"] for event in classified}

    if "persistence" in kinds:
        verdict = "comportamiento de persistencia observado"
    elif "network" in kinds and "file" in kinds:
        verdict = "escribe y se conecta: caracterizar antes de aprobar"
    elif not classified:
        verdict = "no se observó actividad: puede ser inocuo o puede estar esperando"
    else:
        verdict = "actividad observada sin señales de persistencia"

    return {
        "timeline": classified,
        "verdict": verdict,
        "behaviours": sorted(kinds),
        # Una máquina huérfana con una muestra dentro es el peor fallo de este
        # caso, así que se afirma explícitamente en el informe.
        "vmDestroyed": vm_destroyed,
        "clean": vm_destroyed and not kinds,
    }


def handle(payload: dict) -> dict:
    """Punto de entrada: comprueba el equipo y clasifica la línea de tiempo.

    El informe se devuelve **siempre**, tenga este equipo KVM o no. Clasificar
    una línea de tiempo ya observada no necesita virtualización, y devolver medio
    informe según la máquina haría que la respuesta cambiase de forma según dónde
    se ejecute.
    """
    return {"preflight": preflight(), **timeline(payload.get("events", []), payload.get("vmDestroyed", True))}
