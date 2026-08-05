# 🧰 Preparación del entorno

De un equipo en blanco a la primera evidencia reproducible. Si algo falla a
mitad de camino, salta a [docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md).

---

## 0️⃣ Qué necesitas realmente

| Objetivo | Qué hace falta |
|---|---|
| Leer el catálogo y validar contratos | **Solo Node.js 22+** |
| Usar el panel y planificar con `dry-run` | Node.js 22+ (el CLI es opcional) |
| Compilar `sandboxctl` y ejecutar de verdad | **Rust 1.78+** |
| Aislamiento real (`bwrap`, `unshare`) | **Linux o WSL2** |
| WASI | `wasmtime` |
| gVisor / Kata / Firecracker | Host dedicado — ver [COMPATIBILITY.md](COMPATIBILITY.md) |

> [!IMPORTANT]
> Nada de esto convierte el equipo en un entorno seguro para código hostil.
> Para cargas desconocidas, usa una VM que puedas destruir.

---

## 1️⃣ Node.js y pnpm

```bash
node --version      # >= 22
corepack enable     # pnpm lo fija package.json (packageManager)
pnpm install --frozen-lockfile
```

El proyecto Node **no tiene dependencias externas**: `pnpm install` solo
enlaza el workspace. Por eso todos los validadores funcionan con `node` a secas.

## 2️⃣ Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustc --version     # >= 1.78
```

`rust-toolchain.toml` fija el canal `stable` con `rustfmt` y `clippy`, así que
`rustup` los instala solo al entrar al repositorio.

```bash
cargo build --workspace --locked
```

> [!TIP]
> En **Windows nativo** el objetivo MSVC necesita las *C++ Build Tools* de
> Visual Studio. Si no las tienes, compila desde **WSL2** — que además es donde
> los adaptadores de aislamiento funcionan de verdad.

## 3️⃣ Runtimes de aislamiento (Linux / WSL2)

```bash
sudo apt-get update
sudo apt-get install -y bubblewrap util-linux    # bwrap + unshare + prlimit
```

Wasmtime, para el runtime WASI:

```bash
curl https://wasmtime.dev/install.sh -sSf | bash
```

## 4️⃣ Comprobar el host

```bash
cargo run -p sandboxctl -- doctor    # sondeo desde el CLI
bash scripts/doctor.sh               # sondeo desde bash
python tools/preflight.py            # resumen portable
```

Cada runtime aparece como ✅ disponible o ⚪ ausente. Un ⚪ no es un error: el
plan lo tratará como control no soportado y la política decidirá qué hacer.

---

## 5️⃣ Verificar la instalación

```bash
make check          # validadores + suite del Control Center
cargo test --workspace --locked
```

Salida esperada: todos los validadores en ✅ y las pruebas en verde.

## 6️⃣ Primera evidencia

```bash
cargo run -p sandboxctl -- run \
  --workload workloads/benign/hello \
  --runtime dry-run \
  --policy policies/minimal.json
```

Queda un JSON en `evidence/runs/`. Compruébalo:

```bash
node scripts/validate-evidence.mjs
```

## 7️⃣ Levantar el panel

```bash
pnpm dashboard:build
pnpm dashboard:start
```

Abre <http://127.0.0.1:9093>. En Windows también sirve
`launcher/windows/start-sandbox-labs.cmd`.

---

## 🧹 Dejarlo limpio

```bash
pnpm test:cleanup   # borra .sandbox-data/ y las evidencias locales
make clean          # además borra target/
```

---

## 🔗 Ver también

- [Primeros pasos](docs/GETTING_STARTED.md)
- [Compatibilidad por sistema](COMPATIBILITY.md)
- [Windows y WSL2](docs/WINDOWS_WSL2.md)
- [Modos de operación](OPERATING-MODES.md)
