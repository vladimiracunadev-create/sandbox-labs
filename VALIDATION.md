# ✅ Validación de la entrega

Registro de lo que se ejecutó **de verdad** en esta versión, con su resultado.
Este documento no describe intenciones: si algo no se pudo verificar, aparece
en «Fuera del alcance de esta verificación».

> **Versión**: 0.7.0
> **Entorno**: Windows 11 (host) + Ubuntu 24.04 sobre WSL2 · Node.js 24.11.1 ·
> Rust stable · kernel Linux 6.6

---

## 🦀 Workspace Rust

| Comando | Resultado |
|---|---|
| `cargo fmt --all -- --check` | ✅ sin diferencias |
| `cargo clippy --workspace --all-targets -- -D warnings` | ✅ sin avisos |
| `cargo test --workspace --locked` | ✅ **18 pruebas** (2 unitarias + 16 de contrato) |
| `cargo run -p sandboxctl -- doctor` | ✅ sondeo correcto del host |
| `cargo run -p sandboxctl -- validate …` | ✅ política y carga válidas |
| `cargo run -p sandboxctl -- run --runtime dry-run …` | ✅ plan y evidencia generados |

Contratos verificados en `crates/sandbox-core/tests/repository.rs`:

- El catálogo carga, valida y coincide con los directorios de `labs/`.
- Los 8 runtimes del catálogo son tipos conocidos, con `Display`/`FromStr` simétricos.
- Las 6 políticas cargan, validan y su `id` coincide con el nombre del archivo.
- Las 7 cargas cargan, validan y no repiten `id`.
- Ninguna carga de riesgo declara `allowNative`.
- `dry-run` nunca se marca ejecutable.
- `gvisor`, `kata` y `firecracker` nunca se marcan ejecutables.
- `native` se bloquea sin el opt-in explícito, con el motivo accionable.
- Una política `strict` con controles no soportados falla cerrada.
- Para toda política × runtime, `efectivos ∪ no soportados == solicitados`.
- Los argumentos extra hostiles (exceso, longitud, byte nulo) se rechazan.
- Los hashes de carga y de política son deterministas y distinguen contenidos.
- Las rutas de carga en la evidencia son portables (sin unidad ni `\` del host).

## 🧭 Control Center

| Comando | Resultado |
|---|---|
| `node control-center/scripts/build.mjs` | ✅ `dist/` regenerado |
| `node --test control-center/test/*.test.mjs` | ✅ **15 pruebas**, 1 saltada |

La prueba saltada es la de symlink: Windows no permite crearlos sin privilegios,
y se salta con motivo explícito en vez de reportar un fallo ajeno al código. En
Linux y en CI sí se ejecuta.

Cubierto: creación de trabajo registrado y evidencia conforme al esquema,
rechazo de escrituras sin cabecera de confianza (`403`), rechazo de
identificadores no registrados y de argumentos inválidos (`400`), bloqueo de
`Host` no confiable (`421`), traversals que sobreviven a la normalización de
URL (`400`), y cabeceras de seguridad en los estáticos.

## 🔍 Validadores del catálogo

| Comando | Resultado |
|---|---|
| `node scripts/validate-config.mjs` | ✅ 18 labs, 8 runtimes, 6 policies, 7 workloads |
| `node scripts/check-doc-links.mjs` | ✅ enlaces locales sin roturas |
| `node scripts/run-negative-tests.mjs` | ✅ 3 contratos negativos coherentes |
| `node scripts/validate-evidence.mjs` | ✅ evidencias conformes al esquema |
| `node scripts/cleanup-test-state.mjs` | ✅ estado temporal eliminado |

## 🖥️ Panel en ejecución

Verificado en navegador contra el servidor real en `127.0.0.1:9093`:

- Carga del catálogo, métricas, tarjetas de runtimes, políticas y laboratorios.
- Previsión de controles antes de ejecutar: con `high-risk` + `bwrap` anuncia
  el bloqueo por `processes, memory, cpu, syscalls`.
- Creación de un trabajo `hello` + `minimal` + `dry-run` → estado `planned`,
  logs en vivo por SSE y evidencia consultable.
- Sin desbordamiento horizontal a 375 px ni a 1280 px; esquema claro y oscuro.
- Sin errores en la consola del navegador.

## 🛡️ Contención medida (no declarada)

Ejecutado en el trabajo `isolation` de CI sobre `ubuntu-latest` con bubblewrap
0.9.0 y util-linux 2.39.3 instalados. **No es un ejemplo: es la salida del
runner**, y se repite en cada commit.

| Dimensión | `bwrap` | Detalle medido |
|---|:--:|---|
| Salida de red | ✅ | sin salida TCP ni resolución DNS |
| Filesystem | ✅ | ninguna ruta sensible del host es legible |
| Visibilidad de procesos | ✅ | solo 2 PIDs visibles, propio PID 2 |
| Fuga de entorno | ✅ | 4 variables, ninguna sensible |
| Privilegios | ✅ | `CapEff=0x0000000000000000` |
| Límite de memoria | ✅ | `MemoryError` tras 96 MB con presupuesto de 128 MB |
| Límite de procesos | ❌ | 32 procesos con presupuesto de 16 — **hueco conocido** |

| Dimensión | `unshare` | Detalle medido |
|---|:--:|---|
| Salida de red | ✅ | sin salida TCP ni resolución DNS |
| Visibilidad de procesos | ✅ | solo 1 PID visible, propio PID 1 |

**Contraprueba obligatoria:** `native` escapó por 3 dimensiones (red,
filesystem, procesos), como tiene que ser. Si sin aislamiento saliera todo
contenido, las sondas no estarían midiendo nada y los ✅ de arriba no valdrían
nada. CI falla si esa contraprueba deja de escaparse.

El ❌ de `process-limit` es correcto y deliberado: ningún runtime local aplica
todavía un techo real de PIDs, así que el control `processes` **no se declara**
y no hay falsa garantía. `--strict` falla por falsas garantías, no por huecos
documentados.

## 🤖 Integración continua

Cinco workflows en verde sobre `main`. El trabajo `isolation` de CI instala los
runtimes y ejecuta la suite de verdad; los informes JSON quedan como artefacto
del run.

---

## 🚧 Fuera del alcance de esta verificación

Estos puntos **no** se han probado, y por eso ningún adaptador sube a `ready`:

| Pendiente | Motivo |
|---|---|
| Ejecución real con `wasi` | `wasmtime` no está en el runner ni en el entorno local |
| Techo real de PIDs | Necesita el controlador `pids` de cgroups v2 |
| cgroups v2 como control efectivo | Requiere delegación de cgroup en el host |
| seccomp efectivo | El perfil existe pero todavía no se impone |
| gVisor, Kata y Firecracker | Requieren hosts dedicados con KVM y runtimes propios |
| Resistencia frente a un atacante real | La suite acota lo que sabes, no lo que temes |
| Persistencia y multi-tenancy | Fuera del alcance de la versión |

> [!NOTE]
> `bwrap` contiene seis de siete dimensiones de forma verificada en cada
> commit, y aun así sigue en `experimental`. La razón es la de la última fila:
> siete sondas que no escapan no son una prueba de resistencia frente a alguien
> que lo intente en serio, y faltan seccomp y el techo de PIDs.
>
> Un runtime no pasa a `ready` sin ejecución real **y** pruebas negativas que
> demuestren que el control bloquea lo que debe bloquear.

---

## 🔗 Ver también

- [Estado del proyecto](PROJECT_STATUS.md) · [Testing](docs/TESTING.md) · [Backlog](docs/IMPLEMENTATION_BACKLOG.md)
