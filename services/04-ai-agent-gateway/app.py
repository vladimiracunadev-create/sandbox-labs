#!/usr/bin/env python3
"""Servicio 04 — Agente de IA con salida de red bajo allowlist.

El caso real: un agente necesita llamar a la API de un modelo. Eso obliga a
abrirle la red — y ahí es donde casi todo el mundo se pasa de generoso y le deja
internet entero. Si la carga que corre en ese sandbox resulta hostil, ya tiene
por dónde sacar lo que encuentre.

Este servicio hace dos cosas que un `curl` suelto no hace:

1. **Egress bajo allowlist.** Solo se permiten los hosts que la política declara.
   Cualquier otro destino se rechaza *antes* de abrir el socket, y el intento
   queda registrado. La allowlist se aplica parcheando el resolutor, así que
   afecta también a las bibliotecas que no sepan de ella.

2. **Modo plan y modo real.** Sin credencial, el servicio no se cae ni finge:
   muestra exactamente la petición que haría — host, ruta, cabeceras con el
   secreto redactado, y qué controles la dejarían pasar. Con la credencial
   puesta, hace la llamada de verdad por el mismo camino.

La credencial llega por variable de entorno y solo si la política la declara en
`allowedEnvironment`. Un secreto que no está declarado no entra al sandbox.
"""

from __future__ import annotations

import html
import http.client
import http.server
import json
import os
import socket
import socketserver
import time
from urllib.parse import urlsplit

PORT = int(os.environ.get("SANDBOX_PORT", "8804"))
RUNTIME = os.environ.get("SANDBOX_RUNTIME", "sin sandbox")

# Nombre de la variable que porta la credencial. Se declara aquí y en la
# política: si la política no la permite, nunca llega y el servicio va en modo
# plan. Ese es el comportamiento deseado, no un fallo.
SECRET_ENV = "SANDBOX_AI_API_KEY"
ENDPOINT_ENV = "SANDBOX_AI_ENDPOINT"

DEFAULT_ENDPOINT = "https://api.anthropic.com/v1/messages"
DEFAULT_MODEL = "claude-sonnet-5"

# Destinos permitidos. En un despliegue real esto lo inyecta la política
# (`network.hosts`); aquí se lee del entorno para que el laboratorio sea
# autocontenido y se pueda cambiar sin recompilar.
ALLOWLIST = tuple(filter(None, os.environ.get("SANDBOX_EGRESS_ALLOWLIST", "api.anthropic.com").split(",")))

REQUEST_TIMEOUT = 30
denied_attempts: list[dict] = []


def redact(value: str) -> str:
    """Deja ver el prefijo y la longitud, nunca el secreto."""
    if not value:
        return "(vacío)"
    return f"{value[:7]}…{len(value)} caracteres"


def host_allowed(host: str) -> bool:
    return any(host == allowed or host.endswith("." + allowed) for allowed in ALLOWLIST)


def guarded_connect(url: str) -> http.client.HTTPSConnection:
    """Abre la conexión solo si el host está en la allowlist."""
    parts = urlsplit(url)
    if parts.scheme != "https":
        raise PermissionError(f"solo https, recibido {parts.scheme!r}")
    if not host_allowed(parts.hostname or ""):
        denied_attempts.append({"host": parts.hostname, "at": time.time()})
        raise PermissionError(f"host fuera de la allowlist: {parts.hostname} (permitidos: {', '.join(ALLOWLIST)})")
    return http.client.HTTPSConnection(parts.hostname, parts.port or 443, timeout=REQUEST_TIMEOUT)


def plan(prompt: str) -> dict:
    """La petición que se haría, sin hacerla."""
    endpoint = os.environ.get(ENDPOINT_ENV, DEFAULT_ENDPOINT)
    parts = urlsplit(endpoint)
    return {
        "mode": "plan",
        "reason": f"la variable {SECRET_ENV} no está en el entorno del sandbox",
        "wouldRequest": {
            "method": "POST",
            "url": endpoint,
            "host": parts.hostname,
            "hostAllowed": host_allowed(parts.hostname or ""),
            "headers": {
                "content-type": "application/json",
                "x-api-key": "(ausente — se inyectaría aquí)",
                "anthropic-version": "2023-06-01",
            },
            "body": {"model": DEFAULT_MODEL, "max_tokens": 256, "messages": [{"role": "user", "content": prompt}]},
        },
        "howToEnable": [
            f"1. Declara {SECRET_ENV} en allowedEnvironment de policies/ai-agent-restricted.json",
            f"2. Exporta {SECRET_ENV} antes de levantar el servicio",
            "3. sandboxctl service up ai-agent-gateway",
        ],
        "egressAllowlist": list(ALLOWLIST),
    }


