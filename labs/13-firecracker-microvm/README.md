# Lab 13 — firecracker-microvm

**Estado:** `manual` · **Nivel:** `advanced`

## Objetivo

Diseñar ejecución con KVM, jailer y rootfs mínimo.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

Firecracker microVM.

```bash
test -e /dev/kvm && echo KVM
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- No iniciar desde el panel sin provisioning.
- Kernel/rootfs verificados y cleanup de red.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
