# ❓ Preguntas frecuentes

Para operar el día a día mira el [RUNBOOK.md](RUNBOOK.md); para instalar,
[ENVIRONMENT_SETUP.md](ENVIRONMENT_SETUP.md).

---

## 🚪 ¿Cuál es la entrada principal del repo?

Depende de qué quieras:

| Quieres | Entra por |
|---|---|
| Ver el sistema funcionando | El panel: `pnpm dashboard:start` → <http://127.0.0.1:9093> |
| Automatizar o guionizar | El CLI: `cargo run -p sandboxctl -- --help` |
| Aprender los conceptos | [`labs/`](labs/) — 18 recorridos, de menor a mayor |

---

## 🛡️ ¿Esto es seguro para ejecutar código malicioso?

**No.** Y el proyecto no lo afirma en ningún sitio.

`experimental` significa que el adaptador existe y aplica *algunos* controles,
no que resista a un atacante. Para código desconocido usa una VM dedicada que
puedas destruir, y valida antes qué controles quedan efectivos en tu host.

La evidencia de cada ejecución dice exactamente qué se aplicó y qué no. Ese es
el producto: **saber** cuál es tu frontera, no suponerla.

---

## 🤔 ¿Por qué no puedo ejecutar un comando cualquiera?

Porque un panel local que acepta comandos arbitrarios es una shell remota con
otro nombre. La API solo admite identificadores del catálogo: carga, política y
runtime. Cualquier otra cosa devuelve `400`.

Si necesitas ejecutar algo nuevo, **regístralo como carga**: crea el directorio
con su `manifest.json` en `workloads/`. Así queda versionado, hasheado y
auditable.

---

## 🧊 ¿Qué hace realmente `dry-run`?

Compila el plan completo —controles efectivos, no soportados, límites— y
escribe la evidencia **sin ejecutar nada**. Es la forma correcta de entender
qué haría una combinación antes de arriesgarse.

---

## 🚫 Mi trabajo termina en `blocked`. ¿Está roto?

Casi seguro que no: es el comportamiento buscado.

1. Abre la evidencia y mira `policy.unsupportedControls`.
2. Ese es el control que tu política exige y el runtime no aplica en tu host.

Opciones, en orden de preferencia:

1. Usar un runtime que sí lo aplique (`bwrap` cubre más que `unshare`).
2. Instalar lo que falte (`prlimit` habilita `memory` y `processes` en `bwrap`).
3. Elegir una política acorde al riesgo real de la carga.

> [!WARNING]
> Cambiar la política a `best-effort` para «arreglar» el bloqueo no arregla
> nada: solo apaga el aviso de que no tienes ese control.

---

## ⏳ Mi trabajo se queda en `planned` y esperaba una ejecución

El panel no encontró `sandboxctl` compilado y generó una evidencia de reserva
en lugar de fingir un resultado. Compílalo:

```bash
cargo build -p sandboxctl --release --locked
```

O apunta a un binario existente con `SANDBOXCTL_BIN=/ruta/a/sandboxctl`.

---

## 🖥️ ¿Funciona en Windows?

El catálogo, los validadores y el panel, sí. El **aislamiento real, no**:
`bwrap` y `unshare` son de Linux. Usa **WSL2** —donde todo funciona— y consulta
[docs/WINDOWS_WSL2.md](docs/WINDOWS_WSL2.md).

Compilar en Windows nativo requiere las *C++ Build Tools* de Visual Studio para
el objetivo MSVC. Desde WSL2 no hace falta nada de eso.

---

## 🔓 ¿Qué es `SANDBOX_LABS_ALLOW_NATIVE` y por qué existe?

`native` ejecuta la carga **en tu host, sin aislamiento**: solo aplica timeout y
límite de salida. Existe como línea base para comparar —el laboratorio 01 trata
justamente de eso— y por eso lleva dos cerrojos:

1. La variable `SANDBOX_LABS_ALLOW_NATIVE=1`.
2. `allowNative: true` en el manifiesto de la carga.

Ninguna carga de riesgo puede declarar el segundo, y hay una prueba que lo
verifica en cada commit.

---

## 🧾 ¿Para qué sirven los hashes de la evidencia?

Para que un resultado sea reproducible y auditable. La evidencia guarda el
SHA-256 de la política, del contenido de la carga y del binario que la ejecutó.
Si mañana el resultado cambia, los hashes dicen qué cambió.

---

## 📦 ¿Por qué `control-center/dist/` está versionado?

Para que el panel arranque en un host sin toolchain de TypeScript. El «build»
es una copia que reescribe imports (`scripts/build.mjs`), no una compilación:
el código fuente ya es JavaScript válido con tipos. Regenéralo con
`pnpm dashboard:build`.

---

## 🐳 ¿Por qué no usar Docker y ya?

Docker es una de las fronteras posibles, no *la* frontera. El objetivo del
repositorio es exactamente comparar namespaces, contenedores, WASI y microVMs
midiendo qué aplica cada uno — y un `docker run` no te dice qué controles
quedaron efectivos en tu kernel.

Si quieres el recorrido de Docker, está en
[docker-labs](https://github.com/vladimiracunadev-create/docker-labs).

---

## 🧪 ¿Por dónde empiezo con los laboratorios?

```text
01-baseline-unrestricted → 04-linux-namespaces → 05-cgroups-limits
   → 10-rootless-sandbox → 14-wasm-wasi → 15-ai-code-runner
      → 16-escape-test-suite
```

El 01 no es relleno: sin ver una ejecución sin restricciones no se aprecia qué
quita cada control después.

---

## 🤝 ¿Puedo contribuir?

Sí. Lee [CONTRIBUTING.md](CONTRIBUTING.md). La única regla dura:

> Ningún runtime pasa a `ready` sin evidencia y pruebas negativas que lo
> respalden.

---

## 🔗 Ver también

- [Glosario](GLOSSARY.md) · [Runbook](RUNBOOK.md) · [Soporte](SUPPORT.md)
- [Solución de problemas](docs/TROUBLESHOOTING.md)