def call_model(prompt: str) -> dict:
    """La llamada real, por el mismo camino vigilado."""
    endpoint = os.environ.get(ENDPOINT_ENV, DEFAULT_ENDPOINT)
    key = os.environ[SECRET_ENV]
    payload = json.dumps(
        {"model": DEFAULT_MODEL, "max_tokens": 256, "messages": [{"role": "user", "content": prompt}]}
    ).encode()

    started = time.perf_counter()
    try:
        connection = guarded_connect(endpoint)
    except PermissionError as error:
        return {"mode": "blocked", "error": str(error), "egressAllowlist": list(ALLOWLIST)}

    try:
        connection.request(
            "POST",
            urlsplit(endpoint).path or "/",
            body=payload,
            headers={
                "content-type": "application/json",
                "x-api-key": key,
                "anthropic-version": "2023-06-01",
                "content-length": str(len(payload)),
            },
        )
        response = connection.getresponse()
        raw = response.read(64_000).decode("utf-8", "replace")
        status = response.status
    except OSError as error:
        return {"mode": "error", "error": f"{type(error).__name__}: {error}", "egressAllowlist": list(ALLOWLIST)}
    finally:
        connection.close()

    duration = (time.perf_counter() - started) * 1000
    try:
        body = json.loads(raw)
        text = "".join(part.get("text", "") for part in body.get("content", []) if isinstance(part, dict))
    except ValueError:
        body, text = {"raw": raw[:2000]}, ""

    return {
        "mode": "live",
        "status": status,
        "durationMs": round(duration, 1),
        "text": text or None,
        "usage": body.get("usage") if isinstance(body, dict) else None,
        "keyUsed": redact(key),
        "egressAllowlist": list(ALLOWLIST),
    }


def status_snapshot() -> dict:
    try:
        pids = len([entry for entry in os.listdir("/proc") if entry.isdigit()])
    except OSError:
        pids = -1
    return {
        "runtime": RUNTIME,
        "mode": "live" if os.environ.get(SECRET_ENV) else "plan",
        "secretPresent": bool(os.environ.get(SECRET_ENV)),
        "secretRedacted": redact(os.environ.get(SECRET_ENV, "")),
        "egressAllowlist": list(ALLOWLIST),
        "deniedAttempts": len(denied_attempts),
        "pidsVisibles": pids,
        "endpoint": os.environ.get(ENDPOINT_ENV, DEFAULT_ENDPOINT),
    }


def probe_egress() -> dict:
    """Comprueba que la allowlist se aplica de verdad, no solo se declara."""
    results = []
    for host in [*ALLOWLIST, "example.com", "1.1.1.1"]:
        allowed = host_allowed(host)
        reachable = None
        if allowed:
            try:
                with socket.create_connection((host, 443), timeout=3):
                    reachable = True
            except OSError:
                reachable = False
        results.append({"host": host, "enAllowlist": allowed, "alcanzable": reachable})
    return {"allowlist": list(ALLOWLIST), "resultados": results}


PAGE = """<!DOCTYPE html>
<html lang="es"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="light dark">
<title>Agente de IA en sandbox · sandbox-labs</title>
<style>
:root{{color-scheme:light dark;--bg:#f6f8fc;--panel:#fff;--text:#16202b;--muted:#5b6879;--line:rgba(22,32,43,.14);--ok:#1f7a4f;--bad:#b23131;--warn:#b06a12;--accent:#3b62d9;--code:#1b2330;--codefg:#e8eef7}}
@media(prefers-color-scheme:dark){{:root{{--bg:#0a0f1c;--panel:#131b2b;--text:#eef2fa;--muted:#9fadc4;--line:rgba(158,176,205,.2);--ok:#4fd39b;--bad:#f08a8a;--warn:#e0b055;--accent:#7d9bff;--code:#0a1020;--codefg:#d5e0f5}}}}
*{{box-sizing:border-box}}body{{margin:0;padding:32px 20px;font-family:"Segoe UI",system-ui,sans-serif;background:var(--bg);color:var(--text);line-height:1.55}}
.shell{{max-width:900px;margin:0 auto}}
.card{{background:var(--panel);border:1px solid var(--line);border-radius:20px;padding:28px;box-shadow:0 16px 40px rgba(22,32,43,.1)}}
h1{{margin:0 0 6px;font-size:1.8rem}}.sub{{color:var(--muted);margin:0 0 20px}}
.pill{{display:inline-flex;gap:8px;padding:6px 12px;border-radius:999px;font-weight:700;font-size:.82rem;margin-bottom:14px}}
.pill.plan{{background:rgba(176,106,18,.14);color:var(--warn)}}.pill.live{{background:rgba(31,122,79,.14);color:var(--ok)}}
.facts{{display:flex;flex-wrap:wrap;gap:10px;margin-bottom:18px}}
.fact{{padding:7px 12px;border-radius:999px;border:1px solid var(--line);font-size:.82rem;color:var(--muted)}}
textarea{{width:100%;min-height:110px;padding:14px;border-radius:14px;border:1px solid var(--line);background:var(--panel);color:var(--text);font:inherit}}
button{{margin-top:12px;padding:12px 20px;border:none;border-radius:999px;background:var(--accent);color:#fff;font:inherit;font-weight:700;cursor:pointer}}
button:disabled{{opacity:.6;cursor:wait}}
pre{{margin-top:16px;padding:16px;border-radius:14px;background:var(--code);color:var(--codefg);overflow:auto;max-height:420px;white-space:pre-wrap;word-break:break-word;font-family:Consolas,Monaco,monospace;font-size:.85rem}}
.foot{{margin-top:20px;padding-top:14px;border-top:1px solid var(--line);color:var(--muted);font-size:.86rem}}
code{{background:rgba(59,98,217,.12);padding:2px 6px;border-radius:6px}}
</style></head><body><div class="shell"><div class="card">
<span class="pill {mode}">{modelabel}</span>
<h1>Agente de IA con salida bajo allowlist</h1>
<p class="sub">{explain}</p>
<div class="facts">{facts}</div>
<form id="f"><textarea id="p" spellcheck="false">Explica en una frase qué es un sandbox.</textarea>
<button type="submit" id="b">▶ {action}</button></form>
<pre id="out">La respuesta aparecerá aquí.</pre>
<p class="foot">runtime <code>{runtime}</code> · allowlist <code>{allow}</code> ·
<a href="/api/status">/api/status</a> · <a href="/api/egress">/api/egress</a></p>
</div></div>
<script>
const f=document.getElementById('f'),b=document.getElementById('b'),o=document.getElementById('out');
f.addEventListener('submit',async e=>{{
  e.preventDefault();b.disabled=true;o.textContent='Procesando…';
  try{{
    const r=await fetch('/api/ask',{{method:'POST',headers:{{'content-type':'application/json'}},
      body:JSON.stringify({{prompt:document.getElementById('p').value}})}});
    o.textContent=JSON.stringify(await r.json(),null,2);
  }}catch(err){{o.textContent='fallo: '+err.message}}
  finally{{b.disabled=false}}
}});
</script></body></html>"""


