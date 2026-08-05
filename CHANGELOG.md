# 📋 Changelog

Formato basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/).
Versionado semántico.

---

## [0.7.0] - 2026-08-05

Primera versión **ejecutada de extremo a extremo**. La 0.6.0 se entregó sin que
la suite llegara a correr; esta corrige los fallos que lo impedían, multiplica
la cobertura y rehace panel y documentación.

### Fixed

- Los validadores de Node resolvían la raíz con `new URL("..", import.meta.url).pathname`,
  que en Windows devuelve `/C:/…` y produce rutas `C:\C:\…`. Afectaba a
  `run-negative-tests`, `validate-evidence`, `cleanup-test-state` y
  `generate-file-manifest`.
- `cargo fmt --all -- --check` fallaba en los tres crates: formato aplicado.
- La prueba de API derivaba la raíz del repositorio de `process.cwd()`, que
  apunta a `control-center/` cuando corre vía `pnpm --dir` o `make`.
- La prueba de symlink abortaba en Windows sin privilegios; ahora se salta con
  motivo explícito.
- El watchdog del Control Center mataba el trabajo cuando `sandboxctl` se
  invocaba vía `cargo run`: la compilación consumía el timeout de la política.
  El arranque ya no se descuenta del tiempo de ejecución de la carga.
- `native` reportaba «runtime no disponible» en lugar del motivo accionable
  (falta el opt-in `SANDBOX_LABS_ALLOW_NATIVE`).
- El servidor escribía en consola los 4xx del cliente como errores del servidor.
- `pnpm/action-setup` declaraba `version: 9` mientras `package.json` ya fija
  `packageManager`; la acción abortaba y los jobs de Node no llegaban a correr.
- `with: { components: rustfmt, clippy }` es un mapa de flujo YAML: la acción
  descartaba clippy en silencio.
- El panel desbordaba horizontalmente en móvil por el ancho intrínseco de los
  `<select>` con opciones largas.

### Added

- **16 pruebas de contrato del repositorio** en Rust (`tests/repository.rs`):
  catálogo contra `labs/`, carga y validación de todas las políticas y cargas,
  cargas de riesgo sin `allowNative`, fail-closed de políticas estrictas,
  particionado de controles, determinismo de hashes y rutas portables.
- **Pruebas del Control Center** ampliadas de 8 a 16: catálogo sin rutas del
  host, referencias no registradas, argumentos inválidos, anti DNS-rebinding
  con cliente HTTP crudo, traversals que sobreviven a la normalización de URL y
  cabeceras de seguridad.
- Previsión de controles en el panel: antes de crear el trabajo anuncia qué
  controles quedarán efectivos y si la política bloqueará.
- Estado en vivo por **SSE** en la interfaz (antes solo sondeaba cada 3 s).
- Portada estática en `site/` para GitHub Pages.
- Documentación nueva: `FAQ.md`, `GLOSSARY.md`, `SUPPORT.md`,
  `COMPATIBILITY.md`, `ENVIRONMENT_SETUP.md`, `FILE_ARCHITECTURE.md`,
  `OPERATING-MODES.md`, `CODE_OF_CONDUCT.md`, `docs/TROUBLESHOOTING.md` y
  `docs/DOCUMENTATION_INDEX.md`.
- `.gitattributes` que fija LF en todo lo ejecutable: un `.sh` con CRLF rompe CI.
- `.markdownlint.json` y `version.txt`.
- CI verifica que la evidencia generada por el CLI cumple su esquema.
- `security.yml` añade escaneo de secretos con gitleaks sobre el historial y
  auditoría de los propios workflows con zizmor.

### Changed

- Panel rediseñado con el lenguaje visual de los repositorios hermanos
  (`shell`, `hero`, `eyebrow`, `metrics`, `section-head`, `card-grid`, `logs`),
  con soporte de esquema claro y oscuro, enlace de salto, foco visible y
  respeto a `prefers-reduced-motion`.
- `README.md` reescrito: badges, tablas, diagramas Mermaid y rutas de lectura.
- `VALIDATION.md` reescrito para reflejar lo que se ejecutó de verdad, con una
  sección explícita de lo que queda fuera del alcance.
- `Cargo.lock` versionado; CI ya no lo regenera en cada ejecución, de modo que
  `--locked` vuelve a significar algo.
- `check-doc-links.mjs` y `generate-file-manifest.mjs` reescritos legibles; el
  primero ahora reporta todos los enlaces rotos, no solo el primero.
- `actions/checkout` v4 → v5.

---

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
