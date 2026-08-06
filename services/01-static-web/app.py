#!/usr/bin/env python3
"""Servicio 01 — Web estática servida desde dentro del sandbox.

Un servidor HTTP corriente. Lo interesante no es lo que sirve, sino desde
dónde: el proceso vive dentro de una jaula y la página muestra, en vivo, lo
que ese proceso alcanza a ver del host. Es la diferencia entre leer que un
sandbox aísla y verlo desde dentro.

Solo biblioteca estándar: el servicio tiene que arrancar dentro de una jaula
que no tiene pip ni acceso a la red.
"""

from __future__ import annotations

import html
import http.server
import json
import os
import socket
import socketserver
from pathlib import Path

PORT = int(os.environ.get("SANDBOX_PORT", "8801"))
RUNTIME = os.environ.get("SANDBOX_RUNTIME", "sin sandbox")


def visible_pids() -> int:
    try:
        return len([entry for entry in os.listdir("/proc") if entry.isdigit()])
    except OSError:
        return -1


def host_tree_visible() -> bool:
    """¿Se ve el árbol de directorios del host desde aquí dentro?"""
    for marker in ("/home", "/mnt", "/media"):
        try:
            if os.listdir(marker):
                return True
        except OSError:
            continue
    return False


def secrets_readable() -> list[str]:
    candidates = ["/etc/shadow", os.path.expanduser("~/.ssh/id_rsa"), os.path.expanduser("~/.aws/credentials")]
    found = []
    for path in candidates:
        try:
            if Path(path).read_bytes():
                found.append(path)
        except OSError:
            continue
    return found


def network_reachable() -> bool:
    try:
        with socket.create_connection(("1.1.1.1", 53), timeout=1.5):
            return True
    except OSError:
        return False


def effective_capabilities() -> str:
    try:
        for line in Path("/proc/self/status").read_text(encoding="utf-8").splitlines():
            if line.startswith("CapEff:"):
                return line.split()[1]
    except (OSError, IndexError):
        pass
    return "desconocido"


def snapshot() -> dict:
    secrets = secrets_readable()
    return {
        "runtime": RUNTIME,
        "port": PORT,
        "uid": os.getuid(),
        "pidsVisibles": visible_pids(),
        "arbolDelHostVisible": host_tree_visible(),
        "secretosLegibles": secrets,
        "redAlcanzable": network_reachable(),
        "capabilitiesEfectivas": effective_capabilities(),
        "variablesDeEntorno": sorted(os.environ.keys()),
    }


def verdicts(data: dict) -> list[tuple[str, bool, str]]:
    """(etiqueta, contenido, detalle) por dimensión."""
    return [
        ("Visibilidad de procesos", data["pidsVisibles"] <= 12, f"{data['pidsVisibles']} PIDs visibles desde dentro"),
        ("Árbol del host", not data["arbolDelHostVisible"], "no visible" if not data["arbolDelHostVisible"] else "/home o /mnt son visibles"),
        ("Secretos del host", not data["secretosLegibles"], "ninguno legible" if not data["secretosLegibles"] else ", ".join(data["secretosLegibles"])),
        ("Capabilities", data["capabilitiesEfectivas"] in ("0000000000000000", "desconocido"), f"CapEff={data['capabilitiesEfectivas']}"),
        ("Entorno", len(data["variablesDeEntorno"]) <= 12, f"{len(data['variablesDeEntorno'])} variables heredadas"),
    ]


