# Lab 15 — ai-code-runner

**Estado:** `experimental` · **Nivel:** `product`

## Objetivo

Convertir solicitudes en jobs registrados, nunca comandos libres.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

AI Code Runner.

```bash
cd control-center && node scripts/build.mjs && node dist/server.js
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- POST solo acepta workloadId, policyId, runtimeId y argumentos.
- Cancelar, conservar evidencia y limpiar.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
