# Lab 08 — landlock-policies

**Estado:** `documented` · **Nivel:** `intermediate`

## Objetivo

Aplicar restricciones voluntarias de filesystem desde el proceso.

## Preparación

Ejecuta `cargo run -p sandboxctl -- doctor` y trabaja en una VM cuando el laboratorio toque límites del kernel.

## Práctica

Landlock.

```bash
uname -r
```

## Evidencia esperada

- policy y workload identificados por hash;
- runtime y versión;
- controles solicitados, efectivos y no soportados;
- stdout/stderr acotados;
- limitaciones del host;
- prueba negativa cuando corresponda.

## Criterios de finalización

- Detectar ABI del kernel.
- Agregar adapter solo tras prueba positiva y negativa.

> No ejecutes código desconocido en el host. `experimental` no significa apto para cargas hostiles.
