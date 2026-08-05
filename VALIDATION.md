# Validación de la entrega

Ejecutado en el entorno de generación sobre 18 laboratorios, 8 runtimes y 7 workloads registrados:

- `node scripts/validate-config.mjs`
- `node scripts/run-negative-tests.mjs` — 3 contratos negativos coherentes
- `node scripts/validate-evidence.mjs`
- `node scripts/check-doc-links.mjs`
- `node control-center/scripts/build.mjs`
- `node --test control-center/test/*.test.mjs` — 8/8 pruebas aprobadas
- Smoke test HTTP del Control Center y creación `dry-run` en servidor local efímero.
- Validación de Host anti DNS-rebinding, rutas codificadas y symlinks externos.
- Verificación de integridad del ZIP y generación SHA-256.

Limitaciones del entorno de generación:

- No contiene `rustc` ni `cargo`, por lo que el workspace Rust no pudo compilarse aquí.
- El comando `pnpm` está administrado por Corepack, pero la versión fijada requiere una descarga y el entorno no tiene acceso a npm. Como el proyecto Node no tiene dependencias externas, se validó directamente con Node.js 22.
- No están instalados Bubblewrap, Wasmtime, gVisor, Kata Containers ni Firecracker.

Codex o CI deben generar `Cargo.lock`, compilar el workspace y ejecutar las pruebas de integración en hosts que dispongan de los runtimes correspondientes antes de declarar esos adaptadores como operativos.
