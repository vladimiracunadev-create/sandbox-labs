#!/usr/bin/env python3
"""Servicio 03 — Ejecutor de código generado por IA.

El caso de uso que motiva todo el repositorio, funcionando: un endpoint al que
se le manda un fragmento de Python y lo ejecuta **dentro del sandbox donde ya
corre este servicio**, con timeout y salida acotada.

Por qué esto es defendible y un `eval()` no lo es: el proceso que ejecuta el
fragmento ya está enjaulado — sin red, sin ver el filesystem del host, sin
capabilities, con los PIDs propios. El fragmento hereda esa jaula. Si el
sandbox falla, el endpoint no es lo que falla; y la propia página lo dice
mostrando en vivo qué contiene la jaula.

  GET  /                 formulario
  GET  /health           sonda de vida
  POST /api/run          {"code": "..."} → stdout, stderr, código y duración
"""

from __future__ import annotations

import html
import http.server
import json
import os
import socketserver
import subprocess
import sys
import tempfile
import time
from pathlib import Path

PORT = int(os.environ.get("SANDBOX_PORT", "8802"))
RUNTIME = os.environ.get("SANDBOX_RUNTIME", "sin sandbox")
# Cuando el supervisor publica el puerto por nosotros, el servicio escucha en
# un socket Unix y el sandbox se queda SIN red. Sin esta variable se cae al
# modo TCP de siempre, que necesita la red del host.
SOCKET_PATH = os.environ.get("SANDBOX_SOCKET")

MAX_CODE_BYTES = 8 * 1024
MAX_OUTPUT_CHARS = 8 * 1024
TIMEOUT_SECONDS = 5


def containment_summary() -> dict:
    """Lo que la jaula contiene ahora mismo, para mostrarlo junto al formulario."""
    try:
        pids = len([entry for entry in os.listdir("/proc") if entry.isdigit()])
    except OSError:
        pids = -1
    host_visible = False
    for marker in ("/home", "/mnt", "/root"):
        try:
            if os.listdir(marker):
                host_visible = True
                break
        except OSError:
            continue
    caps = "desconocido"
    try:
        for line in Path("/proc/self/status").read_text(encoding="utf-8").splitlines():
            if line.startswith("CapEff:"):
                caps = line.split()[1]
                break
    except (OSError, IndexError):
        pass
    return {"pids": pids, "hostVisible": host_visible, "capEff": caps, "runtime": RUNTIME}


def run_snippet(code: str) -> dict:
    """Ejecuta el fragmento en un proceso hijo, dentro de la misma jaula."""
    if len(code.encode("utf-8")) > MAX_CODE_BYTES:
        return {"error": "el fragmento supera el tamaño máximo", "maxBytes": MAX_CODE_BYTES}

    with tempfile.TemporaryDirectory() as workdir:
        script = Path(workdir) / "snippet.py"
        script.write_text(code, encoding="utf-8")
        started = time.perf_counter()
        try:
            completed = subprocess.run(
                [sys.executable, "-I", str(script)],
                capture_output=True,
                text=True,
                timeout=TIMEOUT_SECONDS,
                cwd=workdir,
                # El fragmento no hereda ni el entorno del servicio: dentro de
                # una jaula ya limpia, esto es cinturón sobre tirantes, pero el
                # coste es cero y la garantía deja de depender de una sola capa.
                env={"PATH": "/usr/local/bin:/usr/bin:/bin", "HOME": workdir, "LANG": "C.UTF-8"},
                check=False,
            )
        except subprocess.TimeoutExpired:
            return {
                "timedOut": True,
                "timeoutSeconds": TIMEOUT_SECONDS,
                "detalle": f"el fragmento superó {TIMEOUT_SECONDS} s y se terminó",
            }
        duration = (time.perf_counter() - started) * 1000

    return {
        "exitCode": completed.returncode,
        "durationMs": round(duration, 1),
        "stdout": completed.stdout[:MAX_OUTPUT_CHARS],
        "stderr": completed.stderr[:MAX_OUTPUT_CHARS],
        "truncated": len(completed.stdout) > MAX_OUTPUT_CHARS or len(completed.stderr) > MAX_OUTPUT_CHARS,
    }


