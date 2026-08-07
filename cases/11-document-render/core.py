#!/usr/bin/env python3
"""Procesamiento de documentos: el caso donde lo desconocido son los datos.

En los demás casos lo desconocido es el código. Aquí el código es **tuyo** —una
biblioteca conocida y respetable— y lo desconocido es el fichero. La conclusión
incómoda es que hay que aislar tu propio software, porque el fallo va a estar
ahí: los parsers de formatos complejos están escritos en C y llevan treinta años
acumulando esquinas.

El documento no tiene que parecer malicioso. Basta con que el parser tenga un
fallo, y los tiene.
"""

from __future__ import annotations

# Firmas de tipo por contenido. Se mira el fichero, **nunca la extensión**: un
# fichero que dice ser una imagen y es un PDF ya merece atención antes de
# abrirlo.
SIGNATURES = [
    (b"%PDF-", "application/pdf"),
    (b"\x89PNG\r\n\x1a\n", "image/png"),
    (b"\xff\xd8\xff", "image/jpeg"),
    (b"PK\x03\x04", "application/zip"),
    (b"GIF8", "image/gif"),
    (b"\x00\x01\x00\x00", "font/ttf"),
    (b"{\\rtf", "application/rtf"),
]

# Marcas de referencia externa dentro de un documento. Ninguna se resuelve: el
# parser no tiene disco ni red. Se anotan porque son el dato útil.
EXTERNAL_MARKERS = [b"/URI", b"/Launch", b"/EmbeddedFile", b"file://", b"http://", b"https://", b"/JavaScript", b"/OpenAction"]


def detect_type(data: bytes) -> str:
    """Tipo real por contenido."""
    for signature, mime in SIGNATURES:
        if data.startswith(signature):
            return mime
    return "application/octet-stream"


def inspect(data: bytes, declared_type: str, limits: dict) -> dict:
    """Examina el documento **antes** de entregárselo al parser.

    Todo lo que se pueda decidir sin ejecutar el parser se decide aquí: cuanto
    menos llegue a la parte frágil, menos superficie hay.
    """
    detected = detect_type(data)
    findings = []

    if declared_type and detected != declared_type:
        findings.append(
            {"kind": "tipo-discrepante", "detail": f"dice ser {declared_type} y es {detected}", "why": "el tipo se mira por contenido, no por extensión"}
        )

    max_bytes = limits.get("maxBytes", 26_214_400)
    if len(data) > max_bytes:
        findings.append({"kind": "demasiado-grande", "detail": f"{len(data)} bytes", "why": "un techo de tamaño es la defensa más barata"})

    external = []
    for marker in EXTERNAL_MARKERS:
        if marker in data:
            external.append(
                {
                    "target": marker.decode("ascii", "replace"),
                    "outcome": "no resuelta: el parser no tiene disco ni red",
                }
            )

    # Una relación de compresión desproporcionada dentro de un contenedor ZIP es
    # la misma bomba del caso 03 con otro traje.
    if detected == "application/zip" and len(data) < 4096:
        findings.append(
            {"kind": "contenedor-sospechoso", "detail": "contenedor comprimido muy pequeño", "why": "puede expandirse sin fin: ver el caso 03"}
        )

    return {
        "detectedType": detected,
        "declaredType": declared_type,
        "bytes": len(data),
        "findings": findings,
        "externalReferences": external,
        "safeToParse": not any(finding["kind"] == "demasiado-grande" for finding in findings),
    }


def requires_memory_limit(limits: dict) -> dict:
    """Comprueba que hay techo de memoria antes de parsear.

    Aquí `memory.max` no es un lujo: una imagen con dimensiones absurdas reserva
    memoria hasta llevarse el equipo por delante. Con política estricta y sin
    cgroups, este caso **debe negarse a ejecutar**, y eso es lo correcto.
    """
    limit = limits.get("memoryLimitMb")
    if not limit:
        return {
            "canRun": False,
            "reason": "sin techo de memoria no se parsea: una imagen de dimensiones absurdas se lleva el equipo",
        }
    return {"canRun": True, "memoryLimitMb": limit}


def handle(payload: dict) -> dict:
    """Punto de entrada: el documento en base64 y sus techos."""
    import base64

    raw = payload.get("documentBase64", "")
    try:
        data = base64.b64decode(raw, validate=True)
    except Exception as error:  # noqa: BLE001
        raise ValueError(f"documentBase64 no es base64 válido: {error}") from error

    limits = payload.get("limits", {})
    return {"preflight": requires_memory_limit(limits), **inspect(data, payload.get("declaredType", ""), limits)}
