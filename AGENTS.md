# Instrucciones para agentes de desarrollo

## Objetivo

Convertir `sandbox-labs` en una plataforma verificable sin degradar silenciosamente controles de seguridad.

## Reglas obligatorias

1. Usa `pnpm`; no introduzcas `npm install` ni `yarn`.
2. Mantén el Control Center sin comandos arbitrarios. Solo IDs registrados y argumentos validados.
3. Una policy `strict` debe rechazar la ejecución cuando un control requerido no pueda aplicarse.
4. No marques un runtime como `ready` hasta tener prueba positiva, prueba negativa y evidencia.
5. No ejecutes cargas adversariales en `native`.
6. Actualiza esquema, documentación, pruebas y catálogo en el mismo cambio.
7. No edites evidencias generadas como si fueran fixtures; usa `tests/fixtures/` para datos estáticos.
8. Conserva compatibilidad Windows + WSL2 para panel y planificación; los runtimes Linux pueden declarar requisito explícito.

## Secuencia de trabajo

```bash
node scripts/validate-config.mjs
node scripts/check-doc-links.mjs
node scripts/run-negative-tests.mjs
node scripts/validate-evidence.mjs
cd control-center && node scripts/build.mjs && node --test test/*.test.mjs
cd ..
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets -- -D warnings
```

## Prioridad

Sigue [docs/IMPLEMENTATION_BACKLOG.md](docs/IMPLEMENTATION_BACKLOG.md). Completa verticalmente Bubblewrap, luego WASI, antes de ampliar el catálogo.
