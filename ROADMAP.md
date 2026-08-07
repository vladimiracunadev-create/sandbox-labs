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
- ✅ **seccomp**: `policy.syscalls.deny` compilado a BPF y aplicado por
  bubblewrap, con sonda que lo mide comparando errno.
- ⛔ **Egress con allowlist**: pendiente, sin enforcement — ver B-04.
- ✅ **Un solo compilador de política** para cargas y servicios, con lo que los
  servicios ganan identidad, capabilities, cgroups y seccomp.

Queda un hueco, y hasta cerrarlo el runtime sigue siendo `experimental`: el
egress con `allowlist` no tiene enforcement, así que ese control no se declara
nunca y una política estricta que lo exija no ejecuta.

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
