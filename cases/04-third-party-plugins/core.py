#!/usr/bin/env python3
"""Concesión de capacidades a un plugin de terceros.

El modelo habitual es **restar**: se da acceso a todo y se quitan permisos. Falla
siempre por el mismo sitio: hay que acordarse de quitar cada uno, y basta olvidar
uno.

Aquí se **suma**. El punto de partida es nada. Cada cosa que el plugin puede
hacer existe porque alguien la concedió, y esa concesión se traduce a controles
reales del sandbox — un montaje de solo lectura, una entrada en la lista de
permitidos del proxy— no a una casilla de confianza.

Este módulo no ejecuta plugins: valida manifiestos, compila concesiones y decide
si un intento estaba autorizado. Lo hace con funciones puras para que se pueda
comprobar sin levantar nada.
"""

from __future__ import annotations

# Las capacidades que existen. Lista cerrada: un manifiesto que pida algo que no
# esté aquí se rechaza **antes** de llegar a la pantalla de aprobación, para que
# el usuario no vea nunca una petición imposible.
CAPABILITIES = {
    "read": "leer una carpeta concreta",
    "write": "escribir en una carpeta de salida",
    "net": "hablar con un host y puerto concretos",
    "clock": "leer el reloj",
    "storage": "almacenamiento propio, aislado del de otros plugins",
    "camera": "una cámara simulada",
    "secret": "un secreto con nombre",
    "events": "recibir eventos del anfitrión",
}

# Capacidades que exigen un argumento: sin él no se pueden traducir a un control.
# «Puede leer» no es una capacidad; «puede leer entrada/» sí.
NEEDS_TARGET = {"read", "write", "net", "secret"}

# Lo que nunca se concede, pida lo que pida el manifiesto. No es una lista de
# prohibidos que haya que mantener: son las dos cosas que permitirían al plugin
# ampliarse a sí mismo.
NEVER = {"grants.modify", "secrets.list"}


class ManifestError(ValueError):
    """Un manifiesto que no se puede convertir en una concesión."""


def parse_capability(raw: str) -> tuple[str, str | None]:
    """`read:entrada/` → `("read", "entrada/")`. Sin argumento → `(kind, None)`."""
    kind, _, target = raw.partition(":")
    return kind.strip(), (target.strip() or None)


def validate(manifest: dict) -> list[str]:
    """Comprueba el manifiesto. Devuelve la lista de problemas, vacía si está bien.

    Se devuelven **todos** los problemas, no el primero: arreglar un manifiesto
    de uno en uno son cinco vueltas en vez de una.
    """
    problems: list[str] = []

    if not manifest.get("id"):
        problems.append("el manifiesto no tiene id")
    if not manifest.get("version"):
        problems.append("el manifiesto no declara versión: sin ella una actualización pasa desapercibida")

    requested = manifest.get("capabilities", [])
    if not isinstance(requested, list):
        return problems + ["capabilities tiene que ser una lista"]

    for raw in requested:
        if not isinstance(raw, str):
            problems.append(f"capacidad que no es texto: {raw!r}")
            continue
        if raw in NEVER:
            problems.append(f"«{raw}» no se concede nunca: permitiría al plugin ampliarse a sí mismo")
            continue
        kind, target = parse_capability(raw)
        if kind not in CAPABILITIES:
            problems.append(f"capacidad desconocida: «{kind}»")
        elif kind in NEEDS_TARGET and target is None:
            problems.append(f"«{kind}» necesita un destino: «{kind}» a secas no se puede traducir a un control")
        elif kind == "read" and target is not None and (target.startswith("/") or ".." in target):
            problems.append(f"«{raw}» sale de la carpeta del plugin")

    return problems


def grant(manifest: dict, approved: list[str]) -> dict:
    """Compila la concesión: lo aprobado se traduce a controles reales.

    Lo pedido y no aprobado queda en `denied` con su motivo. Que se vea la
    diferencia entre «no lo pidió» y «lo pidió y no se le dio» es la mitad del
    valor del caso.
    """
    problems = validate(manifest)
    if problems:
        raise ManifestError("; ".join(problems))

    requested = manifest.get("capabilities", [])
    granted, denied, mounts, allowlist, secrets = [], [], [], [], []

    for raw in requested:
        if raw not in approved:
            denied.append({"capability": raw, "reason": "el usuario no lo aprobó"})
            continue
        kind, target = parse_capability(raw)
        granted.append(raw)
        # Aquí es donde una capacidad deja de ser una palabra.
        if kind == "read":
            mounts.append({"path": target, "mode": "ro"})
        elif kind == "write":
            mounts.append({"path": target, "mode": "rw"})
        elif kind == "net":
            allowlist.append(target)
        elif kind == "secret":
            secrets.append(target)

    return {
        "plugin": f"{manifest['id']}@{manifest['version']}",
        "granted": granted,
        "denied": denied,
        # El sandbox que sale de esto: sin red salvo lo concedido, sin reloj
        # salvo que se conceda, y con exactamente los montajes de arriba.
        "sandbox": {
            "mounts": mounts,
            "network": "allowlist" if allowlist else "none",
            "allowlist": allowlist,
            "secrets": secrets,
            "clock": "clock" in granted,
        },
    }


def check_attempt(grant_record: dict, capability: str) -> dict:
    """Qué pasa cuando el plugin intenta algo.

    Nunca hay «permiso denegado»: si la capacidad no se concedió, **no existe
    dentro de la jaula**. El intento se registra igualmente, y esa lista es el
    producto del caso.
    """
    if capability in NEVER:
        return {
            "capability": capability,
            "outcome": "prohibida",
            "detail": "no se concede nunca: el plugin no puede ampliarse a sí mismo",
        }
    if capability in grant_record["granted"]:
        return {"capability": capability, "outcome": "permitido", "detail": "estaba en la concesión"}
    return {
        "capability": capability,
        "outcome": "no concedida",
        "detail": "no existe en la jaula: no hay nada que denegar",
    }


def requires_reapproval(previous: dict, current: dict) -> bool:
    """¿La versión nueva pide algo que la anterior no pedía?

    Un plugin que se actualiza y amplía capacidades **vuelve a pedir
    aprobación**. Heredar la concesión de la versión anterior es cómo un plugin
    honesto se convierte en otra cosa sin que nadie lo note.
    """
    return bool(set(current.get("capabilities", [])) - set(previous.get("capabilities", [])))


def handle(payload: dict) -> dict:
    """Punto de entrada del servicio: manifiesto + aprobado + intentos."""
    manifest = payload.get("manifest", {})
    problems = validate(manifest)
    if problems:
        return {"valid": False, "problems": problems}

    record = grant(manifest, payload.get("approved", []))
    attempts = [check_attempt(record, capability) for capability in payload.get("attempts", [])]
    return {"valid": True, **record, "attempts": attempts}
