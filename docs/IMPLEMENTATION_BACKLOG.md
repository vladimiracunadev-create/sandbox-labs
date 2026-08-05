# Backlog de implementación para Codex

## P0 — Compilación y smoke tests

- [ ] Compilar Rust 1.78+ en Linux y Windows.
- [ ] Ejecutar `cargo fmt`, tests y clippy.
- [ ] Verificar deserialización y validación con corpus JSON válido/inválido.
- [ ] Ejecutar `scripts/generate-lockfiles.sh` y confirmar `Cargo.lock` con el toolchain objetivo.
- [ ] Ejecutar Control Center y su suite Node.

**Aceptación:** CI verde y `sandboxctl doctor/labs/plan` operativos.

## P1 — Bubblewrap vertical

- [ ] Validar `--new-session`, `--die-with-parent`, user/PID/mount/IPC/UTS/network namespaces.
- [ ] Confirmar mounts mínimos por distro.
- [ ] Evitar symlinks y bind sources fuera del repo.
- [ ] Exportar `/workspace/output` de forma segura.
- [ ] Integrar cgroups v2 con unidad/transient scope o helper dedicado.
- [ ] Medir peak memory, CPU y procesos.
- [ ] Agregar seccomp compilado desde perfiles.
- [ ] Ejecutar los tres escenarios negativos.

**Aceptación:** Bubblewrap pasa prueba positiva y negativas, genera evidencia con controles efectivos observados y cambia a estado `ready`.

## P2 — WASI portable

- [ ] Incorporar fuente y build reproducible de `wasi-hello`.
- [ ] Fijar versión Wasmtime.
- [ ] Configurar preopens exactos.
- [ ] Implementar fuel/epoch interruption y memory limit.
- [ ] Confirmar ausencia de sockets y variables no autorizadas.

**Aceptación:** mismo workload reproducible en Linux, Windows y macOS.

## P3 — API y producto

- [ ] Persistir trabajos al reiniciar y rehidratar estados.
- [ ] Añadir cuotas por usuario/tenant antes de exponer fuera de localhost.
- [ ] Firmar evidencias opcionalmente.
- [ ] Exportar informe HTML/PDF.
- [ ] Añadir comparación de ejecuciones.
- [ ] Añadir autenticación antes de cualquier bind no local.

## P4 — OCI y microVM

- [ ] gVisor: bundle OCI, rootfs, `runsc create/start/delete`.
- [ ] Kata: runtime handler y prueba de frontera VM.
- [ ] Firecracker: jailer, kernel/rootfs mínimos, tap/network namespace, snapshots y cleanup.

**Aceptación:** cada runtime avanzado tiene runbook, doctor, prueba positiva, prueba negativa y evidencia.

## Archivos clave

- `crates/sandbox-runtimes/src/lib.rs`
- `crates/sandbox-core/src/policy.rs`
- `crates/sandbox-core/src/evidence.rs`
- `control-center/src/jobs.ts`
- `tests/scenarios/`
- `schemas/`
