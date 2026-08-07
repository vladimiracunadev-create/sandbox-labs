#!/usr/bin/env python3
"""Migraciones: el código que se ejecuta una vez sobre datos que existen una vez.

Casi todo el software se puede probar ejecutándolo otra vez. Una migración no.
Y llega escrita por alguien que no conoce el estado real de la base, a menudo
generada por una herramienta.

Lo que este caso aporta no es «funcionó» o «falló»: es **la comparación de
esquema y de datos antes y después**, y el coste medido. Eso permite decidir con
conocimiento en vez de con fe, y **antes** de la ventana de mantenimiento.
"""

from __future__ import annotations

import re

# Formas peligrosas. La lista es corta a propósito: cada entrada está aquí
# porque destruye datos o bloquea la base, no porque «suene mal».
DANGEROUS = [
    (re.compile(r"^\s*DROP\s+(TABLE|COLUMN|DATABASE)", re.IGNORECASE), "destruye datos que no se pueden recuperar"),
    (re.compile(r"^\s*TRUNCATE", re.IGNORECASE), "vacía la tabla entera"),
    (re.compile(r"^\s*DELETE\s+FROM\s+\w+\s*;?\s*$", re.IGNORECASE), "DELETE sin WHERE: borra todas las filas"),
    (re.compile(r"^\s*UPDATE\s+\w+\s+SET\b(?!.*\bWHERE\b)", re.IGNORECASE | re.DOTALL), "UPDATE sin WHERE: toca todas las filas"),
    (re.compile(r"^\s*ALTER\s+TABLE\s+\w+\s+ALTER\s+COLUMN.*TYPE", re.IGNORECASE), "cambio de tipo: puede truncar en silencio"),
]


def classify(statement: str) -> dict:
    """Clasifica una sentencia antes de ejecutarla."""
    for pattern, why in DANGEROUS:
        if pattern.search(statement):
            return {"sql": statement.strip()[:200], "risk": "alto", "why": why}
    if re.search(r"^\s*(ALTER|CREATE|DROP\s+INDEX)", statement, re.IGNORECASE):
        return {"sql": statement.strip()[:200], "risk": "medio", "why": "cambia el esquema y puede bloquear la tabla"}
    return {"sql": statement.strip()[:200], "risk": "bajo", "why": ""}


def snapshot(schema: dict, rows: dict) -> dict:
    """Copia del estado. El rollback es restaurar esto, no deshacer sentencias."""
    return {"schema": {table: sorted(columns) for table, columns in schema.items()}, "rows": dict(rows)}


def compare(before: dict, after: dict) -> dict:
    """Compara esquema y datos. Son dos preguntas distintas y se responden aparte.

    Una migración puede no tocar el esquema y cambiar millones de filas; el
    `schemaDiff` vacío no significa que no pasó nada.
    """
    added, removed = [], []
    for table, columns in after["schema"].items():
        previous = set(before["schema"].get(table, []))
        added.extend(f"{table}.{column}" for column in columns if column not in previous)
    for table, columns in before["schema"].items():
        current = set(after["schema"].get(table, []))
        removed.extend(f"{table}.{column}" for column in columns if column not in current)

    rows_changed = sum(abs(after["rows"].get(table, 0) - count) for table, count in before["rows"].items())
    return {"schemaDiff": {"added": sorted(added), "removed": sorted(removed)}, "dataDiff": {"rowsChanged": rows_changed}}


def migrate(statements: list[str], schema: dict, rows: dict, budget: dict, fail_on: list[str]) -> dict:
    """Ejecuta la migración sobre una copia y decide si se aplica o se revierte.

    Se trabaja siempre sobre la copia. El original ni se toca: el rollback no es
    una operación de recuperación, es no haber tocado nada.
    """
    before = snapshot(schema, rows)
    working_schema = {table: list(columns) for table, columns in schema.items()}
    working_rows = dict(rows)

    classified = []
    elapsed_ms = 0
    touched = 0
    blocked = None

    for statement in statements:
        entry = classify(statement)
        # Coste simulado: proporcional a las filas de la tabla que toca. Sirve
        # para ver el orden de magnitud antes de la ventana real.
        table = re.search(r"\b(?:TABLE|FROM|UPDATE|INTO)\s+(\w+)", statement, re.IGNORECASE)
        table_rows = working_rows.get(table.group(1), 0) if table else 0
        entry["rows"] = table_rows
        entry["ms"] = max(1, table_rows // 1000)
        elapsed_ms += entry["ms"]
        touched += table_rows if entry["risk"] == "alto" else 0
        classified.append(entry)

        if entry["risk"] == "alto" and "destructive-without-confirmation" in fail_on:
            blocked = f"sentencia destructiva sin confirmación: {entry['why']}"
            break
        if budget.get("seconds") and elapsed_ms > budget["seconds"] * 1000:
            blocked = "presupuesto de tiempo agotado"
            break
        if budget.get("rowsTouched") and touched > budget["rowsTouched"]:
            blocked = "presupuesto de filas agotado"
            break

        # Se aplica al esquema de trabajo lo poco que este modelo entiende.
        added = re.search(r"ALTER\s+TABLE\s+(\w+)\s+ADD\s+COLUMN\s+(\w+)", statement, re.IGNORECASE)
        if added:
            working_schema.setdefault(added.group(1), []).append(added.group(2))
        dropped = re.search(r"ALTER\s+TABLE\s+(\w+)\s+DROP\s+COLUMN\s+(\w+)", statement, re.IGNORECASE)
        if dropped and dropped.group(2) in working_schema.get(dropped.group(1), []):
            working_schema[dropped.group(1)].remove(dropped.group(2))

    after = snapshot(working_schema, working_rows)
    diff = compare(before, after)

    if blocked:
        return {
            "outcome": "rolled-back",
            "reason": blocked,
            "statements": classified,
            **diff,
            "restoredFromSnapshot": True,
            "elapsedMs": elapsed_ms,
        }

    return {"outcome": "applied", "statements": classified, **diff, "restoredFromSnapshot": False, "elapsedMs": elapsed_ms}


def handle(payload: dict) -> dict:
    """Punto de entrada: sentencias, estado simulado y presupuesto."""
    return migrate(
        payload.get("statements", []),
        payload.get("schema", {}),
        payload.get("rows", {}),
        payload.get("budget", {}),
        payload.get("failOn", ["destructive-without-confirmation"]),
    )
