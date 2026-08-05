# 🗺️ Arquitectura de archivos

Mapa del repositorio: qué vive en cada carpeta y de qué se hace responsable.
Si buscas el *porqué* de las capas, lee [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

---

## 📦 Vista general

```text
sandbox-labs/
├── crates/                  # 🦀 El motor: CLI + núcleo + adaptadores de runtime
├── control-center/          # 🧭 Panel local (API + UI) sin dependencias npm
├── policies/                # 🛡️ Perfiles de aislamiento reproducibles
├── workloads/               # 📦 Cargas registradas (lo único ejecutable)
├── schemas/                 # 📐 Contratos JSON Schema de todo lo anterior
├── labs/                    # 🧪 18 recorridos educativos
├── adapters/                # 📓 Notas y ejemplos por runtime
├── tests/scenarios/         # 🚫 Contratos negativos declarativos
├── scripts/                 # 🔧 Validadores y utilidades (Node + bash)
├── tools/                   # 🩺 Preflight del host
├── launcher/windows/        # 🪟 Arranque del panel desde Windows
├── docs/                    # 📚 Documentación técnica
├── site/                    # 🌐 Portada publicada en GitHub Pages
├── benchmarks/              # 📊 Matriz de comparación entre runtimes
├── profiles/seccomp/        # 🧷 Perfiles seccomp de referencia
└── evidence/runs/           # 🧾 Salidas JSON (ignoradas por Git)
```

---

## 🦀 `crates/` — el motor

| Crate | Responsabilidad | No hace |
|---|---|---|
| `sandbox-core` | Modelos tipados, validación, compilación de plan, hashes y evidencia | No ejecuta procesos |
| `sandbox-runtimes` | `RuntimeAdapter`, supervisor de procesos, adaptadores | No decide políticas |
| `sandboxctl` | CLI: `doctor`, `labs`, `runtimes`, `validate`, `plan`, `run` | No expone servidor |

**Archivos clave**

| Archivo | Qué resuelve |
|---|---|
| [`crates/sandbox-core/src/policy.rs`](crates/sandbox-core/src/policy.rs) | Modelo de política y sus invariantes |
| [`crates/sandbox-core/src/runtime.rs`](crates/sandbox-core/src/runtime.rs) | Sondeo de runtimes y construcción del plan (fail-closed) |
| [`crates/sandbox-core/src/evidence.rs`](crates/sandbox-core/src/evidence.rs) | Formato de evidencia y hashes de integridad |
| [`crates/sandbox-core/src/workload.rs`](crates/sandbox-core/src/workload.rs) | Manifiestos, entrypoint contenido y hash de contenido |
| [`crates/sandbox-runtimes/src/process.rs`](crates/sandbox-runtimes/src/process.rs) | Supervisor con timeout y truncado de salida |
| [`crates/sandbox-core/tests/repository.rs`](crates/sandbox-core/tests/repository.rs) | Contratos del repositorio (catálogo ↔ disco) |

---

## 🧭 `control-center/` — el panel

| Archivo | Qué resuelve |
|---|---|
| [`src/server.ts`](control-center/src/server.ts) | Rutas HTTP, cabeceras de seguridad, estáticos |
| [`src/jobs.ts`](control-center/src/jobs.ts) | Cola de trabajos, invocación del CLI, cancelación, evidencia de reserva |
| [`src/security.ts`](control-center/src/security.ts) | Rutas seguras, Host de confianza, validación de entrada |
| [`src/registry.ts`](control-center/src/registry.ts) | Carga del catálogo, políticas y manifiestos |
| [`src/paths.ts`](control-center/src/paths.ts) | Raíces del repositorio en un solo sitio |
| [`public/`](control-center/public/) | UI: `index.html`, `styles.css`, `app.js` (sin bundler) |
| [`scripts/build.mjs`](control-center/scripts/build.mjs) | «Build»: copia `src/*.ts` a `dist/*.js` reescribiendo imports |

> [!NOTE]
> `dist/` está versionado a propósito: el panel debe arrancar en un host sin
> toolchain de TypeScript. `pnpm dashboard:build` lo regenera.

---

## 🛡️ `policies/` y 📦 `workloads/`

Los dos registros que definen qué puede ejecutarse y bajo qué reglas.

| Ruta | Contenido |
|---|---|
| `policies/*.json` | Un perfil por archivo; el `id` coincide con el nombre |
| `workloads/benign/` | Cargas seguras para el baseline |
| `workloads/resource-abuse/` | Presión de memoria y procesos — nunca en `native` |
| `workloads/adversarial/` | Simulaciones de fuga — nunca en `native` |

Cada carga lleva `manifest.json` validado contra
[`schemas/workload.schema.json`](schemas/workload.schema.json).

---

## 🔧 `scripts/` — validadores

| Script | Qué verifica |
|---|---|
| `validate-config.mjs` | El catálogo y todos los JSON contra sus esquemas |
| `check-doc-links.mjs` | Que ningún enlace relativo de los `.md` esté roto |
| `run-negative-tests.mjs` | Coherencia de los contratos negativos |
| `validate-evidence.mjs` | Que cada evidencia generada cumpla su esquema |
| `cleanup-test-state.mjs` | Borra `.sandbox-data/` y las evidencias locales |
| `generate-file-manifest.mjs` | Regenera `FILE_MANIFEST.txt` (SHA-256 por archivo) |
| `package-release.sh` | Empaqueta el ZIP de release y su checksum |
| `doctor.sh` | Preflight del host desde bash |

---

## 📄 Archivos raíz

| Archivo | Rol |
|---|---|
| `sandbox.config.json` | **Fuente única de verdad**: labs, runtimes, rutas, versión |
| `Cargo.toml` / `Cargo.lock` | Workspace Rust; el lock está versionado |
| `package.json` | Scripts de validación y del panel |
| `Makefile` | Atajos: `check`, `test`, `build`, `dashboard`, `doctor` |
| `version.txt` | Versión en texto plano para scripts e instaladores |
| `FILE_MANIFEST.txt` | SHA-256 de cada archivo versionado |
| `.gitattributes` | Fija LF: un `.sh` con CRLF rompe CI |

---

## 🔗 Ver también

- [Índice de documentación](docs/DOCUMENTATION_INDEX.md)
- [Arquitectura](docs/ARCHITECTURE.md)
- [Modos de operación](OPERATING-MODES.md)
