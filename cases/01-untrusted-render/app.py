#!/usr/bin/env python3
"""Servicio 01 — Contenido web no confiable, interpretado por un proceso aparte.

Este servicio hace **trabajo real**: le pegas contenido que no controlas —HTML o
Markdown de un correo, de una fuente RSS, de un formulario— y te devuelve una
vista segura y un informe de qué intentó hacer.

# El problema que resuelve

Interpretar contenido ajeno es ejecutar la lógica de otro. Un parser de HTML
tiene entidades, referencias externas, rutas y, en cuanto entra una plantilla,
expresiones. Los ataques clásicos no son teóricos:

- **XXE**: una entidad externa que hace que el parser lea `/etc/passwd` y lo
  devuelva en la respuesta.
- **SSRF**: una referencia a `http://169.254.169.254/` que hace que el servidor
  pida credenciales de la nube por ti.
- **Path traversal**: `<img src="file:///home/tú/.ssh/id_rsa">`.

Ninguno necesita ejecutar JavaScript. Solo necesita que quien interpreta el
contenido tenga acceso a algo.

# La idea que enseña: separar por proceso

Aquí hay **dos** roles, y esa separación es todo el caso:

    coordinador                        intérprete
    (este proceso)                     (proceso hijo)
    - conoce el filesystem             - NO tiene filesystem
    - habla por el socket              - NO tiene red
    - valida lo que sale               - solo stdin -> stdout
    - decide qué se permite            - no decide nada

El intérprete recibe el contenido por la entrada estándar y devuelve JSON por la
salida. No abre ficheros, no resuelve nombres, no hace peticiones. Si el
contenido le pide algo de eso, **no falla con "permiso denegado": es que la
capacidad no existe**, y el intento queda registrado.

Que el intérprete sea un proceso aparte y no una función es lo que hace que un
fallo del parser sea un fallo del intérprete y no del servicio.
"""

from __future__ import annotations

import http.server
import json
import os
import socketserver
import subprocess
import sys
import time
from pathlib import Path

PORT = int(os.environ.get("SANDBOX_PORT", "8801"))
RUNTIME = os.environ.get("SANDBOX_RUNTIME", "sin sandbox")
SOCKET_PATH = os.environ.get("SANDBOX_SOCKET")

# Tamaño máximo del contenido. Un parser sin techo de entrada es una denegación
# de servicio esperando a que alguien pegue un fichero grande.
MAX_CONTENT = 256 * 1024

# Cuánto se le da al intérprete antes de matarlo. Un contenido puede hacer que
# un parser tarde mucho sin llegar a colgarse — «ReDoS» es exactamente eso.
INTERPRETER_TIMEOUT = 5

INTERPRETER = Path(__file__).with_name("interpreter.py")


