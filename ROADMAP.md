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
- ✅ **Egress con allowlist**: la carga corre sin red y sale solo por un canal
  explícito que aplica la lista y registra cada intento.
- ✅ **Un solo compilador de política** para cargas y servicios, con lo que los
  servicios ganan identidad, capabilities, cgroups y seccomp.

El runtime sigue siendo `experimental`, y no por una lista de tareas: lo que
falta es tiempo de uso y de ataque contra él. Lo que sí está es que cada control
que declara se puede medir, y que la suite de contención lo mide en cada commit.

Los límites que se mantienen: no es seguro para malware real, ni para
multi-tenancy hostil, ni para producción. Para carga desconocida, VM
desechable.

## Mercado de capitales — cimientos

- ✅ Catálogo con dos familias que no se mezclan.
- ✅ Dinero exacto en unidades mínimas, con la moneda pegada al importe.
- ✅ Libro mayor de doble entrada, solo-añadir, con sus invariantes probadas.
- ⛔ Motor de escenarios con semilla, reloj simulado, participantes,
  instrumentos y política regulatoria como código.
- ✅ **CM-03 · Custodia y segregación**: seis escenarios que se ejecutan y
  declaran lo que esperan detectar.
- ⛔ Los otros veinte casos. Ver
  [domains/capital-markets/](domains/capital-markets/README.md).

Dinero simulado, sin autorización de ninguna autoridad y sin recomendaciones de
inversión. Eso no cambia con el avance del roadmap.

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
