# Estrategia de pruebas

## Estáticas

```bash
node scripts/validate-config.mjs
node scripts/check-doc-links.mjs
node scripts/run-negative-tests.mjs
node scripts/validate-evidence.mjs
```

## Control Center

```bash
cd control-center
node scripts/build.mjs
node --test test/*.test.mjs
```

Incluye path traversal codificado, backslashes, NUL, symlink fuera del public root, Host anti DNS-rebinding, CSRF local, creación de trabajos registrados y validación del documento de evidencia generado.

## Rust

```bash
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets -- -D warnings
```

## Pruebas negativas de runtime

Ejecutar en VM:

1. `hello` debe completar.
2. `path-traversal-simulation` no debe leer archivos fuera del workspace.
3. `network-egress-simulation` debe fallar con policy `minimal`.
4. `memory-pressure` debe ser rechazado en native.
5. una policy strict debe producir evidencia `blocked` si falta un control.
