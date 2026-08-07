# 🔧 Cuando algo falla

Los fallos de este proyecto no son aleatorios: casi todos vienen de que **el
equipo no puede aplicar un control que la política pide**. Y eso es información,
no una avería.

Este documento reúne los fallos que afectan a **cualquier caso**. Cada ficha de
caso tiene además su propia sección «Si algo falla» con lo específico suyo.

> **La regla que explica casi todo lo de aquí.** Si una política estricta pide un
> control obligatorio que este equipo no puede aplicar, **la ejecución no
> ocurre**: falla cerrada y dice qué falta. Eso *es* el comportamiento correcto.
> La alternativa —ejecutar con menos controles de los pedidos— es exactamente
> cómo se construyen sistemas que parecen seguros.

---

## Lo primero, siempre

```bash
cargo run -p sandboxctl -- doctor
```

Enumera qué runtimes hay en **tu** equipo, qué controles puede aplicar cada uno
aquí, y **cuáles se pedirán pero no se podrán aplicar**. Nueve de cada diez
fallos se explican con esa salida.

---

## Índice

- [No se puede crear el sandbox](#no-se-puede-crear-el-sandbox)
- [No hay límites de memoria, CPU o procesos](#no-hay-límites-de-memoria-cpu-o-procesos)
- [Un puerto está ocupado](#un-puerto-está-ocupado)
- [Un servicio se levanta y se muere solo](#un-servicio-se-levanta-y-se-muere-solo)
- [Quedaron procesos vivos que no aparecen en la lista](#quedaron-procesos-vivos-que-no-aparecen-en-la-lista)
- [Errores de compilación](#errores-de-compilación)
- [Problemas del sistema de ficheros en WSL2](#problemas-del-sistema-de-ficheros-en-wsl2)
- [La verificación de evidencia falla](#la-verificación-de-evidencia-falla)
- [Los diagramas del sitio no se ven](#los-diagramas-del-sitio-no-se-ven)
- [Funciona en local y falla en CI](#funciona-en-local-y-falla-en-ci)
- [Cuando nada de lo anterior sirve](#cuando-nada-de-lo-anterior-sirve)

---

## No se puede crear el sandbox

```text
bwrap: No permissions to creating new namespace
bwrap: setting up uid map: Permission denied
```

**Qué pasa.** El sistema no permite crear *namespaces* de usuario sin
privilegios. Es la base de todo el aislamiento sin root, así que sin esto no hay
jaula.

**Alternativas, de la mejor a la peor:**

| Alternativa | Cómo | Qué pierdes |
|---|---|---|
| **1. Habilitarlos** (recomendado) | En Debian/Ubuntu antiguo: `sudo sysctl -w kernel.unprivileged_userns_clone=1`. En Ubuntu 23.10+ además: `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0` | Nada. Es la vía prevista |
| **2. Usar el runtime `unshare`** | `--runtime unshare` | Es **más débil**: no aísla el sistema de ficheros. El proyecto lo declara así, no lo disimula |
| **3. Planificar sin ejecutar** | `--runtime dry-run` | No se ejecuta nada: se genera el plan y la evidencia de lo que *habría* ocurrido |
| **4. Una máquina virtual desechable** | Cualquier VM Linux | Nada, salvo comodidad. Es lo correcto para cargas desconocidas de verdad |

**Lo que no debes hacer:** ejecutar el proyecto como root para saltarte el
límite. Un sandbox creado por root es un sandbox del que se sale siendo root.

---

## No hay límites de memoria, CPU o procesos

```text
systemd-run: Failed to connect to bus
Los controles memory, cpu y processes no aparecen en effectiveControls
```

**Qué pasa.** Los tres límites se piden con `systemd-run --user --scope`, que
necesita systemd en modo usuario y cgroups v2. En WSL2 sin systemd, o en
contenedores, no está.

> **Por qué no se escriben los cgroups directamente:** en WSL2 `/init.scope` no
> es escribible. Pasar por `systemd-run` funciona en los dos entornos.

**Alternativas:**

| Alternativa | Cómo | Qué pierdes |
|---|---|---|
| **1. Activar systemd en WSL2** | En `/etc/wsl.conf`: `[boot]` y `systemd=true`. Después `wsl --shutdown` desde Windows | Nada |
| **2. Aceptar que no hay límites** | Nada: el proyecto **no los declara** en la evidencia | El techo de memoria y CPU. Sigue habiendo aislamiento de red, ficheros, entorno y capacidades |
| **3. Usar una política no estricta** | Quitar el modo `strict` de la política | La garantía de que se aplica lo pedido. Úsalo solo para probar |

**Cuidado con el caso 11 y el caso 12.** Ahí `memory.max` no es un lujo: sin
techo de memoria, una imagen de dimensiones absurdas o un notebook mal escrito se
llevan el equipo por delante. Con política estricta, esos casos **deben**
negarse a ejecutar si no hay cgroups, y eso es lo correcto.

---

## Un puerto está ocupado

```text
Address already in use (os error 98)
```

**Qué pasa.** Casi siempre es un reenviador de puertos de una sesión anterior que
sigue vivo, no otro programa.

```bash
cargo run -p sandboxctl -- service down --all
```

Ese comando baja también **los huérfanos**: procesos que quedaron vivos sin
registro. Si aun así sigue ocupado, mira quién lo tiene:

```bash
ss -ltnp | grep 880
```

---

## Un servicio se levanta y se muere solo

**Qué pasa.** Lo más común es una incoherencia entre el transporte que declara el
servicio y la red que le concede la política: un servicio con `transport: tcp`
dentro de una jaula con `network: none` **no tiene dónde escuchar**.

El proyecto lo comprueba antes de arrancar y lo dice, en vez de dejarlo morir en
silencio.

**Alternativas:**

| Situación | Qué hacer |
|---|---|
| El servicio necesita publicar un puerto y quieres que **no tenga red** | `transport: unix-socket` y `publish: proxy`. El servicio escucha en un socket Unix dentro de la jaula, y un reenviador de fuera publica el puerto. Es lo que hacen los casos 01, 02, 03 y 05 |
| El servicio necesita red de verdad | Política con `network: unrestricted` — y que quede escrito, porque deja de estar contenido en esa dimensión |
| Necesita salir solo a unos destinos | `network: allowlist`. Se le da un namespace propio y un proxy con lista, que además **registra cada intento** |

---

## Quedaron procesos vivos que no aparecen en la lista

Pasó de verdad en este proyecto: **tres sandboxes sobrevivieron cuatro horas** sin
que nada pudiera encontrarlos, porque se había quitado `--die-with-parent` y un
script de limpieza había borrado sus registros.

```bash
cargo run -p sandboxctl -- service down --all
```

El barrido busca **dos clases** de huérfano: la jaula (`bwrap` con
`/workspace/app`) y el reenviador (` service ` y ` forward ` en su línea de
comandos). Si sospechas que queda algo:

```bash
ps -eo pid,args | grep -E '[b]wrap|[s]andboxctl' 
```

**Y no borres `.sandbox-data` a mano** mientras haya servicios levantados: ahí
viven los registros que permiten encontrarlos. `scripts/cleanup-test-state.mjs`
se niega a hacerlo por ese motivo.

---

## Errores de compilación

### En Windows: `dlltool.exe: CreateProcess`

La cadena de compilación de GNU incompleta. **Este proyecto se compila en WSL2**,
no en Windows nativo:

```bash
wsl
cargo build --release
```

### `error: the lock file needs to be updated`

`Cargo.lock` está versionado a propósito: es lo que hace reproducible la
construcción. No lo borres.

```bash
cargo metadata --locked --format-version 1 > /dev/null
```

### `pnpm: command not found`

```bash
corepack enable pnpm
```

**No uses `npm`.** El proyecto usa pnpm y conserva su fichero de bloqueo; mezclar
gestores produce árboles de dependencias distintos entre tu equipo y CI.

---

## Problemas del sistema de ficheros en WSL2

Trabajando sobre `/mnt/c` puede aparecer algo desconcertante: `mkdir` responde
«File exists» y `ls` dice que no existe. Es una desincronización de la caché de
DrvFs, el puente entre Windows y Linux.

**Alternativas:**

| Alternativa | Cómo |
|---|---|
| **1. Reintentar** | El proyecto ya tolera este caso en `dirs.rs`, que trata «existe» como éxito |
| **2. Trabajar desde el sistema de ficheros de Linux** | Clonar en `~/` en vez de en `/mnt/c`. Además es bastante más rápido |
| **3. Reiniciar WSL** | `wsl --shutdown` desde Windows |

---

## La verificación de evidencia falla

```bash
cargo run -p sandboxctl -- evidence verify
```

Cada tipo de fallo significa algo distinto, y solo dos son problemas:

| Lo que dice | Qué significa | ¿Es un problema? |
|---|---|---|
| Huella SHA-256 no coincide | El fichero de evidencia se modificó | **Sí** |
| Firma Ed25519 inválida | Alguien rehízo el documento y recalculó la huella | **Sí** |
| Cadena rota | Falta un informe intermedio | **Sí**, salvo que lo borraras tú |
| Hash de política o de carga distinto | **El código cambió desde aquella ejecución** | **No.** Es un informe viejo diciendo con razón que ya no describe el código de hoy |

Ese último caso es el que más confunde. Vuelve a ejecutar para generar evidencia
del código actual.

> **La firma no es una notarización.** La clave la guarda la misma máquina que
> ejecuta, así que prueba que el informe no cambió tras escribirse, no que la
> ejecución ocurriera.

---

## Los diagramas del sitio no se ven

Los diagramas se dibujan con una biblioteca que se descarga de un CDN. Sin red,
con el CDN caído o con un bloqueador de por medio, **el diagrama se queda como
código legible** en vez de desaparecer. No es un fallo que haya que arreglar: es
la degradación prevista.

---

## Funciona en local y falla en CI

Suele ser una diferencia de configuración del kernel del runner, no del código.
Casos reales de este proyecto:

| Síntoma | Causa | Solución aplicada |
|---|---|---|
| La sonda de seccomp daba resultados distintos | `perf_event_paranoid` cambia el error que devuelve `perf_event_open` entre máquinas | Se cambió a `getcpu`, que siempre funciona: si devuelve `EPERM`, el filtro está puesto |
| Variables del bus se filtraban dentro de la jaula | `--clearenv` limpia el entorno de la carga, no el del proceso que la lanza | Se vacía el entorno entero con `env -i`, no variable a variable |
| Un `git push` no disparaba los workflows | Comportamiento del propio GitHub Actions | `gh workflow run ci.yml --ref main` |

---

## Cuando nada de lo anterior sirve

1. **Lee la evidencia de la ejecución.** Está en `evidence/runs/` y distingue
   `requestedControls`, `effectiveControls`, `unsupportedControls`,
   `failedControls` y `observedControls`. La diferencia entre las dos primeras
   listas suele ser la respuesta entera.
2. **Ejecuta la suite de contención**: `cargo run -p sandboxctl -- escape`. Si
   las sondas fallan, el problema es del entorno y no de tu caso.
3. **Comprueba el catálogo**: `pnpm config:check` valida que políticas, cargas y
   casos están bien declarados.
4. **Abre una incidencia** con la salida de `doctor` y la evidencia de la
   ejecución. Sin esas dos cosas, cualquier diagnóstico es adivinar.

---

**Ver también:** [Estado del proyecto](ESTADO.md) · [Catálogo completo](CATALOGO.md) · [Fichas de los casos](casos/README.md) · [Runbook](../RUNBOOK.md) · [Referencia de políticas](POLICY_REFERENCE.md)
