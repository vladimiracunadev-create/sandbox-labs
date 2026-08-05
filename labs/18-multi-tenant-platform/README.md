# Lab 18 — multi-tenant-platform

**Estado:** `planned` · **Nivel:** `product`

## Objetivo

Diseñar cuotas, identidades y aislamiento entre trabajos.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

Multi-tenant.

```bash
cat docs/IMPLEMENTATION_BACKLOG.md
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- No exponer fuera de localhost sin autenticación.
- Persistencia, cuotas y auditoría antes de uso real.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
