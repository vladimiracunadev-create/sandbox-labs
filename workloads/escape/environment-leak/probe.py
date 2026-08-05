#!/usr/bin/env python3
"""Sonda de contención: fuga de entorno.

El entorno del proceso es la vía de filtración más barata que existe: un
`CI=true` es inofensivo, pero un `AWS_SECRET_ACCESS_KEY` o un `GITHUB_TOKEN`
heredados convierten cualquier ejecución de código ajeno en una filtración de
credenciales.

Un runtime bien configurado limpia el entorno y solo inyecta lo que la
política declara explícitamente.
"""

from __future__ import annotations

import os
import sys

# Nombres cuya presencia con valor no vacío es una fuga, no una comodidad.
SENSITIVE_MARKERS = (
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "APIKEY",
    "API_KEY",
    "PRIVATE_KEY",
    "CREDENTIAL",
    "SESSION",
    "AWS_",
    "AZURE_",
    "GCP_",
    "GOOGLE_APPLICATION",
    "GITHUB_",
    "NPM_",
    "DOCKER_",
    "SSH_",
)

# Variables que un sandbox razonable puede inyectar sin que sea una fuga.
EXPECTED = {"PATH", "HOME", "PWD", "LANG", "LC_ALL", "TMPDIR", "SHLVL", "_", "TERM", "USER", "HOSTNAME"}


def report(probe: str, dimension: str, result: str, detail: str) -> None:
    print(f"probe={probe} dimension={dimension} result={result} detail={detail}", flush=True)


def main() -> int:
    environment = dict(os.environ)

    leaked = sorted(
        name
        for name, value in environment.items()
        if value and any(marker in name.upper() for marker in SENSITIVE_MARKERS)
    )
    if leaked:
        # Se reportan los nombres, nunca los valores: el informe de una fuga no
        # debe ser a su vez una fuga.
        report("environment-secrets", "environment", "escaped", f"variables sensibles heredadas: {','.join(leaked)}")
        return 1

    unexpected = sorted(name for name in environment if name not in EXPECTED)
    if len(unexpected) > 12:
        report(
            "environment-secrets",
            "environment",
            "escaped",
            f"{len(unexpected)} variables no declaradas heredadas del host",
        )
        return 1

    report(
        "environment-secrets",
        "environment",
        "contained",
        f"{len(environment)} variables, ninguna sensible; extra: {','.join(unexpected) or 'ninguna'}",
    )
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:  # noqa: BLE001
        report("environment-secrets", "environment", "error", f"{type(error).__name__}: {error}")
        sys.exit(2)
