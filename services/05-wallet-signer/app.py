#!/usr/bin/env python3
"""Servicio 05 — Firma de transacciones con la clave dentro del sandbox.

El caso real: una wallet o un custodio necesita firmar transacciones. La clave
privada no puede salir del proceso que firma, y ese proceso no debería tener
red — porque una clave privada más una conexión a internet es exactamente la
receta de un robo de fondos.

Este servicio muestra el patrón que usan de verdad los custodios:

- La clave vive **solo dentro del sandbox**, inyectada por la política.
- El sandbox **no tiene red saliente**: el único camino es el loopback por el
  que entra la petición de firma. Si alguien logra ejecutar código aquí, tiene
  la clave pero no por dónde sacarla.
- Fuera solo salen firmas, nunca material de clave. La API responde con la
  firma y el hash; la clave no aparece en ninguna respuesta ni en los logs.
- Cada firma queda registrada con su hash, para que exista una traza.

La firma es HMAC-SHA256 sobre la transacción canonicalizada. **No es un esquema
de firma de blockchain real** (eso sería secp256k1/Ed25519 y saca el foco del
laboratorio): lo que se demuestra aquí es la arquitectura de aislamiento de la
clave, que es idéntica con cualquier algoritmo.
"""

from __future__ import annotations

import hashlib
import hmac
import html
import http.server
import json
import os
import socket
import socketserver
import time
from pathlib import Path

PORT = int(os.environ.get("SANDBOX_PORT", "8805"))
RUNTIME = os.environ.get("SANDBOX_RUNTIME", "sin sandbox")

# Un custodio de claves no debe tener red — ni siquiera loopback. Por eso
# escucha en un **socket Unix** dentro de un directorio montado por el
# supervisor: las peticiones entran por el sistema de archivos, no por la pila
# de red, y el sandbox conserva `network: none`. Es como hablan de verdad
# gpg-agent, ssh-agent y los proxies de firma de los custodios.
SOCKET_PATH = os.environ.get("SANDBOX_SOCKET")

SECRET_ENV = "SANDBOX_WALLET_KEY"
MAX_AMOUNT = 1_000_000
signed_log: list[dict] = []


def key_material() -> bytes | None:
    value = os.environ.get(SECRET_ENV)
    return value.encode() if value else None


def key_fingerprint() -> str:
    """Identifica la clave sin revelarla: SHA-256 de la clave, primeros 16 hex."""
    key = key_material()
    if not key:
        return "(sin clave)"
    return hashlib.sha256(key).hexdigest()[:16]


