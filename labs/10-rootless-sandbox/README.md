# Lab 10 — rootless-sandbox

**Estado:** `experimental` · **Nivel:** `intermediate`

## Objetivo

Combinar user namespace, mounts, capabilities y red sin root.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

Rootless sandbox.

```bash
cargo run -p sandboxctl -- plan --workload workloads/benign/hello --runtime bwrap --policy policies/minimal.json
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- Ejecutar en VM Linux.
- Validar `--new-session` y `--die-with-parent`.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
