# Lab 17 — sandbox-benchmarks

**Estado:** `planned` · **Nivel:** `advanced`

## Objetivo

Comparar startup, memoria, CPU y compatibilidad con la misma carga.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

Benchmarks.

```bash
cat benchmarks/matrix.json
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- Mínimo 10 repeticiones.
- Guardar host/runtime y percentiles, no valores inventados.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
