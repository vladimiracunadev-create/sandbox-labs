# 📊 Estado del proyecto

Este documento responde a una sola pregunta: **qué está construido de verdad, qué
lo demuestra y qué falta.**

La regla que lo gobierna es la del proyecto entero: *no se indica que algo fue
probado si no se ejecutó la prueba, y no se ocultan los errores.* Por eso aquí no
aparece la palabra «listo» en ningún sitio, y cada afirmación de que algo
funciona viene con el comando que hay que ejecutar para comprobarlo.

**Versión:** 0.1.0 · **Última revisión:** 2026-08-07

---

## Resumen en tres números

| | Construido | Del total |
|---|:--:|:--:|
| **Núcleo de aislamiento** | 9 de 9 controles | ✅ completo y verificado en CI |
| **Casos técnicos** | 4 | de 15 |
| **Casos de mercado de capitales** | 2 | de 21 |

El núcleo está terminado. Los casos, no. Ese es el estado honesto: **la
plataforma que aplica y verifica controles funciona; el catálogo de casos que se
apoya en ella está al principio.**

---

## Lo que se puede ejecutar hoy y comprobar

Tres comandos que corren **en cada commit**, no en un documento:

```bash
cargo run -p sandboxctl -- escape
```

Ocho sondas intentan escaparse del sandbox: red, PIDs, memoria, sistema de
ficheros, capacidades, dispositivos, entorno y llamadas al sistema. La suite
declara `bwrap [experimental] sin fugas (8 contenidas)` y, si alguna se escapa,
CI se pone rojo. Hay además una línea base con `native` que **debe** escaparse: si
no lo hace, la suite no está midiendo nada.

```bash
cargo run -p sandboxctl -- evidence verify
```

Verifica que cada acta de ejecución se sostiene: la huella SHA-256 coincide con
el documento, la firma Ed25519 es válida, la cadena con la evidencia anterior no
está rota, y los hashes de la política y de la carga corresponden a lo que hay en
el repositorio.

```bash
cargo run -p sandboxctl -- markets reconcile
```

Seis escenarios de custodia, cada uno con el hallazgo que **debe** producir. Si la
conciliación deja de detectar un descuadre que declara detectar, CI se pone rojo.

```bash
node scripts/verify-cases.mjs
```

Prueba de comportamiento de los casos técnicos. Hoy cubre el caso 01 con diez
comprobaciones. Un caso en estado `ready` **sin prueba aquí hace fallar la
suite**: es el guardián de que el estado declarado sea cierto.

---

## Núcleo de aislamiento — completo

Los nueve puntos del backlog técnico están cerrados, cada uno verificado
empíricamente en WSL2 con bubblewrap 0.9.0 real y confirmado en CI.

| # | Qué se cerró | Cómo se aplica de verdad |
|:--:|---|---|
| B-01 | **Límite de procesos** | `pids.max` vía cgroups v2, pedido con `systemd-run --user --scope` |
| B-02 | **Límite de memoria** | `memory.max`, con observación de `memory.peak` y `oom_kill` |
| B-03 | **Límite de CPU** | `cpu.max`, con lectura de `cpu.stat` durante la ejecución |
| B-04 | **Red por lista de permitidos** | Namespace de red propio + proxy por socket Unix que habla `CONNECT host:port` y registra **cada intento**, permitido o no |
| B-04b | **Puerto publicado con red contenida** | El servicio escucha en un socket Unix dentro de la jaula; un reenviador fuera publica el puerto TCP |
| B-05 | **Llamadas al sistema** | Filtro seccomp BPF compilado con `seccompiler` y entregado a bubblewrap por descriptor de fichero |
| B-06 | **UID y GID** | `--uid` / `--gid` en un namespace de usuario, sin privilegios |
| B-07 | **Compilador único de argumentos** | Un solo sitio traduce política → argumentos de bubblewrap. Antes había varios y divergían |
| B-08 | **Integridad de la evidencia** | Huella → firma Ed25519 → cadena entre actas → rehash de política y carga |