def canonical(transaction: dict) -> bytes:
    """Serialización determinista: dos JSON equivalentes deben firmar igual."""
    return json.dumps(transaction, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def validate_transaction(transaction: dict) -> str | None:
    for field in ("from", "to", "amount"):
        if field not in transaction:
            return f"falta el campo obligatorio: {field}"
    if not isinstance(transaction["amount"], (int, float)) or transaction["amount"] <= 0:
        return "amount debe ser un número positivo"
    if transaction["amount"] > MAX_AMOUNT:
        return f"amount supera el límite de la política ({MAX_AMOUNT})"
    if transaction["from"] == transaction["to"]:
        return "origen y destino no pueden coincidir"
    return None


def sign(transaction: dict) -> dict:
    key = key_material()
    payload = canonical(transaction)
    digest = hashlib.sha256(payload).hexdigest()

    if not key:
        # Sin clave el servicio no se cae ni finge una firma: enseña qué haría.
        return {
            "mode": "plan",
            "reason": f"la variable {SECRET_ENV} no está en el entorno del sandbox",
            "transactionHash": digest,
            "canonicalBytes": len(payload),
            "wouldSignWith": "HMAC-SHA256 sobre la transacción canonicalizada",
            "howToEnable": [
                f"1. export {SECRET_ENV}=$(openssl rand -hex 32)",
                "2. sandboxctl service up wallet-signer",
                "3. La clave solo existirá dentro del sandbox: no hay red por donde salga",
            ],
        }

    signature = hmac.new(key, payload, hashlib.sha256).hexdigest()
    record = {"at": time.time(), "hash": digest, "amount": transaction["amount"], "to": transaction["to"]}
    signed_log.append(record)
    if len(signed_log) > 200:
        del signed_log[:-200]

    return {
        "mode": "live",
        "transactionHash": digest,
        "signature": signature,
        "algorithm": "HMAC-SHA256",
        "keyFingerprint": key_fingerprint(),
        "signedCount": len(signed_log),
        # La clave nunca aparece: ni entera, ni truncada, ni en los logs.
        "note": "la clave privada no sale del sandbox; solo salen firmas",
    }


def egress_check() -> dict:
    """Comprueba que este sandbox NO puede salir a la red.

    Es el control que sostiene todo el diseño: si desde aquí se pudiera abrir
    una conexión, la clave sería exfiltrable y el aislamiento no valdría nada.
    """
    reachable = []
    for host, port in [("1.1.1.1", 53), ("8.8.8.8", 443)]:
        try:
            with socket.create_connection((host, port), timeout=2):
                reachable.append(f"{host}:{port}")
        except OSError:
            continue
    return {
        "salidaBloqueada": not reachable,
        "destinosAlcanzados": reachable,
        "veredicto": (
            "correcto: la clave no tiene por dónde salir"
            if not reachable
            else "PELIGRO: hay salida de red y la clave es exfiltrable"
        ),
    }


def status_snapshot() -> dict:
    try:
        pids = len([entry for entry in os.listdir("/proc") if entry.isdigit()])
    except OSError:
        pids = -1
    secrets_readable = []
    for path in ("/etc/shadow", os.path.expanduser("~/.ssh/id_rsa")):
        try:
            if Path(path).read_bytes():
                secrets_readable.append(path)
        except OSError:
            continue
    return {
        "runtime": RUNTIME,
        "mode": "live" if key_material() else "plan",
        "keyFingerprint": key_fingerprint(),
        "firmasRealizadas": len(signed_log),
        "pidsVisibles": pids,
        "secretosDelHostLegibles": secrets_readable,
        "egress": egress_check(),
    }


PAGE = """<!DOCTYPE html>
<html lang="es"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1"><meta name="color-scheme" content="light dark">
<title>Firma aislada · sandbox-labs</title>
<style>
:root{{color-scheme:light dark;--bg:#f6f8fc;--panel:#fff;--text:#16202b;--muted:#5b6879;--line:rgba(22,32,43,.14);--ok:#1f7a4f;--bad:#b23131;--warn:#b06a12;--accent:#3b62d9;--code:#1b2330;--codefg:#e8eef7}}
@media(prefers-color-scheme:dark){{:root{{--bg:#0a0f1c;--panel:#131b2b;--text:#eef2fa;--muted:#9fadc4;--line:rgba(158,176,205,.2);--ok:#4fd39b;--bad:#f08a8a;--warn:#e0b055;--accent:#7d9bff;--code:#0a1020;--codefg:#d5e0f5}}}}
*{{box-sizing:border-box}}body{{margin:0;padding:32px 20px;font-family:"Segoe UI",system-ui,sans-serif;background:var(--bg);color:var(--text);line-height:1.55}}
.shell{{max-width:900px;margin:0 auto}}
.card{{background:var(--panel);border:1px solid var(--line);border-radius:20px;padding:28px;box-shadow:0 16px 40px rgba(22,32,43,.1)}}
h1{{margin:0 0 6px;font-size:1.8rem}}.sub{{color:var(--muted);margin:0 0 18px}}
.pill{{display:inline-flex;gap:8px;padding:6px 12px;border-radius:999px;font-weight:700;font-size:.82rem;margin-bottom:14px}}
.pill.plan{{background:rgba(176,106,18,.14);color:var(--warn)}}.pill.live{{background:rgba(31,122,79,.14);color:var(--ok)}}
.facts{{display:flex;flex-wrap:wrap;gap:10px;margin-bottom:18px}}
.fact{{padding:7px 12px;border-radius:999px;border:1px solid var(--line);font-size:.82rem;color:var(--muted)}}
.fact.ok{{color:var(--ok);border-color:rgba(31,122,79,.35);background:rgba(31,122,79,.1);font-weight:600}}
.fact.bad{{color:var(--bad);border-color:rgba(178,49,49,.35);background:rgba(178,49,49,.1);font-weight:600}}
textarea{{width:100%;min-height:150px;padding:14px;border-radius:14px;border:1px solid var(--line);background:var(--code);color:var(--codefg);font-family:Consolas,Monaco,monospace;font-size:.86rem}}
button{{margin-top:12px;padding:12px 20px;border:none;border-radius:999px;background:var(--accent);color:#fff;font:inherit;font-weight:700;cursor:pointer}}
pre{{margin-top:16px;padding:16px;border-radius:14px;background:var(--code);color:var(--codefg);overflow:auto;max-height:380px;white-space:pre-wrap;word-break:break-word;font-family:Consolas,Monaco,monospace;font-size:.85rem}}
.foot{{margin-top:20px;padding-top:14px;border-top:1px solid var(--line);color:var(--muted);font-size:.86rem}}
code{{background:rgba(59,98,217,.12);padding:2px 6px;border-radius:6px}}
</style></head><body><div class="shell"><div class="card">
<span class="pill {mode}">{modelabel}</span>
<h1>La clave firma aquí dentro y no sale</h1>
<p class="sub">{explain}</p>
<div class="facts">{facts}</div>
<form id="f"><textarea id="t" spellcheck="false">{example}</textarea>
<button type="submit" id="b">✍️ {action}</button></form>
<pre id="out">La firma aparecerá aquí. La clave nunca.</pre>
<p class="foot">runtime <code>{runtime}</code> · huella de clave <code>{fp}</code> ·
<a href="/api/status">/api/status</a> · <a href="/api/egress">/api/egress</a></p>
</div></div>
<script>
const f=document.getElementById('f'),b=document.getElementById('b'),o=document.getElementById('out');
f.addEventListener('submit',async e=>{{
  e.preventDefault();b.disabled=true;o.textContent='Firmando…';
  try{{
    const r=await fetch('/api/sign',{{method:'POST',headers:{{'content-type':'application/json'}},body:document.getElementById('t').value}});
    o.textContent=JSON.stringify(await r.json(),null,2);
  }}catch(err){{o.textContent='fallo: '+err.message}}
  finally{{b.disabled=false}}
}});
</script></body></html>"""

EXAMPLE = json.dumps({"from": "wallet-a", "to": "wallet-b", "amount": 250, "nonce": 7}, indent=2, ensure_ascii=False)


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
            self._json(200, {"status": "ok", "mode": "live" if key_material() else "plan"})
            return
        if self.path == "/api/status":
            self._json(200, status_snapshot())
            return
        if self.path == "/api/egress":
            self._json(200, egress_check())
            return

        snapshot = status_snapshot()
        live = snapshot["mode"] == "live"
        blocked = snapshot["egress"]["salidaBloqueada"]
        facts = "".join(
            f'<span class="fact {cls}">{html.escape(text)}</span>'
            for cls, text in [
                ("ok" if blocked else "bad", "salida de red bloqueada" if blocked else "HAY SALIDA DE RED"),
                ("ok" if not snapshot["secretosDelHostLegibles"] else "bad",
                 "secretos del host no legibles" if not snapshot["secretosDelHostLegibles"] else "secretos del host LEGIBLES"),
                ("", f"{snapshot['pidsVisibles']} PIDs visibles"),
                ("", f"{snapshot['firmasRealizadas']} firmas emitidas"),
            ]
        )
        page = PAGE.format(
            mode="live" if live else "plan",
            modelabel="🟢 modo real · clave cargada" if live else "🟡 modo plan · sin clave",
            explain=(
                "La clave está dentro del sandbox y las transacciones se firman de verdad. Fíjate en que "
                "la respuesta trae la firma y la huella de la clave, nunca la clave."
                if live
                else f"No hay clave en el sandbox, así que se muestra qué se firmaría. Exporta "
                f"<code>{SECRET_ENV}</code> y vuelve a levantar el servicio para firmar de verdad."
            ),
            facts=facts,
            action="Firmar la transacción" if live else "Ver qué se firmaría",
            example=html.escape(EXAMPLE),
            runtime=html.escape(RUNTIME),
            fp=html.escape(snapshot["keyFingerprint"]),
        )
        self._send(200, page.encode(), "text/html; charset=utf-8")

    def do_POST(self) -> None:  # noqa: N802
        if self.path != "/api/sign":
            self._json(404, {"error": "no encontrado"})
            return
        try:
            length = int(self.headers.get("content-length", "0"))
            transaction = json.loads(self.rfile.read(min(length, 16_384)).decode("utf-8"))
            if not isinstance(transaction, dict):
                raise TypeError
        except (ValueError, TypeError):
            self._json(400, {"error": "se espera una transacción JSON"})
            return

        problem = validate_transaction(transaction)
        if problem:
            self._json(400, {"error": problem})
            return
        self._json(200, sign(transaction))

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
    modo = "real" if key_material() else "plan"
    if SOCKET_PATH:
        # Un socket huérfano de una ejecución anterior impediría enlazar.
        try:
            os.unlink(SOCKET_PATH)
        except FileNotFoundError:
            pass
        os.makedirs(os.path.dirname(SOCKET_PATH), exist_ok=True)
        with UnixServer(SOCKET_PATH, Handler) as server:
            os.chmod(SOCKET_PATH, 0o660)
            print(
                f"servicio 05-wallet-signer en unix:{SOCKET_PATH} · runtime={RUNTIME} · "
                f"modo={modo} · huella={key_fingerprint()} · sin red",
                flush=True,
            )
            server.serve_forever()
        return

    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer(("127.0.0.1", PORT), Handler) as server:
        print(
            f"servicio 05-wallet-signer en 127.0.0.1:{PORT} · runtime={RUNTIME} · "
            f"modo={modo} · huella={key_fingerprint()}",
            flush=True,
        )
        server.serve_forever()


if __name__ == "__main__":
    main()
