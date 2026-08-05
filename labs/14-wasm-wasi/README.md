# Lab 14 — wasm-wasi

**Estado:** `experimental` · **Nivel:** `intermediate`

## Objetivo

Ejecutar módulos con capacidades y preopens mínimos.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

WASM/WASI.

```bash
cargo run -p sandboxctl -- plan --workload workloads/benign/wasi-hello --runtime wasi --policy policies/minimal.json
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- Compilar `hello.wasm` de forma reproducible.
- Aplicar fuel/epoch y memoria antes de ready.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
