#!/usr/bin/env python3
"""Runner de CI para el pull request de un desconocido.

Es el sitio donde ejecutar código ajeno con privilegios está institucionalizado:
alguien abre un pull request y, sin que nadie lo lea, arrancan las pruebas. Y la
máquina que las ejecuta suele tener el token del repositorio, las credenciales de
despliegue y las claves de firma.

La regla del caso cabe en una línea: **la etapa que ejecuta código ajeno no tiene
llaves, y la que tiene llaves no ejecuta código ajeno.**
"""

from __future__ import annotations

import re

# Nombres de variable que casi siempre llevan algo que no debe salir. Se usan
# para dos cosas distintas: comprobar que el entorno del runner está vacío, y
# tachar lo que aparezca en los registros.
SECRET_HINTS = ("TOKEN", "SECRET", "KEY", "PASSWORD", "CREDENTIAL", "PRIVATE")

# Formas reconocibles de secreto dentro de un texto, para tacharlas aunque
# lleguen sin nombre de variable.
SECRET_SHAPES = [
    re.compile(r"gh[pousr]_[A-Za-z0-9]{16,}"),
    re.compile(r"AKIA[0-9A-Z]{12,}"),
    re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
    re.compile(r"\b[A-Za-z0-9_-]{32,}\.[A-Za-z0-9_-]{16,}\.[A-Za-z0-9_-]{16,}\b"),
]


def build_environment(trusted: bool, requested_secrets: list[str]) -> dict:
    """Qué entorno recibe la etapa que ejecuta el pull request.

    Con `trusted=False` la respuesta es **siempre** un entorno vacío. No es un
    valor por defecto configurable: pedir secretos para código no confiable es
    una contradicción, y se responde con un error en vez de con una excepción
    silenciosa.
    """
    if trusted:
        return {"environment": {name: "***" for name in requested_secrets}, "secretsPresent": bool(requested_secrets)}
    if requested_secrets:
        return {
            "environment": {},
            "secretsPresent": False,
            "refused": [
                f"«{name}» no se inyecta: el código del pull request no es confiable" for name in requested_secrets
            ],
        }
    return {"environment": {}, "secretsPresent": False}


def probe_environment(environment: dict) -> list[str]:
    """Sonda: ¿queda algo que parezca un secreto dentro de la jaula?

    Comprobar que el entorno está vacío **mirándolo desde dentro** es distinto de
    confiar en que se limpió. Es la misma idea que la suite de contención.
    """
    return [name for name in environment if any(hint in name.upper() for hint in SECRET_HINTS)]


def redact(text: str) -> str:
    """Tacha lo que parezca un secreto antes de que llegue al registro.

    Un log público no se borra: si un secreto llega ahí, lo único que queda es
    rotarlo. Por eso se tacha antes de escribir, no después.
    """
    for shape in SECRET_SHAPES:
        text = shape.sub("[TACHADO]", text)
    # Y también `NOMBRE=valor` cuando el nombre delata lo que es.
    return re.sub(
        rf"\b(\w*(?:{'|'.join(SECRET_HINTS)})\w*)\s*[=:]\s*\S+",
        r"\1=[TACHADO]",
        text,
        flags=re.IGNORECASE,
    )


def network_attempt(host: str, allowlist: list[str]) -> dict:
    """Cada intento de red se registra, salga o no. La lista es el control; el
    registro es el dato."""
    allowed = host in allowlist
    return {"host": host, "outcome": "permitido" if allowed else "bloqueado por lista de permitidos"}


def run(pull_request: int, trusted: bool, requested_secrets: list[str], allowlist: list[str], attempts: list[str], logs: str) -> dict:
    """Ejecuta la etapa no confiable y devuelve el acta."""
    environment = build_environment(trusted, requested_secrets)
    leaked = probe_environment(environment["environment"])

    return {
        "pullRequest": pull_request,
        "trusted": trusted,
        "secretsPresent": environment["secretsPresent"],
        "refusedSecrets": environment.get("refused", []),
        # Si esto no está vacío con `trusted=False`, el aislamiento falló y el
        # build tiene que caerse.
        "secretsVisibleInsideCage": leaked,
        "networkAttempts": [network_attempt(host, allowlist) for host in attempts],
        "logs": redact(logs),
        # La publicación es otra etapa, con aprobación humana entre medias.
        "canPublish": False,
        "publishRequiresHumanApproval": True,
    }


def handle(payload: dict) -> dict:
    """Punto de entrada: un pull request y lo que intentó."""
    return run(
        int(payload.get("pullRequest", 0)),
        bool(payload.get("trusted", False)),
        payload.get("secrets", []),
        payload.get("allowlist", []),
        payload.get("networkAttempts", []),
        payload.get("logs", ""),
    )
