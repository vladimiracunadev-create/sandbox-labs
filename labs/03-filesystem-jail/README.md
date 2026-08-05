# Lab 03 — filesystem-jail

**Estado:** `documented` · **Nivel:** `initial`

## Objetivo

Separar workload read-only y output escribible.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

Filesystem jail.

```bash
cargo run -p sandboxctl -- plan --workload workloads/benign/filesystem-probe --runtime bwrap --policy policies/minimal.json
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- No seguir symlinks.
- Lecturas fuera del workspace deben bloquearse.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
