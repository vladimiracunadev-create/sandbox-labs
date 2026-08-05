# 🔧 Solución de problemas

Síntoma → causa → arreglo. Para operación rutinaria, el
[RUNBOOK](../RUNBOOK.md); para conceptos, la [FAQ](../FAQ.md).

---

## 🧭 Diagnóstico rápido

```bash
cargo run -p sandboxctl -- doctor    # qué runtimes ve el host
node scripts/validate-config.mjs      # el catálogo es coherente
make check                            # suite completa
```

---

## 🖥️ El panel

### El panel no arranca

```bash
node scripts/validate-config.mjs      # ¿el catálogo es válido?
pnpm dashboard:build                  # regenera dist/
node control-center/dist/server.js    # arranca en primer plano y muestra el error
```

### `EADDRINUSE: 9093`

Ya hay algo escuchando en ese puerto.

```bash
# Linux / WSL
ss -ltnp | grep 9093
# Windows
netstat -ano | findstr 9093
```

Cierra el proceso o cambia `project.controlCenterPort` en
`sandbox.config.json`.

### El navegador responde `421 untrusted_host`

Estás entrando por un nombre que el panel no reconoce. Usa
<http://127.0.0.1:9093> o <http://localhost:9093>. Es la defensa anti
DNS-rebinding: no la desactives con un proxy inverso.

### El formulario devuelve `403 untrusted_request`

Las escrituras exigen la cabecera `x-sandbox-request: 1`. La UI la envía sola;
si llamas por `curl`, añádela:

```bash
curl -X POST http://127.0.0.1:9093/api/jobs \
  -H 'content-type: application/json' \
  -H 'x-sandbox-request: 1' \
  -d '{"workloadId":"hello","policyId":"minimal","runtimeId":"dry-run","arguments":[]}'
```

### La lista de trabajos no se actualiza sola

El estado llega por SSE. Si un proxy o una extensión corta `text/event-stream`,
el panel cae al sondeo lento (10 s) y sigue funcionando. Revisa la consola del
navegador si sospechas del stream.

---

## 🦀 El CLI

### `linker 'link.exe' not found` (Windows)

El objetivo MSVC necesita las *C++ Build Tools* de Visual Studio. La ruta más
corta es compilar desde **WSL2**, que además es donde el aislamiento funciona.

### `dlltool could not create import library` (Windows, toolchain GNU)

El toolchain `x86_64-pc-windows-gnu` de rustup viene sin binutils completos.
Mismo consejo: compila en WSL2.

### `error: the lock file needs to be updated`

`Cargo.lock` está versionado y CI compila con `--locked`. Si cambiaste
dependencias:

```bash
cargo generate-lockfile
git add Cargo.lock
```

### `No se pudo resolver <ruta>` / `Ruta fuera del repositorio`

El CLI solo acepta rutas dentro del repositorio, resueltas de forma canónica.
Pásalas relativas a la raíz o usa `--root`.

---

## 🚫 Trabajos que no ejecutan

### Estado `blocked`

Comportamiento previsto: la política es `strict` y falta un control.

```bash
jq '.policy.unsupportedControls, .result.reason' evidence/runs/<runId>.json
```

| Control faltante | Arreglo habitual |
|---|---|
| `memory`, `processes` | Instala `util-linux` para tener `prlimit` |
| `filesystem`, `capabilities` | Usa `bwrap` en vez de `unshare` |
| `syscalls`, `cpu` | Ningún runtime local los aplica todavía |

### Estado `planned` cuando esperabas ejecución

El panel no encontró `sandboxctl`:

```bash
cargo build -p sandboxctl --release --locked
# o
export SANDBOXCTL_BIN=/ruta/a/sandboxctl
```

### `native requiere SANDBOX_LABS_ALLOW_NATIVE=1 y allowNative=true`

Los dos cerrojos de `native`. Y solo para cargas benignas:

```bash
SANDBOX_LABS_ALLOW_NATIVE=1 cargo run -p sandboxctl -- run \
  --workload workloads/benign/hello --runtime native --policy policies/web-application.json
```

### `runtime no disponible: No such file or directory`

Falta el binario del runtime en el host.

```bash
sudo apt-get install -y bubblewrap util-linux    # bwrap, unshare, prlimit
curl https://wasmtime.dev/install.sh -sSf | bash # wasmtime
```

### Estado `timeout`

El trabajo superó `resources.timeoutSeconds` de la política. Sube el límite en
la política o revisa por qué la carga tarda más de lo previsto — un timeout no
es un fallo del sistema, es un control funcionando.

---

## 🧪 Pruebas y validadores

### `Enlace roto en <archivo>`

`check-doc-links.mjs` encontró un enlace relativo a un archivo inexistente.
Lista **todos** los rotos de una vez: arréglalos y vuelve a ejecutarlo.

### La prueba de symlink aparece como `skipped`

Windows no permite crear symlinks sin privilegios ni modo desarrollador. La
prueba se salta a propósito: en Linux y en CI sí corre.

### `cargo fmt --all -- --check` falla en CI y en local no

Ejecuta `cargo fmt --all` y commitea el resultado. `rustfmt.toml` fija el
estilo, así que el formato no es opinable.

---

## 🪟 Windows y WSL2

### `bwrap` y `unshare` no aparecen en `doctor`

Son de Linux. Ejecuta el proyecto dentro de la distro WSL, no desde PowerShell.

### Los scripts `.sh` fallan con `bad interpreter`

Fin de línea CRLF. `.gitattributes` fija LF para todo lo ejecutable; si el
archivo viene de fuera:

```bash
sed -i 's/\r$//' script.sh
```

### El rendimiento en `/mnt/c` es malo

El cruce de sistema de archivos entre Windows y WSL es lento. Clona el
repositorio dentro del sistema de archivos de la distro (`~/`) para trabajar.

---

## 🧹 Volver a un estado limpio

```bash
pnpm test:cleanup   # borra .sandbox-data/ y las evidencias locales
make clean          # además borra target/
```

---

## 🔗 Ver también

- [FAQ](../FAQ.md) · [Runbook](../RUNBOOK.md) · [Soporte](../SUPPORT.md)
- [Windows y WSL2](WINDOWS_WSL2.md)
