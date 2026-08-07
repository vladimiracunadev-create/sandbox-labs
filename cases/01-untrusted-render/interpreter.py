#!/usr/bin/env python3
"""Intérprete de contenido ajeno. Lee de stdin, escribe JSON en stdout.

Este proceso **no abre ficheros, no resuelve nombres y no hace peticiones**. No
porque se le prohíba comprobación a comprobación, sino porque nunca llama a las
funciones que harían falta: no hay `open`, no hay `socket`, no hay `urllib`. Lo
único que cruza la frontera es texto por la entrada estándar y JSON por la
salida.

Cuando el contenido pide algo de fuera —una entidad externa, una imagen remota,
un `file://`— el intérprete **no lo resuelve**: lo anota como solicitud
rechazada, con el motivo. Esa lista es el producto del caso; la vista limpia es
el subproducto.

La política de capacidades es la de arriba y cabe en una línea:

    capacidades = {}

Cada regla de abajo existe porque hay un incidente real detrás. Están comentadas
una a una en `REJECTIONS`.
"""

from __future__ import annotations

import json
import re
import sys
from html.parser import HTMLParser

# Ninguna de estas capacidades está concedida. La lista está aquí, explícita,
# para que se vea que la decisión es «no dar», no «dar y filtrar».
CAPABILITIES: dict[str, bool] = {
    "filesystem": False,
    "network": False,
    "subprocess": False,
    "clock": False,
    "environment": False,
}

# Techos de trabajo. Un contenido hostil no necesita salir de la jaula para
# hacer daño: le basta con que el parser no tenga fin.
MAX_NODES = 20_000
MAX_TEXT = 200_000
MAX_DEPTH = 100

# Etiquetas que nunca sobreviven a la interpretación. No se «escapan»: se
# descartan con su contenido, porque un `<script>` escapado sigue siendo un
# `<script>` para el siguiente que lo procese.
DROPPED_TAGS = {"script", "style", "iframe", "object", "embed", "applet", "frame", "frameset", "link", "meta", "base"}

# Lo que sí puede quedar. Lista de permitidos, no de prohibidos: lo que no está
# aquí desaparece, incluida la etiqueta que se invente mañana.
ALLOWED_TAGS = {
    "p", "br", "hr", "b", "strong", "i", "em", "u", "code", "pre", "blockquote",
    "ul", "ol", "li", "h1", "h2", "h3", "h4", "h5", "h6", "span", "div",
    "table", "thead", "tbody", "tr", "th", "td", "a", "img",
}

# Atributos permitidos por etiqueta. Todo lo demás se cae, y con ello se caen
# los `onerror=`, `onload=` y compañía sin tener que enumerarlos.
ALLOWED_ATTRS = {"a": {"href", "title"}, "img": {"src", "alt", "title"}, "*": {"title"}}

# Esquemas de URL que se dejan pasar en un atributo. `javascript:` y `data:` no
# están, y `file:` tampoco: ninguno de los tres tiene sentido en contenido que
# llega de fuera.
SAFE_SCHEMES = ("http://", "https://", "mailto:", "/", "#")

# Rangos que aparecen cuando alguien intenta que el servidor pida algo por él.
# 169.254.169.254 es el servicio de metadatos de las nubes grandes: quien lo
# alcanza desde dentro se lleva credenciales.
SSRF_HOSTS = ("169.254.169.254", "metadata.google.internal", "localhost", "127.0.0.1", "0.0.0.0", "[::1]", "100.100.100.200")

# Una entidad externa en el DOCTYPE es XXE. Aquí no hay resolución de entidades
# en absoluto, así que basta con detectarla y contarla.
DOCTYPE_ENTITY = re.compile(r"<!ENTITY\s+(\S+)\s+(?:SYSTEM|PUBLIC)\s+[^>]*>", re.IGNORECASE)


class Rejection(dict):
    """Una solicitud del contenido que no se atendió, y por qué."""

    def __init__(self, kind: str, detail: str, why: str) -> None:
        super().__init__(kind=kind, detail=detail[:300], why=why)


