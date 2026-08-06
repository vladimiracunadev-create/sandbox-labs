# 📋 Changelog

Formato basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/).
Versionado semántico.

---

## [Unreleased]

### Fixed — tres afirmaciones del núcleo que no eran ciertas

- **La evidencia declaraba una red aislada que bubblewrap no aislaba.** El
  adaptador escribía siempre `network → "isolated network namespace"` en los
  límites efectivos, pero `--unshare-net` solo se añade con `network.mode:
  none`. La política de **todos** los servicios del catálogo pedía otra cosa, así
  que justo donde la carga conservaba la red del host la evidencia decía lo
  contrario. El cálculo salió a la función pura `effective_limits`, con pruebas.
- **`loopback` era un sinónimo suave de «sin aislar».** De los cuatro modos de
  red, solo `none` creaba namespace propio; los otros tres conservaban la red
  del host entera. Ahora `none` y `loopback` crean namespace, `allowlist` y
  `unrestricted` no, y la diferencia vive en `NetworkPolicy::isolates_host_network()`
  en vez de en cuatro comparaciones sueltas contra la cadena `"none"`.
- **`ai-agent-restricted` vendía un filtrado de egress inexistente.** No hay
  proxy de salida ni reglas de firewall que hagan cumplir `network.hosts`. La
  política se queda como la frontera que se quiere, pero su descripción avisa de
  que ningún runtime la aplica y de que, al ser estricta, no ejecuta.
- **Un servicio TCP con red aislada esperaba veinte segundos a nada.** El puerto
  nacía dentro del namespace y no era alcanzable desde el host. Ahora
  `sandboxctl service up` falla en cerrado, nombra el modo que lo provoca y da
  las dos salidas.
- **Las variables del bus de systemd se filtraban al PID 1 del sandbox.**
  Regresión introducida al añadir cgroups y encontrada por la suite de
  contención en CI, no por una revisión: `--clearenv` de bubblewrap limpia el
  entorno de la carga, no el del propio `init`, así que la carga leía
  `XDG_RUNTIME_DIR` y `DBUS_SESSION_BUS_ADDRESS` en `/proc/1/environ`. Se borran
  con `env -u` entre el scope y el runtime.

### Added — límites de recursos que existen de verdad

- **cgroups v2 en bubblewrap.** `memoryMb`, `processes` y `cpu` pasan de
  documentación a `memory.max`, `pids.max` y `cpu.max` a través de
  `systemd-run --user --scope`. Escribir el cgroup a mano no vale en la
  plataforma objetivo: en WSL2 el proceso arranca en `/init.scope`, que existe y
  no es escribible.
- **El sondeo usa el mecanismo en vez de suponerlo.** Antes de la primera
  ejecución se levanta un scope real con los tres límites puestos —la misma
  forma de comando que se ejecutará después— y solo si el kernel los acepta se
  declaran los controles. `sandboxctl doctor` muestra el resultado y deja de
  comprobar únicamente que `/sys/fs/cgroup/cgroup.controllers` exista.
- **`docs/IMPLEMENTATION_BACKLOG.md`**: los huecos del núcleo con qué falta, qué
  se hace en su lugar y qué haría falta para cerrarlos. El código lo enlazaba
  desde un comentario y no existía.

### Added — el repositorio deja de describir aislamiento y pasa a medirlo

- **`sandboxctl escape`: suite de contención.** Siete sondas que **intentan
  salirse** del sandbox (red, filesystem, visibilidad de procesos, fuga de
  entorno, privilegios efectivos, memoria y procesos) ejecutadas bajo cada
  runtime, con una matriz de resultados. Cada sonda es una carga registrada
  normal: se ejecuta por el mismo camino que el resto, porque una vía especial
  no mediría el sistema real.
- **Veredicto `❌ DECLARADO`** para el caso más peligroso: el runtime declara el
  control y la sonda demuestra que no lo aplica. Peor que no declararlo, porque
  invita a confiar.
- **`sandboxctl bench`: comparativa entre fronteras.** Misma carga y misma
  política en todos los runtimes, con p50, p95 y sobrecoste contra el más
  rápido. Repetición de calentamiento descartada; se reporta la cola porque una
  media sola esconde justo el caso que hará esperar al usuario.
- **Trabajo `isolation` en CI**: instala bubblewrap y ejecuta la suite de
  verdad. Tres comprobaciones que se sostienen entre sí — bubblewrap debe
  contenerlo todo (`--strict`), unshare debe cortar red y PIDs, y **native
  debe ESCAPAR**. Esta última es una contraprueba deliberada: si sin
  aislamiento saliera todo contenido, las sondas no estarían midiendo nada.
- Política `containment-audit`: `best-effort` a propósito, porque una `strict`
  falla cerrada antes de ejecutar y no mediría nada.
- `docs/CONTAINMENT_SUITE.md` y esquema `escape-suite.schema.json`.

### Fixed — hallazgos de la propia suite

- **PID namespace sin `/proc` remontado.** El adaptador `unshare` pasaba
  `--pid --fork` y creaba el namespace, pero sin `--mount-proc` el proceso
  seguía leyendo el `/proc` del host y enumeraba sus 48 PIDs. El namespace
  existía y no se notaba.
- **`RLIMIT_NPROC` no es un límite de procesos de contenedor.** Los adaptadores
  declaraban el control `processes` porque envolvían la carga con
  `prlimit --nproc`, pero RLIMIT_NPROC cuenta los procesos del UID en **todo el
  host**: fijarlo al presupuesto de la política mataba la ejecución al arrancar
  y hacía pasar por contención algo que no lo era. Se retiró, y el control
  `processes` ya no se declara hasta que exista con cgroups v2.

### Changed — laboratorios profesionales

- Los 18 laboratorios reescritos: de plantillas de 35 líneas a ~105 líneas con
  concepto, motivo, diagrama Mermaid, comandos reales sobre la nueva
  herramienta, salida esperada, verificación, caso de uso y errores comunes.
- Estado de cada laboratorio sincronizado con el catálogo, **con una prueba de
  contrato que impide que vuelvan a divergir** y que además exige que cada
  README traiga diagrama, práctica y verificación.
- Los adaptadores `bwrap` y `unshare` declaran ahora `memory` (RLIMIT_AS
  verificado en el host) y ya no declaran `processes`.

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
