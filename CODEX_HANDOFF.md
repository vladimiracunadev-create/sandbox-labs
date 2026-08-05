# Handoff para Codex

Este ZIP deja una base de ingeniería completa y un flujo `dry-run` funcional. Codex debe validar y endurecer los runtimes en hosts reales.

## Primera sesión

```bash
corepack enable
pnpm install --frozen-lockfile
pnpm check
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p sandboxctl -- doctor
cargo run -p sandboxctl -- run --workload workloads/benign/hello --runtime dry-run --policy policies/minimal.json
```

## Prioridad 1: Bubblewrap

- Ejecutar las pruebas negativas en Linux rootless.
- Verificar que `/etc`, `$HOME`, sockets del host y variables secretas no sean visibles.
- Añadir cgroup v2 delegado; no presentar `RLIMIT_AS` como sustituto de una cuota cgroup.
- Incorporar seccomp compilado al adaptador.
- Conservar `--die-with-parent`, `--new-session`, user/mount/pid/network namespaces y capacidades vacías.
- Marcar `bwrap` como `ready` únicamente tras CI en host compatible.

## Prioridad 2: WASI

- Fijar una versión de Wasmtime y ajustar opciones CLI a esa versión.
- Incorporar fuel/epoch interruption y memoria máxima.
- Crear artefacto `.wasm` reproducible para `wasi-hello`.
- Probar ausencia de red y directorios no preabiertos.

## Prioridad 3: Evidencia

- Medir memoria pico, CPU y motivo de señal.
- Registrar hash del binario del runner y versión del kernel.
- Añadir firma opcional Ed25519 y verificación.
- Generar informe HTML sin ejecutar contenido incluido en stdout/stderr.

## Prioridad 4: gVisor, Kata y Firecracker

- gVisor: generar bundle OCI mínimo y comparar `runc`/`runsc`.
- Kata: integrar mediante containerd sin comandos arbitrarios.
- Firecracker: usar `jailer`, KVM, kernel/rootfs inmutables y red aislada; no habilitar desde el panel en Windows.

## Criterio de término

Un runtime es funcional cuando una ejecución permitida finaliza y todas las pruebas de acceso prohibido fallan de la forma esperada, con evidencia que prueba los controles efectivos.
