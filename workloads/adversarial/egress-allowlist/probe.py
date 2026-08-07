#!/usr/bin/env python3
"""Carga que usa el canal de salida filtrado, y otro que no le corresponde.

Demuestra las tres cosas que hacen del canal un control y no una promesa:

1. La carga **no tiene red**. Un `socket.create_connection` directo falla
   antes de salir de su namespace.
2. Lo que la política autoriza sí atraviesa el canal.
3. Lo que no autoriza recibe un `403` — y queda registrado igual, que es lo
   que permite auditarlo después.

Los destinos se pasan por argumento, así que la carga no sabe cuál es cuál: lo
decide la política, no ella.
"""

from __future__ import annotations

import os
import socket
import sys

SOCKET = os.environ.get("SANDBOX_EGRESS_SOCKET")


def direct(target: str) -> str:
    """Intenta salir SIN el canal. Debe fallar: la carga no tiene red."""
    host, port = target.rsplit(":", 1)
    try:
        with socket.create_connection((host, int(port)), timeout=3):
            return "conectó"
    except OSError as error:
        return f"{type(error).__name__}"


def through_channel(target: str) -> tuple[str, str]:
    """Pide `target` por el canal. Devuelve (código, cuerpo recibido)."""
    if not SOCKET:
        return ("sin-canal", "")
    client = socket.socket(socket.AF_UNIX)
    try:
        client.connect(SOCKET)
        client.sendall(f"CONNECT {target} HTTP/1.1\r\n\r\n".encode())
        status = client.recv(128).decode(errors="replace").split("\r\n", 1)[0]
        code = status.split(" ")[1] if " " in status else "?"
        if code != "200":
            return (code, "")
        # El canal quedó empalmado con el destino: lo que se escriba a partir de
        # aquí va al otro lado.
        client.sendall(b"ping")
        client.shutdown(socket.SHUT_WR)
        received = b""
        while True:
            chunk = client.recv(4096)
            if not chunk:
                break
            received += chunk
        return (code, received.decode(errors="replace"))
    except OSError as error:
        return (f"error:{type(error).__name__}", "")
    finally:
        client.close()


def main() -> int:
    if len(sys.argv) < 3:
        print("uso: probe.py <destino-permitido> <destino-denegado>", flush=True)
        return 2
    allowed, denied = sys.argv[1], sys.argv[2]

    print(f"canal={'sí' if SOCKET else 'no'}", flush=True)
    print(f"sin-canal={direct(allowed)}", flush=True)

    code, body = through_channel(allowed)
    print(f"permitido={code} respuesta={body!r}", flush=True)

    code, _ = through_channel(denied)
    print(f"denegado={code}", flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
