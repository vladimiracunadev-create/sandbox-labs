# Roadmap

## v0.6.0 — Foundation & Codex Handoff

- Contratos tipados para catálogo, políticas, workloads, trabajos y evidencias.
- CLI, adaptadores experimentales, Control Center, pruebas negativas y CI.
- Estados honestos: `ready`, `experimental`, `documented`, `manual` y `planned`.

## v0.7.0 — Rootless Execution

- ✅ Bubblewrap validado en Linux: la suite de contención corre en CI y mide las
  siete dimensiones con bubblewrap instalado de verdad.
- ✅ cgroups v2 aplicados **y observados**: `memory.max`, `pids.max` y `cpu.max`
  a través de un scope de systemd, con `memory.peak`, `pids.peak`, `cpu.stat` y
  el contador de OOM leídos mientras la carga corre.
- ✅ Métricas reales de CPU, memoria y procesos en `limits.observed` de la
  evidencia.
- ✅ Identidad no privilegiada: `--uid`/`--gid` de la política, aplicados.
- ✅ Red contenida también en los servicios, mediante reenviador del supervisor.
- ✅ Evidencia verificable: `sandboxctl evidence verify`.
- ⛔ **seccomp**: pendiente. Los perfiles existen y ningún runtime los aplica —
  ver [B-05](docs/IMPLEMENTATION_BACKLOG.md).
- ⛔ **Egress con allowlist**: pendiente, sin enforcement — ver B-04.
- ⛔ **Un solo compilador de política** para cargas y servicios — ver B-07.

Nada de esto marca el runtime como `ready`: sigue siendo `experimental` hasta
que los huecos de arriba estén cerrados.

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
