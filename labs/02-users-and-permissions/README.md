# Lab 02 — users-and-permissions

**Estado:** `documented` · **Nivel:** `initial`

## Objetivo

Comparar UID/GID del host, user namespace y usuario configurado.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

Identidad y permisos.

```bash
id
```
```bash
unshare --user --map-root-user id
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- El proceso ve identidad diferente dentro del namespace.
- La evidencia explica que esto no aísla el filesystem.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
