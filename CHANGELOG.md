# Changelog

## [0.6.0] - 2026-08-05

### Added

- Workspace Rust modular con dependencias declaradas y generación reproducible de `Cargo.lock` en CI.
- RuntimeAdapter y adaptadores dry-run, native, Bubblewrap, unshare, WASI y avanzados fail-closed.
- Policies strict/best-effort y controles requested/effective/unsupported.
- Evidencia con hashes SHA-256, host, runtime, límites y resultados.
- API de trabajos, cancelación, SSE y fallback de planificación.
- Esquemas de workload y job request.
- Pruebas negativas, validación profunda de evidencias y seguridad de archivos estáticos.
- Protección anti DNS-rebinding, cancelación con escalamiento a SIGKILL y logs visibles en el panel.
- Handoff específico para Codex.

### Changed

- Estados normalizados a ready, experimental, documented, manual y planned.
- Build del Control Center corregido a `dist/server.js`.
- CI genera `Cargo.lock`, ejecuta Cargo con `--locked` y usa instalación pnpm congelada.