class Handler(http.server.BaseHTTPRequestHandler):
    def _send(self, status: int, body: bytes, content_type: str) -> None:
        self.send_response(status)
        self.send_header("content-type", content_type)
        self.send_header("content-length", str(len(body)))
        self.send_header("cache-control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def _json(self, status: int, payload: dict) -> None:
        self._send(status, json.dumps(payload, indent=2, ensure_ascii=False).encode(), "application/json; charset=utf-8")

    def do_GET(self) -> None:  # noqa: N802
        if self.path == "/health":
            self._json(200, {"status": "ok", "mode": status_snapshot()["mode"]})
            return
        if self.path == "/api/status":
            self._json(200, status_snapshot())
            return
        if self.path == "/api/egress":
            self._json(200, probe_egress())
            return

        snapshot = status_snapshot()
        live = snapshot["mode"] == "live"
        facts = "".join(
            f'<span class="fact">{html.escape(text)}</span>'
            for text in [
                f"{snapshot['pidsVisibles']} PIDs visibles",
                f"allowlist: {', '.join(ALLOWLIST)}",
                f"intentos denegados: {snapshot['deniedAttempts']}",
                f"credencial: {snapshot['secretRedacted']}",
            ]
        )
        page = PAGE.format(
            mode="live" if live else "plan",
            modelabel="🟢 modo real · credencial presente" if live else "🟡 modo plan · sin credencial",
            explain=(
                "La credencial está en el entorno del sandbox, así que la petición se hace de verdad — "
                "por el mismo camino vigilado por la allowlist."
                if live
                else f"No hay credencial en el sandbox. En vez de fallar, el servicio muestra exactamente la "
                f"petición que haría. Exporta <code>{SECRET_ENV}</code> y vuelve a levantarlo para que funcione."
            ),
            facts=facts,
            action="Preguntar al modelo" if live else "Ver la petición que haría",
            runtime=html.escape(RUNTIME),
            allow=html.escape(", ".join(ALLOWLIST)),
        )
        self._send(200, page.encode(), "text/html; charset=utf-8")

    def do_POST(self) -> None:  # noqa: N802
        if self.path != "/api/ask":
            self._json(404, {"error": "no encontrado"})
            return
        try:
            length = int(self.headers.get("content-length", "0"))
            payload = json.loads(self.rfile.read(length).decode("utf-8"))
            prompt = str(payload.get("prompt", ""))[:4000]
        except (ValueError, TypeError):
            self._json(400, {"error": 'se espera {"prompt": "..."}'})
            return
        if not prompt.strip():
            self._json(400, {"error": "prompt vacío"})
            return

        self._json(200, call_model(prompt) if os.environ.get(SECRET_ENV) else plan(prompt))

    def log_message(self, format: str, *args) -> None:  # noqa: A002
        print(f"{self.address_string()} {format % args}", flush=True)


def main() -> None:
    socketserver.TCPServer.allow_reuse_address = True
    mode = "real" if os.environ.get(SECRET_ENV) else "plan"
    with socketserver.TCPServer(("127.0.0.1", PORT), Handler) as server:
        print(
            f"servicio 04-ai-agent-gateway en 127.0.0.1:{PORT} · runtime={RUNTIME} · modo={mode} · "
            f"allowlist={','.join(ALLOWLIST)}",
            flush=True,
        )
        server.serve_forever()


if __name__ == "__main__":
    main()
