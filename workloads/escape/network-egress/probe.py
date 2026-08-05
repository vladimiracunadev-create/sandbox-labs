#!/usr/bin/env python3
"""Sonda de contención: salida de red.

Intenta abrir conexiones salientes desde dentro del sandbox. Si alguna
prospera, el runtime NO está conteniendo la red y hay que saberlo.

Contrato de salida (una línea por sonda, parseada por `sandboxctl escape`):

    probe=<id> dimension=<dim> result=<contained|escaped|error> detail=<texto>

La sonda no ataca a nadie: los destinos son resolutores públicos de DNS y el
bucle local, y el objetivo es medir el propio host, no alcanzar un tercero.
"""

from __future__ import annotations

import socket
import sys

TIMEOUT = 2.0

# Destinos representativos: DNS público por IP (no depende de resolución),
# DNS público alternativo, y el propio loopback del host.
TARGETS = [
    ("1.1.1.1", 53, "dns-cloudflare"),
    ("8.8.8.8", 53, "dns-google"),
    ("127.0.0.1", 22, "loopback-ssh"),
]


def report(result: str, detail: str) -> None:
    print(f"probe=network-egress dimension=network result={result} detail={detail}", flush=True)


def try_connect(host: str, port: int) -> str | None:
    """Devuelve None si la conexión fue bloqueada, o una descripción si prosperó."""
    try:
        with socket.create_connection((host, port), timeout=TIMEOUT):
            return f"{host}:{port}"
    except OSError:
        return None


def main() -> int:
    reached = [name for host, port, name in TARGETS if try_connect(host, port)]

    if reached:
        report("escaped", f"conexiones establecidas: {','.join(reached)}")
        return 1

    # Resolución DNS por separado: un runtime puede cortar el tráfico y dejar
    # el resolutor accesible, lo que sigue filtrando información.
    try:
        socket.setdefaulttimeout(TIMEOUT)
        socket.gethostbyname("example.com")
        report("escaped", "sin conexiones TCP pero la resolución DNS funciona")
        return 1
    except OSError:
        pass

    report("contained", "sin salida TCP ni resolución DNS")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:  # noqa: BLE001 - una sonda nunca debe romper la suite
        report("error", f"{type(error).__name__}: {error}")
        sys.exit(2)
