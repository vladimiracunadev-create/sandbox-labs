# Lab 11 — gvisor-runsc

**Estado:** `documented` · **Nivel:** `advanced`

## Objetivo

Preparar bundles OCI efímeros y comparar compatibilidad.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

gVisor runsc.

```bash
cargo run -p sandboxctl -- runtimes
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- Imagen fijada por digest.
- Create/start/delete y cleanup reproducibles.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