PAGE = """<!DOCTYPE html>
<html lang="es"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="light dark">
<title>Servicio en sandbox · sandbox-labs</title>
<style>
:root{{color-scheme:light dark;--bg:#f6f8fc;--panel:#fff;--text:#16202b;--muted:#5b6879;--line:rgba(22,32,43,.14);--ok:#1f7a4f;--bad:#b23131;--accent:#3b62d9}}
@media(prefers-color-scheme:dark){{:root{{--bg:#0a0f1c;--panel:#131b2b;--text:#eef2fa;--muted:#9fadc4;--line:rgba(158,176,205,.2);--ok:#4fd39b;--bad:#f08a8a;--accent:#7d9bff}}}}
*{{box-sizing:border-box}}body{{margin:0;padding:32px 20px;font-family:"Segoe UI",system-ui,sans-serif;background:var(--bg);color:var(--text);line-height:1.55}}
.shell{{max-width:820px;margin:0 auto}}
.card{{background:var(--panel);border:1px solid var(--line);border-radius:20px;padding:28px;box-shadow:0 16px 40px rgba(22,32,43,.1)}}
h1{{margin:0 0 6px;font-size:1.9rem}}
.sub{{color:var(--muted);margin:0 0 22px}}
.pill{{display:inline-flex;align-items:center;gap:8px;padding:6px 12px;border-radius:999px;background:rgba(59,98,217,.12);color:var(--accent);font-weight:700;font-size:.82rem;margin-bottom:16px}}
table{{width:100%;border-collapse:collapse;margin-top:8px}}
td{{padding:11px 8px;border-bottom:1px solid var(--line);vertical-align:top}}
td:first-child{{font-weight:600;width:38%}}
.v{{font-weight:700;white-space:nowrap}}.ok{{color:var(--ok)}}.bad{{color:var(--bad)}}
.d{{display:block;color:var(--muted);font-weight:400;font-size:.88rem}}
.foot{{margin-top:22px;padding-top:16px;border-top:1px solid var(--line);color:var(--muted);font-size:.88rem}}
code{{background:rgba(59,98,217,.12);padding:2px 6px;border-radius:6px;font-size:.9em}}
a{{color:var(--accent)}}
</style></head><body><div class="shell"><div class="card">
<span class="pill">🛡️ runtime: {runtime}</span>
<h1>Este servicio corre dentro de un sandbox</h1>
<p class="sub">La página la sirve un proceso Python enjaulado. Lo que ves abajo es lo que
ese proceso alcanza a ver del equipo — medido ahora, no declarado en un documento.</p>
<table>{rows}</table>
<p class="foot">Puerto <code>{port}</code> · uid <code>{uid}</code> ·
<a href="/api/status">/api/status</a> devuelve lo mismo en JSON ·
<a href="/health">/health</a> para el panel</p>
</div></div></body></html>"""


class Handler(http.server.BaseHTTPRequestHandler):
    def _send(self, status: int, body: bytes, content_type: str) -> None:
        self.send_response(status)
        self.send_header("content-type", content_type)
        self.send_header("content-length", str(len(body)))
        self.send_header("cache-control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802 - firma impuesta por la stdlib
        if self.path == "/health":
            self._send(200, b'{"status":"ok"}', "application/json; charset=utf-8")
            return
        if self.path == "/api/status":
            body = json.dumps(snapshot(), indent=2, ensure_ascii=False).encode()
            self._send(200, body, "application/json; charset=utf-8")
            return

        data = snapshot()
        rows = "".join(
            f'<tr><td>{html.escape(label)}</td>'
            f'<td class="v {"ok" if contained else "bad"}">{"✅ contenido" if contained else "❌ expuesto"}'
            f'<span class="d">{html.escape(detail)}</span></td></tr>'
            for label, contained, detail in verdicts(data)
        )
        page = PAGE.format(runtime=html.escape(RUNTIME), rows=rows, port=data["port"], uid=data["uid"])
        self._send(200, page.encode(), "text/html; charset=utf-8")

    def log_message(self, format: str, *args) -> None:  # noqa: A002 - firma de la stdlib
        # Se registra en el log del servicio, que el panel muestra en vivo.
        print(f"{self.address_string()} {format % args}", flush=True)


def main() -> None:
    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer(("127.0.0.1", PORT), Handler) as server:
        print(f"servicio 01-static-web escuchando en 127.0.0.1:{PORT} (runtime={RUNTIME})", flush=True)
        server.serve_forever()


if __name__ == "__main__":
    main()