class Interpreter(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.out: list[str] = []
        self.rejections: list[Rejection] = []
        self.nodes = 0
        self.depth = 0
        self.max_depth = 0
        self.dropping: str | None = None
        self.truncated = False

    # --- utilidades -------------------------------------------------------

    def _budget(self) -> bool:
        self.nodes += 1
        if self.nodes > MAX_NODES:
            if not self.truncated:
                self.rejections.append(
                    Rejection("presupuesto", f"más de {MAX_NODES} nodos", "un documento sin fin es una denegación de servicio")
                )
                self.truncated = True
            return False
        return True

    def _url(self, tag: str, attr: str, value: str) -> str | None:
        raw = (value or "").strip()
        low = raw.lower().replace("\t", "").replace("\n", "")
        if low.startswith("javascript:") or low.startswith("vbscript:"):
            self.rejections.append(Rejection("script-en-url", f"<{tag} {attr}={raw}>", "una URL no es un sitio donde ejecutar código"))
            return None
        if low.startswith("data:"):
            self.rejections.append(Rejection("data-uri", f"<{tag} {attr}={raw[:60]}>", "un data: URI trae su propio contenido y se salta el origen"))
            return None
        if low.startswith("file://") or low.startswith("/etc/") or ".." in low:
            self.rejections.append(Rejection("acceso-a-fichero", f"<{tag} {attr}={raw}>", "el intérprete no tiene filesystem: no hay nada que leer"))
            return None
        if any(host in low for host in SSRF_HOSTS):
            self.rejections.append(Rejection("ssrf", f"<{tag} {attr}={raw}>", "pedir esa dirección desde el servidor es pedir sus credenciales"))
            return None
        if not low.startswith(SAFE_SCHEMES):
            self.rejections.append(Rejection("esquema-desconocido", f"<{tag} {attr}={raw}>", "solo http, https, mailto y rutas relativas"))
            return None
        if low.startswith(("http://", "https://")):
            # No se descarga: se deja el enlace, pero se anota que el contenido
            # quería que saliésemos a la red.
            self.rejections.append(Rejection("red-no-concedida", raw, "el intérprete no tiene red: la referencia queda sin resolver"))
        return raw

    # --- ganchos del parser ----------------------------------------------

    def handle_starttag(self, tag: str, attrs) -> None:
        if not self._budget():
            return
        self.depth += 1
        self.max_depth = max(self.max_depth, self.depth)
        if self.depth > MAX_DEPTH:
            self.rejections.append(Rejection("anidamiento", f"más de {MAX_DEPTH} niveles", "un árbol muy profundo revienta parsers recursivos"))
            return
        if self.dropping:
            return
        if tag in DROPPED_TAGS:
            self.dropping = tag
            self.rejections.append(Rejection("etiqueta-descartada", f"<{tag}>", "esta etiqueta ejecuta o carga algo, y aquí no se ejecuta ni se carga nada"))
            return
        if tag not in ALLOWED_TAGS:
            self.rejections.append(Rejection("etiqueta-no-permitida", f"<{tag}>", "solo sobrevive lo que está en la lista de permitidos"))
            return

        kept = []
        allowed = ALLOWED_ATTRS.get(tag, set()) | ALLOWED_ATTRS["*"]
        for name, value in attrs:
            if name.lower().startswith("on"):
                self.rejections.append(Rejection("manejador-de-evento", f"<{tag} {name}>", "un atributo on* es código dentro de un atributo"))
                continue
            if name.lower() not in allowed:
                self.rejections.append(Rejection("atributo-no-permitido", f"<{tag} {name}>", "cada etiqueta declara los atributos que admite"))
                continue
            if name.lower() in ("href", "src"):
                value = self._url(tag, name, value or "")
                if value is None:
                    continue
            kept.append(f' {name}="{escape(value or "")}"')
        self.out.append(f"<{tag}{''.join(kept)}>")

    def handle_endtag(self, tag: str) -> None:
        self.depth = max(0, self.depth - 1)
        if self.dropping == tag:
            self.dropping = None
            return
        if self.dropping or tag not in ALLOWED_TAGS:
            return
        self.out.append(f"</{tag}>")

    def handle_data(self, data: str) -> None:
        if self.dropping or not self._budget():
            return
        self.out.append(escape(data))

    def handle_comment(self, data: str) -> None:
        # Los comentarios condicionales de IE y los `<!--[if]-->` han sido
        # vectores; además pueden esconder marcado. No se conservan.
        if "<" in data or "[if" in data.lower():
            self.rejections.append(Rejection("comentario-con-marcado", data[:80], "un comentario no debería contener etiquetas"))

    def handle_decl(self, decl: str) -> None:
        for entity in DOCTYPE_ENTITY.finditer(f"<!{decl}>"):
            self.rejections.append(
                Rejection("entidad-externa", entity.group(0), "XXE: una entidad externa haría que el parser leyese un fichero por ti")
            )

    def error(self, message: str) -> None:  # pragma: no cover - API antigua
        self.rejections.append(Rejection("parser", message, "el contenido no está bien formado"))


def escape(text: str) -> str:
    return (
        text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;").replace("'", "&#39;")
    )


def markdown_hazards(content: str, rejections: list[Rejection]) -> None:
    """Markdown no es inocente: sus enlaces admiten los mismos esquemas."""
    for match in re.finditer(r"\[[^\]]*\]\(([^)]+)\)", content):
        target = match.group(1).strip().lower()
        if target.startswith(("javascript:", "data:", "file://")):
            rejections.append(Rejection("enlace-markdown", match.group(0)[:80], "un enlace Markdown puede llevar el mismo esquema hostil"))


def interpret(content: str) -> dict:
    if len(content) > MAX_TEXT:
        content = content[:MAX_TEXT]
        truncated = True
    else:
        truncated = False

    parser = Interpreter()
    # El DOCTYPE se examina y se retira antes de parsear. `HTMLParser` no
    # entiende el subconjunto interno —el `[ ... ]`— y deja caer su cola dentro
    # del texto; peor aún, ahí es donde vive el XXE. Se saca entero.
    entities = [
        Rejection("entidad-externa", found.group(0), "XXE: una entidad externa haría que el parser leyese un fichero por ti")
        for found in DOCTYPE_ENTITY.finditer(content)
    ]
    body = re.sub(r"<!DOCTYPE[^>\[]*(\[[^\]]*\])?\s*>", "", content, flags=re.IGNORECASE | re.DOTALL)
    parser.rejections.extend(entities)
    parser.feed(body)
    parser.close()

    markdown_hazards(content, parser.rejections)

    kinds: dict[str, int] = {}
    for rejection in parser.rejections:
        kinds[rejection["kind"]] = kinds.get(rejection["kind"], 0) + 1

    return {
        "capabilities": CAPABILITIES,
        "safeHtml": "".join(parser.out).strip()[:MAX_TEXT],
        "rejections": list(parser.rejections),
        "rejectionsByKind": kinds,
        "stats": {
            "inputBytes": len(content),
            "nodes": parser.nodes,
            "maxDepth": parser.max_depth,
            "inputTruncated": truncated,
            "outputTruncated": parser.truncated,
        },
    }


def main() -> None:
    print(json.dumps(interpret(sys.stdin.read()), ensure_ascii=False))


if __name__ == "__main__":
    main()
