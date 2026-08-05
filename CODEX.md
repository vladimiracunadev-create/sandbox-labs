# Codex handoff

El repositorio ya contiene contratos, esquemas, CLI, UI y pruebas de regresión. Codex debe concentrarse en validar y completar la ejecución real por plataforma.

Empieza por:

1. Compilar el workspace Rust y corregir cualquier diferencia de toolchain.
2. Ejecutar `hello` con native opt-in en una VM de desarrollo.
3. Ejecutar `hello`, `path-traversal-simulation` y `network-egress-simulation` con Bubblewrap.
4. Confirmar `requested/effective/unsupported` usando datos observados, no supuestos.
5. Agregar cgroups v2 y medición de memoria/CPU.
6. Compilar `wasi-hello` y completar pruebas Wasmtime.
7. Incorporar el repositorio a GitHub y activar CI solo después de validar localmente.

Consulta [AGENTS.md](AGENTS.md) y [docs/IMPLEMENTATION_BACKLOG.md](docs/IMPLEMENTATION_BACKLOG.md).
