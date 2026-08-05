# Lab 06 — linux-capabilities

**Estado:** `experimental` · **Nivel:** `intermediate`

## Objetivo

Eliminar privilegios granulares y comprobar el set efectivo.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

Capabilities.

```bash
capsh --print 2>/dev/null || true
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- Bubblewrap usa `--cap-drop ALL`.
- La evidencia no debe declarar capabilities en unshare sin prueba.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
