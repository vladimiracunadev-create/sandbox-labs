#!/usr/bin/env python3
"""Servicio del caso 07 — Runtime determinista de contratos.

Este fichero es deliberadamente fino: publica `core.py` por HTTP y nada más. La
lógica del caso, que es lo que se puede comprobar sin levantar nada, vive allí.

Como los demás casos, escucha en un socket Unix cuando el supervisor le publica
el puerto: así el sandbox se queda **sin red** y es un reenviador de fuera quien
expone `127.0.0.1`.
"""

from __future__ import annotations

import http.server
import json
import os
import socketserver
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import core  # noqa: E402

PORT = int(os.environ.get("SANDBOX_PORT", "8807"))
RUNTIME = os.environ.get("SANDBOX_RUNTIME", "sin sandbox")
SOCKET_PATH = os.environ.get("SANDBOX_SOCKET")

# Techo del cuerpo. Un servicio sin límite de entrada es una denegación de
# servicio esperando a que alguien pegue un fichero grande.
MAX_BODY = 1024 * 1024


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.0"

    def _json(self, code: int, payload: dict) -> None:
        body = json.dumps(payload, indent=2, ensure_ascii=False).encode()
        self.send_response(code)
        self.send_header("content-type", "application/json; charset=utf-8")
        self.send_header("content-length", str(len(body)))
        self.send_header("cache-control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802
        if self.path in ("/health", "/api/status"):
            self._json(200, {"status": "ok", "case": "07", "runtime": RUNTIME})
            return
        if self.path == "/":
            self._json(200, {
                "case": "07",
                "title": "Runtime determinista de contratos",
                "teaches": "Acotar por tiempo destruye el determinismo: la máquina lenta se corta donde la rápida siguió.",
                "runtime": RUNTIME,
                "post": "/api/run con el cuerpo JSON que describe la ficha del caso",
                "docs": "docs/casos/",
            })
            return
        self._json(404, {"error": "no existe"})

    def do_POST(self) -> None:  # noqa: N802
        if self.path != "/api/run":
            self._json(404, {"error": "no existe"})
            return
        length = int(self.headers.get("content-length", "0") or 0)
        if length > MAX_BODY:
            self._json(413, {"error": f"cuerpo de más de {MAX_BODY} bytes"})
            return
        try:
            payload = json.loads(self.rfile.read(length) or b"{}")
        except json.JSONDecodeError as error:
            self._json(400, {"error": f"cuerpo JSON no válido: {error}"})
            return
        try:
            self._json(200, core.handle(payload))
        except Exception as error:  # noqa: BLE001 — el fallo se devuelve, no tumba el servicio
            self._json(400, {"error": str(error), "kind": type(error).__name__})

    def log_message(self, format: str, *args) -> None:  # noqa: A002
        print(f"{self.address_string()} {format % args}", flush=True)


class UnixServer(socketserver.ThreadingUnixStreamServer):
    """`BaseHTTPRequestHandler` pide la dirección del par y en un socket Unix no
    hay ninguna: se devuelve una etiqueta fija en vez de dejar que reviente."""

    allow_reuse_address = True

    def get_request(self):
        connection, _ = super().get_request()
        return connection, ("unix", 0)


def main() -> None:
    if SOCKET_PATH:
        try:
            os.unlink(SOCKET_PATH)
        except FileNotFoundError:
            pass
        os.makedirs(os.path.dirname(SOCKET_PATH), exist_ok=True)
        with UnixServer(SOCKET_PATH, Handler) as server:
            os.chmod(SOCKET_PATH, 0o660)
            print(f"servicio 07-deterministic-contracts en unix:{SOCKET_PATH} · runtime={RUNTIME} · sin red", flush=True)
            server.serve_forever()
        return

    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer(("127.0.0.1", PORT), Handler) as server:
        print(f"servicio 07-deterministic-contracts en 127.0.0.1:{PORT} · runtime={RUNTIME}", flush=True)
        server.serve_forever()


if __name__ == "__main__":
    main()
