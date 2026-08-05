# Lab 12 — kata-containers

**Estado:** `documented` · **Nivel:** `advanced`

## Objetivo

Estudiar una frontera de VM para contenedores.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

Kata Containers.

```bash
kata-runtime --version 2>/dev/null || true
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- Registrar hipervisor y kernel invitado.
- Comparar RAM e inicio con gVisor y bwrap.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
