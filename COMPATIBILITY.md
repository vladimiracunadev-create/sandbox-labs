# 🧩 Compatibilidad

Qué funciona dónde, sin promesas de más.

---

## 💻 Sistemas operativos

| Sistema | Catálogo y validadores | Panel `:9093` | `sandboxctl` | Aislamiento real |
|---|:--:|:--:|:--:|---|
| **Linux (x86_64)** | ✅ | ✅ | ✅ | ✅ `bwrap`, `unshare`, WASI |
| **Windows 11 + WSL2** | ✅ | ✅ | ✅ | ✅ dentro de la distro WSL |
| **Windows nativo** | ✅ | ✅ | ⚠️ requiere MSVC Build Tools | ❌ sin namespaces Linux |
| **macOS** | ✅ | ✅ | ✅ | ❌ solo `dry-run` |

> [!NOTE]
> En Windows nativo y macOS el proyecto sigue siendo útil: se planifica, se
> valida y se genera evidencia. Lo que no existe es la frontera de aislamiento.

---

## ⚙️ Runtimes

| Runtime | Estado | Requiere | Qué aplica hoy |
|---|---|---|---|
| `dry-run` | 🟢 ready | nada | Plan y evidencia sin ejecutar |
| `native` | 🟡 experimental | opt-in explícito | Timeout y límite de salida. **No es aislamiento** |
| `bwrap` | 🟡 experimental | Linux, `bubblewrap` | Filesystem, namespaces, red cerrada, capabilities, `prlimit` |
| `unshare` | 🟡 experimental | Linux, `util-linux` | Namespaces y red cerrada. Sin jail completo de filesystem |
| `wasi` | 🟡 experimental | `wasmtime` | Preopens y ejecución de módulos WASI registrados |
| `gvisor` | ⚪ documented | Linux, `runsc`, bundle OCI | Contrato y backlog. No ejecuta |
| `kata` | ⚪ manual | Linux, containerd, Kata | Contrato y backlog. No ejecuta |
| `firecracker` | ⚪ manual | KVM, jailer, kernel, rootfs | Requiere integración específica del host |

Los estados `documented` y `manual` **no ejecutan nunca**: el plan los bloquea
con motivo explícito. Ver
[docs/CONTROL_ENFORCEMENT_MATRIX.md](docs/CONTROL_ENFORCEMENT_MATRIX.md).

---

## 🧱 Versiones de herramientas

| Herramienta | Mínimo | Probado con | Dónde se fija |
|---|---|---|---|
| Node.js | 22 | 22 y 24 | `package.json` → `engines` |
| pnpm | 9 | 9.15.0 | `package.json` → `packageManager` |
| Rust | 1.78 | stable | `Cargo.toml` → `rust-version` |
| Python | 3.10 | 3.12 | Solo para las cargas y `tools/preflight.py` |

El proyecto Node no tiene dependencias externas y el workspace Rust solo usa
crates de uso general (`serde`, `clap`, `anyhow`, `sha2`, `chrono`, `walkdir`,
`uuid`, `tempfile`, `wait-timeout`, `thiserror`).

---

## 🌐 Navegadores

El panel usa `<dialog>`, `EventSource` y módulos ES nativos, sin bundler ni
polyfills. Funciona en versiones actuales de Chrome, Edge, Firefox y Safari.

Se adapta a esquema claro y oscuro (`prefers-color-scheme`) y respeta
`prefers-reduced-motion`.

---

## 🔐 Superficie de red

| Aspecto | Valor |
|---|---|
| Interfaz de escucha | `127.0.0.1` — nunca `0.0.0.0` |
| Puerto | `9093` (`controlCenterPort` en `sandbox.config.json`) |
| Autenticación | Ninguna: la frontera es el bind local |
| Comandos arbitrarios | **No existen**: no hay endpoint que los acepte |
| Anti DNS-rebinding | Validación de `Host` en cada petición |

> [!WARNING]
> No expongas el panel fuera de `localhost`. No está diseñado para ser
> multiusuario ni para atravesar una red.

---

## 🔗 Ver también

- [Preparación del entorno](ENVIRONMENT_SETUP.md)
- [Modelo de amenazas](docs/THREAT_MODEL.md)
- [Windows y WSL2](docs/WINDOWS_WSL2.md)
