# Lab 04 — linux-namespaces

**Estado:** `experimental` · **Nivel:** `intermediate`

## Objetivo

Inspeccionar PID, user, mount, UTS, IPC y red.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

Namespaces Linux.

```bash
cargo run -p sandboxctl -- plan --workload workloads/benign/hello --runtime unshare --policy policies/web-application.json
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- Documentar cada namespace.
- No confundir namespace con límite de procesos.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
