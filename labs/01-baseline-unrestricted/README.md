# Lab 01 — baseline-unrestricted

**Estado:** `ready` · **Nivel:** `initial`

## Objetivo

Comprender qué puede hacer un proceso sin aislamiento.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

Línea base nativa.

```bash
SANDBOX_LABS_ALLOW_NATIVE=1 cargo run -p sandboxctl -- run --workload workloads/benign/hello --runtime native --policy policies/web-application.json
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- Solo workload `hello`.
- Registrar que native no es sandbox.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
