# Lab 16 — escape-test-suite

**Estado:** `ready` · **Nivel:** `advanced`

## Objetivo

Hacer que el test apruebe cuando la acción peligrosa falla.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

Escape Test Suite.

```bash
node scripts/run-negative-tests.mjs
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- Filesystem y red bloqueados.
- Native rechaza workloads no autorizados.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
