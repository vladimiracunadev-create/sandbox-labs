# 📋 Changelog

Formato basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/).
Versionado semántico.

---

## [Unreleased]

### Security

- `sha2` actualizado a 0.11 (cambio mayor). La versión 0.11 dejó de
  implementar `LowerHex` sobre la salida del digest, así que la codificación
  hexadecimal se centraliza en `sandbox_core::hash` en lugar de repetirse en
  cada llamador: la próxima actualización de la dependencia deja de ser una
  migración a mano. El módulo trae vectores de prueba del NIST.

- **Todas las acciones de GitHub fijadas a SHA** con el tag en comentario. Un
  tag es mutable: `@v5` puede apuntar mañana a otro código. `zizmor` lo
  verifica en cada ejecución, así que deja de ser una convención olvidable.
- `persist-credentials: false` en todos los checkouts: por defecto el token de
  Actions queda en `.git/config` y cualquier paso posterior puede leerlo.
  Ningún workflow del repositorio necesita empujar commits.
- Permisos reducidos al mínimo por trabajo. `pages: write` e `id-token: write`
  ya no se declaran a nivel de workflow.
- El workflow de release ya no restaura caché de pnpm: no debe consumir una
  caché que otra rama pudo haber escrito.
- `softprops/action-gh-release` sustituido por `gh release create`, que ya
  viene en el runner — una acción de terceros menos en la ruta que firma el
  release.

### Added

- **Trabajo `panel` en CI**: arranca el servidor real y comprueba el contrato
  de la API de extremo a extremo (modo seguro, `403` sin cabecera de
  confianza, `404` en comandos arbitrarios, `421` anti DNS-rebinding y un
  trabajo registrado que llega a estado terminal con evidencia).
- CI verifica que `control-center/dist/` versionado coincide con lo que genera
  el build: el build es determinista, así que una diferencia significa que
  alguien editó `src/` sin regenerar `dist/`.
- **`actionlint`** además de `zizmor` en el workflow de seguridad: corrección
  de sintaxis, expresiones y shell embebido, no solo seguridad.
- Workflow **Pages**, que publica `site/` y comprueba antes que la portada esté
  completa y no cargue recursos externos.
- Workflow **Release** rehecho: valida que la versión cuadre en los cinco
  manifiestos, ejecuta la puerta de calidad completa y después **abre el ZIP y
  cuenta lo que lleva dentro** — un artefacto puede compilar, cuadrar de
  checksum y estar vacío.
- Caché de dependencias de Cargo en CI.
- `timeout-minutes` en todos los trabajos.
- Resúmenes en `$GITHUB_STEP_SUMMARY` para Rust, Pages y Release.
- `docs/CI_WORKFLOWS.md`: qué garantiza cada workflow y cómo reproducirlo.

### Changed

- `docs.yml` se divide en dos trabajos —enlaces y lint— porque fallan por
  motivos distintos y el fallo debe decir qué arreglar.
- `dependabot.yml`: actualizaciones agrupadas por ecosistema, con etiquetas,
  prefijo de commit y límite de PRs abiertos.

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
