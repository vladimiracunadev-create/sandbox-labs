# 🧰 Instalación

De un equipo en blanco a tener un sandbox levantado.

---

## Por qué hace falta Linux o WSL2

Un sandbox no es una biblioteca: son **primitivas del kernel de Linux**.
Namespaces, cgroups, capabilities y seccomp no existen en Windows ni en macOS,
así que ahí no hay nada que aislar.

No es una elección de este proyecto. Tus alternativas están en la misma
situación, solo que lo disimulan: Docker Desktop arranca una máquina virtual
Linux por debajo, y WSL lleva el Linux en el nombre.

| Sistema | Catálogo y docs | Panel `:9093` | Sandbox real |
|---|:--:|:--:|---|
| **Linux** | ✅ | ✅ | ✅ completo |
| **Windows + WSL2** | ✅ | ✅ | ✅ dentro de la distro |
| **Windows nativo** | ✅ | ✅ | ❌ no hay namespaces |
| **macOS** | ✅ | ✅ | ❌ solo planificación |

> [!TIP]
> En Windows nativo y macOS el repositorio sigue siendo útil: se lee el
> catálogo, se valida y se planifica. Lo que no existe es la frontera.

---

## 1 · El sistema base

En **Windows**, instala WSL2 y entra en la distro. Todo lo demás se hace dentro:

```powershell
wsl --install
wsl
```

En **Linux** (o ya dentro de WSL):

```bash
sudo apt update
sudo apt install -y bubblewrap util-linux
```

- `bubblewrap` es el runtime que más contiene: jaula de filesystem, namespaces,
  red cerrada y capabilities.
- `util-linux` trae `unshare` y `prlimit`.

> [!NOTE]
> En Ubuntu 24.04 los *user namespaces* sin privilegios están restringidos por
> AppArmor. Si un sandbox falla al arrancar con
> `bwrap: loopback: Failed RTM_NEWADDR`, esa es la causa:
> `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0`.

## 2 · Las herramientas

```bash
# Rust, para el motor
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# Node 22+, para el panel y los validadores
node --version
corepack enable
```

El proyecto **no tiene dependencias npm**: `pnpm install` solo enlaza el
workspace, y todos los validadores funcionan con `node` a secas.

## 3 · Compilar y comprobar

```bash
git clone https://github.com/vladimiracunadev-create/sandbox-labs.git
cd sandbox-labs

cargo build --workspace --locked
node scripts/validate-config.mjs
```

## 4 · Ver qué tiene tu host

```bash
cargo run -p sandboxctl -- doctor
```

Cada runtime sale como ✅ disponible o ⚪ ausente. Un ⚪ no es un error: el plan
lo tratará como control no soportado y la política decidirá qué hacer.

## 5 · Levantar el primer sandbox

```bash
cargo run -p sandboxctl -- cases                       # qué casos hay
cargo run -p sandboxctl -- service up file-detonation  # levantar uno
cargo run -p sandboxctl -- service list                # ver su estado
```

Abre <http://127.0.0.1:8803> y trabaja dentro. Para apagarlo:

```bash
cargo run -p sandboxctl -- service down --all
```

## 6 · El entorno raíz

```bash
pnpm install --frozen-lockfile
pnpm dashboard:build
pnpm dashboard:start
```

Abre <http://127.0.0.1:9093>: desde ahí se levantan y apagan todos los casos con
un clic, con su estado en vivo y la política bajo la que corren.

---

## Comprobar que contiene de verdad

```bash
cargo run -p sandboxctl -- escape
```

Lanza sondas que **intentan escaparse** y devuelve una matriz por runtime. Es lo
mismo que ejecuta la integración continua en cada commit.

---

## Problemas frecuentes

| Síntoma | Causa | Solución |
|---|---|---|
| `linker 'link.exe' not found` | Compilando en Windows nativo | Compila desde WSL2 |
| `bwrap: loopback: Failed RTM_NEWADDR` | AppArmor restringe los user namespaces | Ver la nota del paso 1 |
| `El puerto 88xx ya está ocupado` | Otro proceso o un sandbox anterior | `sandboxctl service down --all` |
| El servicio muere al arrancar | Falta el runtime | `sandboxctl doctor` y revisa el log del caso |
| `create_dir_all` falla con EEXIST | Caché de DrvFs en `/mnt/c` | `wsl --shutdown` y reintenta |
| Rendimiento malo en `/mnt/c` | Cruce de sistemas de archivos | Clona dentro de `~` en la distro |

---

## Dejarlo limpio

```bash
cargo run -p sandboxctl -- service down --all
pnpm test:cleanup      # borra .sandbox-data y las evidencias locales
make clean             # además borra target/
```

---

## Siguiente paso

- [Qué es un sandbox](QUE-ES-UN-SANDBOX.md) · [Los cinco casos](CASOS.md)
- [Runbook](../RUNBOOK.md) — operación diaria
