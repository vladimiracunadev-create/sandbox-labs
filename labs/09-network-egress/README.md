# Lab 09 — network-egress

**Estado:** `experimental` · **Nivel:** `intermediate`

## Objetivo

Diferenciar none, loopback y allowlist real.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

Network egress.

```bash
cargo run -p sandboxctl -- plan --workload workloads/adversarial/network-egress-simulation --runtime bwrap --policy policies/minimal.json
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- `none` usa network namespace.
- Allowlist requiere proxy/firewall; DNS no basta.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
