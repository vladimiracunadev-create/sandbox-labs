#!/usr/bin/env python3
"""Prueba negativa local: comprueba que una ruta no salga de una raíz permitida."""
from pathlib import Path

root = Path(__file__).resolve().parents[2]
for candidate in ["workloads/benign/hello", "../../etc/passwd", "../README.md"]:
    resolved = (root / candidate).resolve()
    allowed = resolved == root or root in resolved.parents
    print(f"{candidate}: {'allowed' if allowed else 'blocked'} -> {resolved}")
