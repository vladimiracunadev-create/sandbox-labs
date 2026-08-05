# Roadmap

## v0.6.0 — Foundation & Codex Handoff

- Contratos tipados para catálogo, políticas, workloads, trabajos y evidencias.
- CLI, adaptadores experimentales, Control Center, pruebas negativas y CI.
- Estados honestos: `ready`, `experimental`, `documented`, `manual` y `planned`.

## v0.7.0 — Rootless Execution

- Bubblewrap validado en Linux.
- cgroups v2 y seccomp aplicados y observados.
- Métricas reales de CPU, memoria y procesos.
- Pruebas positivas y negativas reproducibles antes de marcar el runtime `ready`.

## v0.8.0 — WASI Portable Runner

- Módulos reproducibles.
- Límites de Wasmtime mediante fuel/epoch y memoria.
- Ejecución portable y matriz Windows, Linux y macOS.

## v0.9.0 — OCI Isolation

- gVisor, bundles OCI efímeros y comparación con un runtime OCI convencional.
- Evidencias de syscalls, filesystem y red efectivamente aplicadas.

## v1.0.0 — MicroVM & Platform

- Firecracker con jailer, artefactos verificados, red dedicada y snapshots.
- Jobs persistentes, reportes comparativos y matriz pública de seguridad.
