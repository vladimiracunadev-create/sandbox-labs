#!/usr/bin/env python3
"""Ejecución determinista con presupuesto por instrucciones.

Los demás casos restringen **lo que el código puede tocar**. Este restringe **lo
que puede saber**: sin reloj, sin aleatoriedad, sin red, sin entorno. Dos
máquinas que ejecuten lo mismo tienen que llegar al mismo estado final, o no hay
acuerdo posible.

Y hay un detalle que rompe el determinismo y casi nadie ve: **acotar por tiempo
lo destruye**. Si la ejecución se corta a los 5 segundos, el resultado depende de
qué máquina la ejecutó. Aquí el presupuesto se cuenta en **instrucciones**, así
que la máquina lenta y la rápida se detienen exactamente en el mismo punto.

La máquina de abajo es diminuta a propósito: lo que enseña el caso no es el
lenguaje, es el presupuesto y el rollback.
"""

from __future__ import annotations

import hashlib
import json

# Coste en «gas» de cada instrucción. Publicado: sin tabla de costes, dos
# implementaciones cobran distinto y dejan de coincidir.
COST = {"PUSH": 1, "ADD": 3, "SUB": 3, "MUL": 5, "LOAD": 10, "STORE": 20, "JMPZ": 4, "LOG": 8, "HALT": 0}


class Halt(Exception):
    """Fin normal del programa."""


def canonical(state: dict) -> str:
    """Serialización canónica: un mismo estado, una misma cadena de bytes.

    Claves ordenadas y sin espacios. Sin esto, comparar hashes no significa
    nada: dos representaciones del mismo estado darían huellas distintas.
    """
    return json.dumps(state, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def state_hash(state: dict) -> str:
    return "sha256:" + hashlib.sha256(canonical(state).encode()).hexdigest()


def run(program: list[list], initial_state: dict, gas_limit: int) -> dict:
    """Ejecuta y devuelve el resultado.

    Si se agota el presupuesto o el contrato falla, **el estado no cambia**: se
    devuelve el inicial. El rollback no es una operación aparte, es no haber
    aplicado nada todavía.
    """
    # Se trabaja sobre una copia. El estado real solo se sustituye al final y
    # solo si todo fue bien.
    state = dict(initial_state)
    stack: list[int] = []
    logs: list[str] = []
    gas_used = 0
    pointer = 0
    steps = 0

    # Techo de pasos independiente del gas: un JMPZ que salta a sí mismo con
    # coste cero sería un bucle infinito con presupuesto de sobra.
    max_steps = gas_limit + 1_000

    try:
        while pointer < len(program):
            steps += 1
            if steps > max_steps:
                raise RuntimeError("el programa no avanza")

            instruction = program[pointer]
            opcode = instruction[0]
            if opcode not in COST:
                raise RuntimeError(f"instrucción desconocida: {opcode}")

            gas_used += COST[opcode]
            if gas_used > gas_limit:
                return {
                    "outcome": "out-of-gas",
                    "finalState": initial_state,
                    "stateHash": state_hash(initial_state),
                    "gasUsed": gas_limit,
                    "logs": logs,
                    "deterministic": True,
                }

            pointer += 1
            if opcode == "PUSH":
                stack.append(int(instruction[1]))
            elif opcode in ("ADD", "SUB", "MUL"):
                right, left = stack.pop(), stack.pop()
                stack.append({"ADD": left + right, "SUB": left - right, "MUL": left * right}[opcode])
            elif opcode == "LOAD":
                stack.append(int(state.get(instruction[1], 0)))
            elif opcode == "STORE":
                state[instruction[1]] = stack.pop()
            elif opcode == "JMPZ":
                if stack.pop() == 0:
                    pointer = int(instruction[1])
            elif opcode == "LOG":
                logs.append(str(instruction[1]))
            elif opcode == "HALT":
                raise Halt
    except Halt:
        pass
    except Exception as error:  # noqa: BLE001 — cualquier fallo del contrato es rollback
        return {
            "outcome": "failed",
            "reason": str(error),
            "finalState": initial_state,
            "stateHash": state_hash(initial_state),
            "gasUsed": gas_used,
            "logs": logs,
            "deterministic": True,
        }

    return {
        "outcome": "applied",
        "finalState": state,
        "stateHash": state_hash(state),
        "gasUsed": gas_used,
        "logs": logs,
        "deterministic": True,
    }


def capabilities() -> dict:
    """Lo que el contrato **no** tiene. Todas en `False`, siempre."""
    return {"clock": False, "randomness": False, "network": False, "filesystem": False, "environment": False}


def handle(payload: dict) -> dict:
    """Punto de entrada: programa, estado inicial y presupuesto."""
    return {
        "capabilities": capabilities(),
        **run(payload.get("program", []), payload.get("initialState", {}), int(payload.get("gasLimit", 1_000))),
    }