EXAMPLE = """# El fragmento hereda la jaula del servicio. Pruébalo:
import os, socket

print("PIDs visibles:", len([p for p in os.listdir('/proc') if p.isdigit()]))
try:
    socket.create_connection(("1.1.1.1", 53), timeout=2)
    print("red: ALCANZABLE")
except OSError as e:
    print("red: bloqueada ->", type(e).__name__)
try:
    print(open("/etc/shadow").read()[:40])
except OSError as e:
    print("/etc/shadow: bloqueado ->", type(e).__name__)
"""

PAGE = """<!DOCTYPE html>
<html lang="es"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="light dark">
<title>Ejecutor de código en sandbox · sandbox-labs</title>
<style>
:root{{color-scheme:light dark;--bg:#f6f8fc;--panel:#fff;--text:#16202b;--muted:#5b6879;--line:rgba(22,32,43,.14);--ok:#1f7a4f;--bad:#b23131;--accent:#3b62d9;--code:#1b2330;--codefg:#e8eef7}}
@media(prefers-color-scheme:dark){{:root{{--bg:#0a0f1c;--panel:#131b2b;--text:#eef2fa;--muted:#9fadc4;--line:rgba(158,176,205,.2);--ok:#4fd39b;--bad:#f08a8a;--accent:#7d9bff;--code:#0a1020;--codefg:#d5e0f5}}}}
*{{box-sizing:border-box}}body{{margin:0;padding:32px 20px;font-family:"Segoe UI",system-ui,sans-serif;background:var(--bg);color:var(--text);line-height:1.55}}
.shell{{max-width:900px;margin:0 auto}}
.card{{background:var(--panel);border:1px solid var(--line);border-radius:20px;padding:28px;box-shadow:0 16px 40px rgba(22,32,43,.1)}}
h1{{margin:0 0 6px;font-size:1.8rem}}.sub{{color:var(--muted);margin:0 0 20px}}
.pill{{display:inline-flex;gap:8px;padding:6px 12px;border-radius:999px;background:rgba(59,98,217,.12);color:var(--accent);font-weight:700;font-size:.82rem;margin-bottom:14px}}
.facts{{display:flex;flex-wrap:wrap;gap:10px;margin-bottom:20px}}
.fact{{padding:7px 12px;border-radius:999px;border:1px solid var(--line);font-size:.82rem}}
.fact.ok{{color:var(--ok);border-color:rgba(31,122,79,.35);background:rgba(31,122,79,.1);font-weight:600}}
.fact.bad{{color:var(--bad);border-color:rgba(178,49,49,.35);background:rgba(178,49,49,.1);font-weight:600}}
textarea{{width:100%;min-height:230px;padding:14px;border-radius:14px;border:1px solid var(--line);background:var(--code);color:var(--codefg);font-family:Consolas,Monaco,monospace;font-size:.88rem;line-height:1.5}}
button{{margin-top:14px;padding:12px 20px;border:none;border-radius:999px;background:var(--accent);color:#fff;font:inherit;font-weight:700;cursor:pointer}}
button:disabled{{opacity:.6;cursor:wait}}
pre{{margin-top:18px;padding:16px;border-radius:14px;background:var(--code);color:var(--codefg);overflow:auto;max-height:340px;white-space:pre-wrap;word-break:break-word;font-family:Consolas,Monaco,monospace;font-size:.86rem}}
.foot{{margin-top:20px;padding-top:14px;border-top:1px solid var(--line);color:var(--muted);font-size:.86rem}}
code{{background:rgba(59,98,217,.12);padding:2px 6px;border-radius:6px}}
</style></head><body><div class="shell"><div class="card">
<span class="pill">🤖 runtime: {runtime}</span>
<h1>Ejecuta código dentro del sandbox</h1>
<p class="sub">El fragmento se ejecuta en un proceso hijo <b>dentro de esta misma jaula</b>,
con {timeout} s de límite y salida acotada. No hay <code>eval()</code>: hay un sandbox.</p>
<div class="facts">{facts}</div>
<form id="f"><textarea id="code" spellcheck="false">{example}</textarea>
<button type="submit" id="b">▶ Ejecutar en el sandbox</button></form>
<pre id="out">La salida aparecerá aquí.</pre>
<p class="foot">Límites: {maxkb} KB de código · {timeout} s de ejecución · 8 KB de salida ·
<code>POST /api/run</code> acepta <code>{{"code":"..."}}</code></p>
</div></div>
<script>
const f=document.getElementById('f'),b=document.getElementById('b'),o=document.getElementById('out');
f.addEventListener('submit',async e=>{{
  e.preventDefault();b.disabled=true;o.textContent='Ejecutando…';
  try{{
    const r=await fetch('/api/run',{{method:'POST',headers:{{'content-type':'application/json'}},
      body:JSON.stringify({{code:document.getElementById('code').value}})}});
    const d=await r.json();
    o.textContent=d.error?('error: '+d.error)
      :d.timedOut?('⏱ '+d.detalle)
      :`exit=${{d.exitCode}} · ${{d.durationMs}} ms\\n\\n--- stdout ---\\n${{d.stdout||'(vacío)'}}\\n--- stderr ---\\n${{d.stderr||'(vacío)'}}`;
  }}catch(err){{o.textContent='fallo de red: '+err.message}}
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
            self._json(200, {"status": "ok", "runtime": RUNTIME})
            return
        if self.path == "/api/containment":
            self._json(200, containment_summary())
            return
        summary = containment_summary()
        facts = "".join(
            f'<span class="fact {cls}">{html.escape(text)}</span>'
            for cls, text in [
                ("ok" if summary["pids"] <= 12 else "bad", f"{summary['pids']} PIDs visibles"),
                ("ok" if not summary["hostVisible"] else "bad",
                 "árbol del host oculto" if not summary["hostVisible"] else "árbol del host VISIBLE"),
                ("ok" if summary["capEff"] in ("0000000000000000", "desconocido") else "bad",
                 f"CapEff={summary['capEff']}"),
            ]
        )
        page = PAGE.format(
            runtime=html.escape(RUNTIME), facts=facts, example=html.escape(EXAMPLE),
            timeout=TIMEOUT_SECONDS, maxkb=MAX_CODE_BYTES // 1024,
        )
        self._send(200, page.encode(), "text/html; charset=utf-8")

    def do_POST(self) -> None:  # noqa: N802
        if self.path != "/api/run":
            self._json(404, {"error": "no encontrado"})
            return
        try:
            length = int(self.headers.get("content-length", "0"))
        except ValueError:
            length = 0
        if length <= 0 or length > MAX_CODE_BYTES * 2:
            self._json(413, {"error": "cuerpo ausente o demasiado grande"})
            return
        try:
            payload = json.loads(self.rfile.read(length).decode("utf-8"))
            code = payload["code"]
            if not isinstance(code, str):
                raise TypeError
        except (ValueError, KeyError, TypeError):
            self._json(400, {"error": 'se espera {"code": "..."} en JSON'})
            return
        self._json(200, run_snippet(code))

    def log_message(self, format: str, *args) -> None:  # noqa: A002
        print(f"{self.address_string()} {format % args}", flush=True)


class UnixServer(socketserver.ThreadingUnixStreamServer):
    """Servidor HTTP sobre socket Unix.

    `BaseHTTPRequestHandler` espera poder pedir la dirección del par; en un
    socket Unix no hay ninguna, así que se devuelve una etiqueta fija en vez de
    dejar que reviente al registrar la petición.
    """

    allow_reuse_address = True

    def get_request(self):
        connection, _ = super().get_request()
        return connection, ("unix", 0)


def main() -> None:
    if SOCKET_PATH:
        # Un socket huérfano de una ejecución anterior impediría enlazar.
        try:
            os.unlink(SOCKET_PATH)
        except FileNotFoundError:
            pass
        os.makedirs(os.path.dirname(SOCKET_PATH), exist_ok=True)
        with UnixServer(SOCKET_PATH, Handler) as server:
            os.chmod(SOCKET_PATH, 0o660)
            print(f"servicio 02-code-runner en unix:{SOCKET_PATH} · runtime={RUNTIME} · sin red", flush=True)
            server.serve_forever()
        return

    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer(("127.0.0.1", PORT), Handler) as server:
        print(f"servicio 02-code-runner en 127.0.0.1:{PORT} · runtime={RUNTIME}", flush=True)
        server.serve_forever()


if __name__ == "__main__":
    main()
