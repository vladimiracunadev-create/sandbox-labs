# Lab 05 — cgroups-limits

**Estado:** `documented` · **Nivel:** `intermediate`

## Objetivo

Aplicar CPU, memoria y pids como límites medibles.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

cgroups v2.

```bash
cat /sys/fs/cgroup/cgroup.controllers
```

```bash
cargo run -p sandboxctl -- doctor
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- Medir `memory.peak`, `cpu.stat` y `pids.current`.
- Codex debe integrar cgroups antes de marcar ready.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