### La regla que lo gobierna todo

> **Un control solicitado, un control aplicado y un control reportado tienen que
> describir la misma realidad.**

Por eso toda acta distingue cinco listas: `requestedControls`,
`effectiveControls`, `unsupportedControls`, `failedControls` y
`observedControls`. Y por eso, si una política estricta pide un control
obligatorio que este equipo no puede aplicar, **la ejecución no ocurre**: falla
cerrada y explica qué falta. El detalle está en
[Referencia de políticas](POLICY_REFERENCE.md) y en
[Formato de evidencia](EVIDENCE_FORMAT.md).

---

## Casos técnicos — 4 de 15

| # | Caso | Estado | Qué hay | Qué falta |
|:--:|---|:--:|---|---|
| 01 | [Contenido web no confiable](casos/01-contenido-web-no-confiable.md) | 🟡 `building` | Coordinador e intérprete separados por proceso, 15 tipos de rechazo, **10 comprobaciones automáticas** | Ficha en el panel; levantar el servicio bajo `bwrap` dentro de CI |
| 02 | [Código generado por IA](casos/02-codigo-generado-por-ia.md) | 🟡 `building` | Ejecución sin red, entorno vacío, disco temporal, techo de tiempo | **Rediseño**: un sandbox efímero por ejecución, con cola y cancelación. Más lenguajes |
| 03 | [Procesamiento de archivos](casos/03-procesamiento-seguro-de-archivos.md) | 🟡 `building` | zip slip, zip bomb, rutas absolutas, enlaces, informe por entrada | Renombrar a `03-safe-archive-processing`; MIME real, más formatos, checksum |
| 04 | [Plugins de terceros](casos/04-plugins-de-terceros.md) | 🔴 `planned` | — | Todo: manifiesto, concesión, seis plugins de ejemplo |
| 05 | [Custodia de claves y firma](casos/05-custodia-de-claves-y-firma.md) | 🟡 `building` | Firma Ed25519 en la jaula, red `none`, socket Unix, clave fuera del repositorio | Dividir: el determinismo se va al caso 07. Límites de monto, rotación, revocación |
| 06–15 | [Los diez restantes](casos/README.md#-familia-técnica--15-casos) | 🔴 `planned` | — | Especificados con ficha completa; sin código |

**Ninguno está en `ready`.** Ninguno tiene todavía evidencia firmada por
ejecución, que es el requisito para `verified`.

---

## Mercado de capitales — 2 de 21

> [!WARNING]
> **Sin dinero real, sin valores reales, sin credenciales reales y sin
> conectividad de producción.** El simulador **no es una autorización
> regulatoria** de la CMF ni de ninguna otra autoridad, y nada de lo que produzca
> es una recomendación de inversión.

### La base: construida y probada

El crate [`sandbox-markets`](../crates/sandbox-markets) tiene las dos piezas de
las que dependen todos los casos financieros:

- **`Money`** — enteros en unidades mínimas, con la moneda pegada al importe.
  Pesos y dólares **no se suman porque no compila**. Nunca coma flotante: un
  `f64` no representa 0,10 de forma exacta, y en un libro contable eso es un
  descuadre que aparece tarde y en producción.
- **`Ledger`** — partida doble, append-only, con reversas e idempotencia. Una
  transacción que no cuadra no se registra.

### Los casos

| # | Caso | Estado | Qué hay | Qué falta |
|:--:|---|:--:|---|---|
| CM-02 | [Sistema alternativo de transacción](casos/cm-02-sistema-alternativo-de-transaccion.md) | 🟠 `prototype` | Libro de órdenes con **11 invariantes**: prioridad precio-tiempo, precio fijado por la orden que descansa, libro nunca cruzado | Los 7 escenarios; órdenes de mercado; **reconstrucción completa de la sesión** |
| CM-03 | [Custodia y segregación de activos](casos/cm-03-custodia-y-segregacion-de-activos.md) | 🟢 `functional` | El invariante de custodia, 5 tipos de hallazgo, **6 escenarios verificados en cada commit** | Dividendos, bloqueos, garantías, insolvencia del custodio |
| CM-00, CM-01, CM-04–CM-20 | [Los diecinueve restantes](casos/README.md#-familia-mercado-de-capitales--21-casos) | 🔴 `planned` | — | Especificados con ficha completa; sin código |

---

## Lo que está construido y no se ve

Piezas que no son un caso pero sostienen todo lo demás:

| Pieza | Dónde | Qué hace |
|---|---|---|
| Compilador de políticas | `crates/sandbox-core/src/compiler.rs` | El **único** sitio que traduce una política a argumentos de bubblewrap |
| cgroups v2 | `crates/sandbox-core/src/cgroup.rs` | Pide límites por `systemd-run` y **observa** lo que realmente se consumió |
| seccomp | `crates/sandbox-core/src/seccomp.rs` | Compila el filtro BPF y lo entrega por descriptor de fichero |
| Proxy de salida | `crates/sandbox-core/src/egress.rs` | Lista de permitidos sin comodines, con registro de cada conexión |
| Firma de evidencia | `crates/sandbox-core/src/signing.rs` | Ed25519, clave local con permisos `0600`, fuera del control de versiones |
| Reenviador de puertos | `crates/sandboxctl/src/forward.rs` | Puente TCP ↔ socket Unix, para publicar sin dar red a la jaula |
| Barrido de huérfanos | `crates/sandboxctl/src/service.rs` | `service down --all` mata también lo que quedó vivo sin registro |

Esa última línea existe por un incidente real de este proyecto: tres sandboxes
sobrevivieron cuatro horas sin que nada pudiera encontrarlos, porque se había
quitado `--die-with-parent` y un script de limpieza había borrado sus registros.
Está contado en [RUNBOOK](../RUNBOOK.md).

---

## Lo que se decidió no hacer, y por qué

| Decisión | Motivo |
|---|---|
| No usar `npm`, sino **pnpm** | Ficheros de bloqueo deterministas y una única forma de instalar |
| No usar `libseccomp` (C), sino `seccompiler` (Rust puro) | Evitar una dependencia nativa que rompe la construcción reproducible |
| No escribir cgroups directamente | En WSL2 `/init.scope` no es escribible; `systemd-run --user --scope` funciona en ambos |
| Vaciar el entorno con `env -i` en vez de quitar variables una a una | systemd inyecta variables propias; quitarlas de una en una siempre deja alguna |
| No incluir malware real, ni siquiera para el caso 06 | Regla del proyecto, sin excepciones |
| No conectar con ningún sistema financiero real | Regla de la familia de mercado de capitales, sin excepciones |

---

## Los estados y lo que significan

| Estado | Qué significa exactamente |
|---|---|
| `planned` | Especificado, **sin código** |
| `prototype` | Código que corre, sin prueba de comportamiento |
| `functional` | Se ejecuta y **hay una prueba concreta** que lo demuestra |
| `verified` | Además emite evidencia firmada y CI la valida |
| `production-research` | Investigación, no desplegable |
| `deprecated` | Se conserva por compatibilidad |

---

## Qué sigue

El orden no es arbitrario: cada elemento desbloquea al siguiente.

1. **Caso 01 a `functional`** — levantarlo bajo `bwrap` en CI y añadir su ficha al
   panel.
2. **Caso 04** — plugins por capacidades. Se apoya en el proxy de salida, que ya
   existe.
3. **CM-02 a `functional`** — reconstrucción completa de la sesión, que es
   requisito previo de CM-09.
4. **Caso 02, el rediseño** — sandbox efímero por ejecución.
5. **CM-00** — la puerta de la familia financiera: sin ella, los demás casos no
   tienen límites que respetar.

---

**Ver también:** [Catálogo completo](CATALOGO.md) · [Fichas de los casos](casos/README.md) · [Backlog de implementación](IMPLEMENTATION_BACKLOG.md) · [Runbook](../RUNBOOK.md)
