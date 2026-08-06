#!/usr/bin/env python3
"""Servicio 02 — API de contención.

Expone como API lo que las sondas miden de un disparo. La diferencia importa:
un servicio largo permite comprobar la contención **mientras algo corre**, que
es cuando de verdad hace falta. Un sandbox que aísla al arrancar y se degrada a
los diez minutos pasaría desapercibido con una medición puntual.

Endpoints:
  GET /health                    sonda de vida para el panel
  GET /api/containment           veredicto por dimensión
  GET /api/probe/network         intenta salir a la red ahora
  GET /api/probe/filesystem      intenta leer secretos del host ahora
  GET /api/probe/processes       enumera lo que ve del árbol de procesos
"""

from __future__ import annotations

import http.server
import json
import os
import socket
import socketserver
import time
from pathlib import Path

PORT = int(os.environ.get("SANDBOX_PORT", "8802"))
RUNTIME = os.environ.get("SANDBOX_RUNTIME", "sin sandbox")
STARTED = time.time()

SECRET_PATHS = [
    "/etc/shadow",
    os.path.expanduser("~/.ssh/id_rsa"),
    os.path.expanduser("~/.aws/credentials"),
    os.path.expanduser("~/.config/gh/hosts.yml"),
]


def probe_network() -> dict:
    targets = [("1.1.1.1", 53), ("8.8.8.8", 53)]
    reached = []
    for host, port in targets:
        try:
            with socket.create_connection((host, port), timeout=1.5):
                reached.append(f"{host}:{port}")
        except OSError:
            continue
    dns = True
    try:
        socket.setdefaulttimeout(1.5)
        socket.gethostbyname("example.com")
    except OSError:
        dns = False
    return {
        "dimension": "network",
        "contained": not reached and not dns,
        "tcpAlcanzado": reached,
        "dnsResuelve": dns,
        "detalle": "sin salida TCP ni DNS" if not reached and not dns else f"alcanzado: {reached or 'DNS'}",
    }


def probe_filesystem() -> dict:
    readable = []
    for path in SECRET_PATHS:
        try:
            if Path(path).read_bytes():
                readable.append(path)
        except OSError:
            continue
    host_dirs = []
    for marker in ("/home", "/mnt", "/media", "/root"):
        try:
            entries = os.listdir(marker)
            if entries:
                host_dirs.append(f"{marker} ({len(entries)})")
        except OSError:
            continue
    return {
        "dimension": "filesystem",
        "contained": not readable and not host_dirs,
        "secretosLegibles": readable,
        "directoriosDelHost": host_dirs,
        "detalle": "nada del host es visible" if not readable and not host_dirs else "el host está expuesto",
    }


def probe_processes() -> dict:
    try:
        pids = sorted(int(entry) for entry in os.listdir("/proc") if entry.isdigit())
    except OSError:
        pids = []
    return {
        "dimension": "processes",
        "contained": 0 < len(pids) <= 12,
        "pidsVisibles": len(pids),
        "propioPid": os.getpid(),
        "detalle": f"{len(pids)} PIDs visibles",
    }


def probe_privilege() -> dict:
    caps = "desconocido"
    try:
        for line in Path("/proc/self/status").read_text(encoding="utf-8").splitlines():
            if line.startswith("CapEff:"):
                caps = line.split()[1]
                break
    except (OSError, IndexError):
        pass
    return {
        "dimension": "privilege",
        "contained": caps in ("0000000000000000", "desconocido"),
        "capEff": caps,
        "uid": os.getuid(),
        "detalle": f"CapEff={caps}, uid={os.getuid()}",
    }


def probe_environment() -> dict:
    markers = ("TOKEN", "SECRET", "PASSWORD", "KEY", "AWS_", "GITHUB_")
    leaked = sorted(name for name, value in os.environ.items() if value and any(m in name.upper() for m in markers))
    return {
        "dimension": "environment",
        "contained": not leaked,
        "variables": len(os.environ),
        "sensiblesHeredadas": leaked,
        "detalle": f"{len(os.environ)} variables, {len(leaked)} sensibles",
    }


PROBES = {
    "network": probe_network,
    "filesystem": probe_filesystem,
    "processes": probe_processes,
    "privilege": probe_privilege,
    "environment": probe_environment,
}


def containment() -> dict:
    results = [probe() for probe in PROBES.values()]
    return {
        "runtime": RUNTIME,
        "uptimeSegundos": round(time.time() - STARTED, 1),
        "contenidas": sum(1 for value in results if value["contained"]),
        "total": len(results),
        "dimensiones": results,
    }


class Handler(http.server.BaseHTTPRequestHandler):
    def _json(self, status: int, payload: dict) -> None:
        body = json.dumps(payload, indent=2, ensure_ascii=False).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json; charset=utf-8")
        self.send_header("content-length", str(len(body)))
        self.send_header("cache-control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802
        if self.path == "/health":
            self._json(200, {"status": "ok", "runtime": RUNTIME})
            return
        if self.path in ("/", "/api/containment"):
            self._json(200, containment())
            return
        if self.path.startswith("/api/probe/"):
            name = self.path.rsplit("/", 1)[-1]
            probe = PROBES.get(name)
            if not probe:
                self._json(404, {"error": "sonda desconocida", "disponibles": sorted(PROBES)})
                return
            self._json(200, probe())
            return
        self._json(404, {"error": "no encontrado"})

    def log_message(self, format: str, *args) -> None:  # noqa: A002
        print(f"{self.address_string()} {format % args}", flush=True)


def main() -> None:
    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer(("127.0.0.1", PORT), Handler) as server:
        print(f"servicio 02-containment-api escuchando en 127.0.0.1:{PORT} (runtime={RUNTIME})", flush=True)
        server.serve_forever()


if __name__ == "__main__":
    main()
