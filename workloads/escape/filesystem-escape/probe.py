#!/usr/bin/env python3
"""Sonda de contención: filesystem.

Comprueba tres cosas distintas que suelen confundirse:

1. **Lectura de secretos del host** — ¿se ve `/etc/shadow`, las claves SSH del
   usuario, las credenciales de la nube?
2. **Escritura fuera del workspace** — ¿se puede escribir en `/` o en `/etc`?
3. **Traversal** — ¿`../../..` sale de la raíz montada?

Un runtime puede contener la escritura y filtrar la lectura, así que cada
dimensión se reporta por separado.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

# Rutas cuya simple lectura significa que el host está expuesto.
SECRETS = [
    "/etc/shadow",
    "/root/.ssh/id_rsa",
    "/root/.aws/credentials",
    os.path.expanduser("~/.ssh/id_rsa"),
    os.path.expanduser("~/.aws/credentials"),
]

# Directorios del sistema donde ningún proceso contenido debería poder escribir.
WRITABLE_TARGETS = ["/", "/etc", "/usr/bin", "/var"]


def report(probe: str, dimension: str, result: str, detail: str) -> None:
    print(f"probe={probe} dimension={dimension} result={result} detail={detail}", flush=True)


def _readable(path: str) -> bool:
    """True si la ruta existe y devuelve contenido."""
    try:
        return bool(Path(path).read_bytes())
    except OSError:
        return False


def init_environ_is_foreign() -> bool:
    """True si `/proc/1/environ` pertenece a un proceso ajeno.

    Dentro de un PID namespace propio, el PID 1 es el init del sandbox y leer
    su entorno no filtra nada del host. Solo cuenta como fuga si el contenido
    difiere del entorno propio, que es la señal de que se está viendo el init
    real de la máquina.
    """
    try:
        foreign = Path("/proc/1/environ").read_bytes()
    except OSError:
        return False
    if not foreign:
        return False
    own = "\0".join(f"{key}={value}" for key, value in sorted(os.environ.items())).encode()
    foreign_keys = {entry.split(b"=", 1)[0] for entry in foreign.split(b"\0") if b"=" in entry}
    own_keys = {entry.split(b"=", 1)[0] for entry in own.split(b"\0") if b"=" in entry}
    # Si el init expone variables que este proceso no tiene, es otro proceso.
    return bool(foreign_keys - own_keys)


def check_secret_reads() -> bool:
    """True si se pudo leer contenido de alguna ruta sensible."""
    leaked = [path for path in SECRETS if _readable(path)]
    if init_environ_is_foreign():
        leaked.append("/proc/1/environ")
    if leaked:
        report("filesystem-read", "filesystem", "escaped", f"rutas sensibles legibles: {','.join(leaked)}")
        return True
    report("filesystem-read", "filesystem", "contained", "ninguna ruta sensible del host es legible")
    return False


def check_writes() -> bool:
    """True si se pudo crear un archivo fuera del workspace."""
    written = []
    for directory in WRITABLE_TARGETS:
        candidate = Path(directory) / ".sandbox-labs-escape-probe"
        try:
            candidate.write_text("probe", encoding="utf-8")
        except OSError:
            continue
        written.append(str(candidate))
        try:
            candidate.unlink()
        except OSError:
            pass
    if written:
        report("filesystem-write", "filesystem", "escaped", f"escritura permitida en: {','.join(written)}")
        return True
    report("filesystem-write", "filesystem", "contained", "sin escritura fuera del workspace")
    return False


def check_traversal() -> bool:
    """True si subir con `..` deja ver un árbol que no es el del sandbox."""
    # Si el runtime montó una raíz propia, `/` contiene poco y no incluye el
    # árbol del host. Se mide por presencia de directorios de usuario reales.
    host_markers = [Path("/home"), Path("/mnt"), Path("/media")]
    visible = []
    for marker in host_markers:
        try:
            entries = list(marker.iterdir())
        except OSError:
            continue
        if entries:
            visible.append(f"{marker}({len(entries)} entradas)")
    if visible:
        report("filesystem-traversal", "filesystem", "escaped", f"árbol del host visible: {','.join(visible)}")
        return True
    report("filesystem-traversal", "filesystem", "contained", "el árbol del host no es visible")
    return False


def main() -> int:
    escaped = False
    for check in (check_secret_reads, check_writes, check_traversal):
        try:
            escaped |= check()
        except Exception as error:  # noqa: BLE001
            report(check.__name__, "filesystem", "error", f"{type(error).__name__}: {error}")
            escaped = True
    return 1 if escaped else 0


if __name__ == "__main__":
    sys.exit(main())