def interpret(content: str) -> dict:
    """Lanza el intérprete en un proceso aparte y recoge su informe.

    El contenido va por **stdin**, nunca por argumento: un argumento aparece en
    `ps` y en los logs del sistema, y aquí puede traer datos de un tercero.
    """
    started = time.monotonic()
    try:
        finished = subprocess.run(
            [sys.executable, str(INTERPRETER)],
            input=content,
            capture_output=True,
            text=True,
            timeout=INTERPRETER_TIMEOUT,
            # Entorno vacío: el intérprete no necesita saber nada del host, y
            # una variable heredada es una filtración esperando a un parser
            # curioso.
            env={"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"},
        )
    except subprocess.TimeoutExpired:
        return {
            "ok": False,
            "reason": f"el intérprete no terminó en {INTERPRETER_TIMEOUT}s y se le cortó",
            "elapsedMs": int((time.monotonic() - started) * 1000),
        }
    except OSError as error:
        return {"ok": False, "reason": f"no se pudo lanzar el intérprete: {error}", "elapsedMs": 0}

    elapsed = int((time.monotonic() - started) * 1000)
    if finished.returncode != 0:
        # Un intérprete que revienta es un intérprete que revienta: el
        # coordinador sigue vivo. Es la razón de que sean dos procesos.
        return {
            "ok": False,
            "reason": f"el intérprete terminó con código {finished.returncode}",
            "stderr": finished.stderr[:500],
            "elapsedMs": elapsed,
        }
    try:
        report = json.loads(finished.stdout)
    except json.JSONDecodeError:
        return {"ok": False, "reason": "el intérprete no devolvió JSON válido", "elapsedMs": elapsed}

    report["ok"] = True
    report["elapsedMs"] = elapsed
    return report


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
            self._json(200, {"status": "ok", "runtime": RUNTIME})
            return
        if self.path == "/":
            body = PAGE.encode()
            self.send_response(200)
            self.send_header("content-type", "text/html; charset=utf-8")
            self.send_header("content-length", str(len(body)))
            # El contenido que se muestra viene de un tercero: sin scripts.
            self.send_header("content-security-policy", "default-src 'none'; style-src 'unsafe-inline'")
            self.end_headers()
            self.wfile.write(body)
            return
        self._json(404, {"error": "no existe"})

    def do_POST(self) -> None:  # noqa: N802
        if self.path != "/api/render":
            self._json(404, {"error": "no existe"})
            return
        length = int(self.headers.get("content-length", "0") or 0)
        if length > MAX_CONTENT:
            self._json(413, {"error": f"contenido de más de {MAX_CONTENT} bytes"})
            return
        raw = self.rfile.read(length).decode("utf-8", errors="replace")
        try:
            content = json.loads(raw).get("content", "")
        except json.JSONDecodeError:
            content = raw
        if not content.strip():
            self._json(400, {"error": "no hay contenido que interpretar"})
            return
        self._json(200, interpret(content))

    def log_message(self, format: str, *args) -> None:  # noqa: A002
        print(f"{self.address_string()} {format % args}", flush=True)


PAGE = """<!doctype html><meta charset="utf-8"><title>01 · Contenido no confiable</title>
<style>body{font:15px system-ui;max-width:52rem;margin:2rem auto;padding:0 1rem;line-height:1.6}
textarea{width:100%;height:9rem;font:13px ui-monospace}pre{background:#f4f4f5;padding:1rem;overflow:auto}
button{padding:.6rem 1.2rem;font:inherit}</style>
<h1>Contenido web no confiable</h1>
<p>Pega HTML o Markdown de origen desconocido. Lo interpreta un <b>proceso aparte
sin filesystem ni red</b>, y abajo verás qué intentó hacer.</p>
<textarea id="c">&lt;!DOCTYPE r [&lt;!ENTITY x SYSTEM "file:///etc/passwd"&gt;]&gt;
&lt;p&gt;Hola &amp;x;&lt;/p&gt;
&lt;img src="http://169.254.169.254/latest/meta-data/"&gt;
&lt;a href="file:///home/usuario/.ssh/id_rsa"&gt;mira esto&lt;/a&gt;</textarea>
<p><button onclick="go()">Interpretar</button></p>
<pre id="o">…</pre>
<script>
async function go(){
  const r = await fetch('/api/render',{method:'POST',headers:{'content-type':'application/json'},
    body: JSON.stringify({content: document.getElementById('c').value})});
  document.getElementById('o').textContent = JSON.stringify(await r.json(), null, 2);
}
</script>
"""


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
        try:
            os.unlink(SOCKET_PATH)
        except FileNotFoundError:
            pass
        os.makedirs(os.path.dirname(SOCKET_PATH), exist_ok=True)
        with UnixServer(SOCKET_PATH, Handler) as server:
            os.chmod(SOCKET_PATH, 0o660)
            print(f"servicio 01-untrusted-render en unix:{SOCKET_PATH} · runtime={RUNTIME} · sin red", flush=True)
            server.serve_forever()
        return

    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer(("127.0.0.1", PORT), Handler) as server:
        print(f"servicio 01-untrusted-render en 127.0.0.1:{PORT} · runtime={RUNTIME}", flush=True)
        server.serve_forever()


if __name__ == "__main__":
    main()
