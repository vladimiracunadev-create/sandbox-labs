# Lab 07 — seccomp-syscalls

**Estado:** `documented` · **Nivel:** `intermediate`

## Objetivo

Compilar una allowlist de syscalls por workload.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

Seccomp.

```bash
cat profiles/seccomp/strict.json
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- El perfil de ejemplo no es universal.
- Una syscall bloqueada debe aparecer como violación.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
