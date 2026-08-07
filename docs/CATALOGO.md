# 📋 Catálogo completo de casos

Este documento es **la lista entera de casos del proyecto**: los 15 técnicos y
los 21 de mercado de capitales. Para cada uno dice qué resuelve, por qué existe,
qué hace falta para ejecutarlo y **en qué estado está de verdad hoy**.

No hay casos escondidos ni casos implícitos. Si un caso no aparece aquí, no
existe. Si aparece con estado `planned`, **no está construido** y este documento
lo dice en lugar de sugerir lo contrario.

> **Cómo leer el estado.** Los cinco estados están definidos en
> [Estados de madurez](#estados-de-madurez), al final. El resumen: `planned` no
> tiene código, `prototype` tiene código sin prueba de comportamiento,
> `functional` se ejecuta y hay una prueba que lo demuestra, `verified` además
> tiene evidencia firmada y CI que la valida.

**Qué está construido y qué no, en una línea:** 4 casos técnicos de 15 y 2 de
mercado de capitales de 21. El detalle honesto de cada uno, con la prueba que lo
respalda o la ausencia de ella, está en **[Estado del proyecto](ESTADO.md)**.

---

## Índice

- [Requisitos comunes a todos los casos](#requisitos-comunes-a-todos-los-casos)
  - [Software necesario](#software-necesario)
  - [Instalación](#instalación)
  - [Procesos que se crean al levantar un caso](#procesos-que-se-crean-al-levantar-un-caso)
  - [Tiempo de carga](#tiempo-de-carga)
  - [Esquemas](#esquemas)
- [Familia técnica — 15 casos](#familia-técnica--15-casos)
- [Familia mercado de capitales — 21 casos](#familia-mercado-de-capitales--21-casos)
- [Estados de madurez](#estados-de-madurez)

---

## Requisitos comunes a todos los casos

Los requisitos son **los mismos para toda la familia técnica**, porque todos los
casos se levantan con el mismo supervisor (`sandboxctl service up`) y corren bajo
el mismo compilador de políticas. Se documentan aquí una vez en lugar de
repetirlos 15 veces.

### Software necesario

| Componente | Versión | Para qué | ¿Obligatorio? |
|---|---|---|---|
| **Linux o WSL2** | kernel 5.10+ | Los namespaces de usuario sin privilegios son la base de todo | Sí para ejecutar; no para leer o planificar |
| **Rust** | 1.75+ (edición 2021) | El núcleo: `sandboxctl`, políticas, evidencia, mercado de capitales | Sí |
| **`bubblewrap`** | 0.6+ (probado en 0.9.0) | El runtime que aplica los controles de verdad | Sí para `runtime=bwrap` |
| **`util-linux`** (`unshare`, `prlimit`) | 2.37+ | Runtime alternativo, más débil, y límites de recursos | Sí para `runtime=unshare` |
| **`systemd`** en modo usuario | 249+ | Los cgroups v2 (memoria, PIDs, CPU) se piden por `systemd-run --user --scope` | Solo si quieres límites de memoria/CPU reales |
| **Python** | 3.11+ | Los servicios de los casos técnicos 01–05 están escritos con la biblioteca estándar, sin dependencias | Sí para esos casos |
| **Node.js** | 20+ | Panel de control, validación del catálogo y construcción del sitio | Solo para el panel |
| **pnpm** | 9+ | Gestor de paquetes del proyecto. **No se usa `npm`** | Solo para el panel |

Lo que **no** hace falta: Docker, permisos de root, una máquina virtual dedicada
o conexión a internet una vez clonado el repositorio.

> [!WARNING]
> Este es un proyecto **experimental y educativo**. Para cargas de trabajo
> desconocidas de verdad, usa una máquina virtual desechable. **Nunca ejecutes
> malware real en el equipo anfitrión**, ni siquiera dentro de estos sandboxes.

### Instalación

```bash
git clone https://github.com/vladimiracunadev-create/sandbox-labs
cd sandbox-labs
sudo apt install bubblewrap util-linux python3   # Debian/Ubuntu/WSL2
cargo build --release
cargo run -p sandboxctl -- doctor                # dice qué falta en TU equipo
```

`doctor` es el paso que importa: enumera qué runtimes están disponibles, qué
controles puede aplicar cada uno en este equipo concreto y qué controles se
pedirán pero **no** se podrán aplicar. Los pasos completos, incluida la
configuración de WSL2, están en [Instalación](INSTALACION.md).

### Procesos que se crean al levantar un caso

Levantar un caso técnico no arranca un proceso, arranca **hasta cuatro**, y
saber cuáles son es la diferencia entre operar el sistema y adivinar:

```text
sandboxctl service up <caso>
  │
  ├─ systemd --user scope            ← el cgroup: memory.max, pids.max, cpu.max
  │   └─ bwrap                       ← la jaula: namespaces, montajes, seccomp
  │       └─ python3 app.py          ← el servicio, sin red y sin ver el host
  │
  └─ sandboxctl service forward      ← puente TCP↔socket Unix (solo si publish=proxy)
```

El reenviador es un proceso aparte **a propósito**: el servicio no tiene red, así
que escucha en un socket Unix dentro de su jaula y es el puente —fuera de la
jaula— quien publica el puerto en `127.0.0.1`. Si el puente muere, el servicio
queda inalcanzable pero sigue contenido, que es el fallo correcto.

`sandboxctl service down --all` baja todo, **incluidos los huérfanos**: procesos
que quedaron vivos sin registro. La operación diaria está en
[RUNBOOK](../RUNBOOK.md).

### Tiempo de carga

Medido en WSL2 (Ubuntu 24.04, bubblewrap 0.9.0) sobre un portátil corriente. Son
órdenes de magnitud, no promesas de rendimiento:

| Operación | Coste típico | Qué domina |
|---|---|---|
| `cargo build --release` (primera vez) | 1–3 min | Compilación del árbol completo |
| `sandboxctl doctor` | < 100 ms | Sondeo de binarios y del sistema de ficheros |
| Arranque de una jaula `bwrap` | 5–15 ms | Creación de namespaces y montajes |
| Arranque de una jaula `unshare` | 3–8 ms | Menos namespaces, menos montajes |
| Envoltura en cgroup (`systemd-run`) | 30–80 ms | Ida y vuelta al bus de systemd |
| `service up` completo hasta responder `/health` | 0,5–2 s | El intérprete de Python arrancando |
| Una ejecución en `dry-run` | < 50 ms | No ejecuta nada: planifica y firma |

La comparativa de coste entre fronteras se mide en cada commit con
`sandboxctl bench` y queda publicada en el resumen de CI.

### Esquemas

Todo lo que se declara en este proyecto tiene esquema JSON y se valida en cada
commit con `pnpm config:check`. No hay ficheros de configuración de forma libre.

| Esquema | Qué describe | Documento que lo explica |
|---|---|---|
| [`schemas/catalog.schema.json`](../schemas/catalog.schema.json) | El catálogo: runtimes, casos, familias | Este documento |
| [`schemas/policy.schema.json`](../schemas/policy.schema.json) | Una política: qué se pide y con qué rigor | [Referencia de políticas](POLICY_REFERENCE.md) |
| [`schemas/workload.schema.json`](../schemas/workload.schema.json) | Una carga de trabajo ejecutable | [Arquitectura](ARCHITECTURE.md) |
| [`schemas/service.schema.json`](../schemas/service.schema.json) | Un caso levantable como servicio | Este documento |
| [`schemas/evidence.schema.json`](../schemas/evidence.schema.json) | El acta de una ejecución, firmada y encadenada | [Formato de evidencia](EVIDENCE_FORMAT.md) |

La regla que gobierna los esquemas de política y evidencia es una sola: **un
control solicitado, un control aplicado y un control reportado tienen que
describir la misma realidad**. Por eso toda evidencia distingue
`requestedControls`, `effectiveControls`, `unsupportedControls`,
`failedControls` y `observedControls`. Si una política estricta pide un control
obligatorio que este equipo no puede aplicar, **la ejecución no ocurre**: falla
cerrada y explica qué falta.

---

## Familia técnica — 15 casos

Ejecutar código, archivos, plugins, agentes y secretos que no controlas.

**Cada caso tiene su ficha completa** —por qué existe, esquemas, software,
instalación, procesos, tiempo de carga y diagramas— en
[docs/casos/](casos/README.md).

| # | Ficha del caso | Carpeta | Estado | La idea que enseña |
|---|---|---|:--:|---|
| 01 | [Contenido web no confiable](casos/01-contenido-web-no-confiable.md) | `cases/01-untrusted-render` | 🟡 `building` | Quien interpreta contenido ajeno no toca el disco |
| 02 | [Código generado por IA](casos/02-codigo-generado-por-ia.md) | `cases/02-ai-code-runner` | 🟡 `building` | Efímero: se crea, ejecuta y se destruye |
| 03 | [Procesamiento seguro de archivos](casos/03-procesamiento-seguro-de-archivos.md) | `cases/03-file-detonation` | 🟡 `building` | El informe por entrada vale más que el bloqueo |
| 04 | [Plugins de terceros](casos/04-plugins-de-terceros.md) | — | 🔴 `planned` | Conceder capacidades una a una, no restar permisos |
| 05 | [Custodia de claves y firma](casos/05-custodia-de-claves-y-firma.md) | `cases/05-smart-contracts` | 🟡 `building` | El secreto entra solo si manifiesto, política y entorno coinciden |
| 06 | [Detonación en microVM](casos/06-detonacion-en-microvm.md) | — | 🔴 `planned` | Cuando el namespace no basta: máquina desechable |
| 07 | [Runtime determinista de contratos](casos/07-runtime-determinista-de-contratos.md) | — | 🔴 `planned` | Medir el trabajo, no el tiempo |
| 08 | [Sandbox de herramientas de agente IA](casos/08-sandbox-de-herramientas-de-agente-ia.md) | — | 🔴 `planned` | Un prompt inyectado no puede ampliar capacidades |
| 09 | [Runner de CI con pull request externo](casos/09-runner-de-ci-con-pull-request-externo.md) | — | 🔴 `planned` | El código del PR no alcanza el token de CI |
| 10 | [Construcción de paquetes](casos/10-construccion-de-paquetes.md) | — | 🔴 `planned` | Red abierta al resolver, cerrada al compilar |
| 11 | [Renderizado de documentos](casos/11-renderizado-de-documentos.md) | — | 🔴 `planned` | El parser de PDF es el eslabón débil |
| 12 | [Notebooks de ciencia de datos](casos/12-notebooks-de-ciencia-de-datos.md) | — | 🔴 `planned` | Datos de solo lectura, salida separada |
| 13 | [Migraciones de base de datos](casos/13-migraciones-de-base-de-datos.md) | — | 🔴 `planned` | Snapshot y rollback como control |
| 14 | [Análisis de binarios de terceros](casos/14-analisis-de-binarios-de-terceros.md) | — | 🔴 `planned` | Ejecutar lo desconocido en máquina desechable |
| 15 | [Instalación de cadena de suministro](casos/15-instalacion-de-cadena-de-suministro.md) | — | 🔴 `planned` | El `postinstall` es código que nadie leyó |

### 01 · Contenido web no confiable

**Qué resuelve.** Interpretar HTML o Markdown que llega de fuera —un correo, una
fuente RSS, un formulario— sin que el intérprete pueda leer ficheros ni salir a
la red.

**Por qué existe este caso.** Interpretar contenido ajeno es ejecutar la lógica
de otro. Los ataques clásicos **no necesitan JavaScript**, solo que quien
interpreta tenga acceso a algo: una entidad externa en el DOCTYPE hace que el
parser lea `/etc/passwd` (XXE); una imagen apuntando a `169.254.169.254` hace que
el servidor pida las credenciales de la nube por ti (SSRF); un `file://` en un
enlace es una travesía de rutas.

**La idea que enseña.** Separar por **proceso**. Hay un coordinador que conoce el
sistema de ficheros y un intérprete que no lo conoce: recibe texto por la entrada
estándar y devuelve JSON por la salida. Cuando el contenido pide algo de fuera,
no falla con «permiso denegado»: **la capacidad no existe**, y el intento queda
anotado con su motivo.

**Qué hace falta.** Python 3.11+. Nada más: el intérprete usa solo la biblioteca
estándar y no abre sockets ni ficheros.

**Cómo se ejecuta.**

```bash
cargo run -p sandboxctl -- service up untrusted-render   # el producto, en :8801
```

```bash
python3 cases/01-untrusted-render/interpreter.py < contenido.html   # solo el intérprete
```

**Cómo se comprueba que funciona.** `node scripts/verify-cases.mjs` le da nueve
entradas hostiles concretas —XXE, `<script>`, SSRF a metadatos, `onerror=`,
`file://`, `javascript:`, `data:`, enlace Markdown hostil y un documento sin
fin— y comprueba dos cosas por cada una: que el rechazo esperado aparece en el
informe, y que el fragmento peligroso **no** sobrevive en la salida.

**Qué falta.** Ficha en el panel y ejecución del servicio completo bajo `bwrap`
verificada en CI. Por eso está en `building` y no en `ready`.

### 02 · Código generado por IA

**Qué resuelve.** Ejecutar un fragmento de código que acaba de escribir un modelo
de lenguaje, sin que ese fragmento tenga red, disco persistente ni acceso al
entorno del anfitrión.

**Por qué existe este caso.** El código generado no es malicioso ni fiable: es
**desconocido**. Puede tener un bucle infinito, escribir donde no debe o intentar
resolver un nombre de dominio. Ejecutarlo en el mismo proceso que atiende la web
convierte cada fallo del fragmento en un fallo del servicio.

**La idea que enseña.** Lo efímero como control. El sandbox no se reutiliza: se
crea para una ejecución y se destruye con ella. Lo que el fragmento escriba en su
sistema de ficheros desaparece con él.

**Qué hace falta.** Python 3.11+, `bubblewrap`.

**Cómo se ejecuta.** `cargo run -p sandboxctl -- service up ai-code-runner`, en
`:8802`.

**Qué falta.** El rediseño que pide el prompt maestro: hoy el fragmento se
ejecuta dentro del servicio web persistente, y debe pasar a **un sandbox efímero
por ejecución** registrado como trabajo (`panel → job → sandbox → destrucción`),
con cola limitada, cancelación y soporte progresivo de más lenguajes. Está
descrito en [el backlog](IMPLEMENTATION_BACKLOG.md).

### 03 · Procesamiento seguro de archivos comprimidos

**Qué resuelve.** Extraer un archivo comprimido que no controlas y devolver un
informe por entrada, en vez de un «se extrajo correctamente» que no dice nada.

**Por qué existe este caso.** Un ZIP con `../../etc/cron.d/` dentro ha
sobrescrito binarios de sistema en incidentes reales. Es el caso donde el sandbox
**no es opcional**: la extracción escribe en el disco por definición.

**La idea que enseña.** El sandbox como microscopio. Detecta zip slip, zip bomb,
rutas absolutas, rutas de Windows, enlaces simbólicos y duros, nombres Unicode y
archivos anidados, y por cada entrada dice qué hizo y por qué.

**Qué hace falta.** Python 3.11+, `bubblewrap`.

**Cómo se ejecuta.** `cargo run -p sandboxctl -- service up file-detonation`, en
`:8803`.

**Qué falta.** Renombrarlo a `03-safe-archive-processing`, que es lo que hace de
verdad: **no es detonación**. La detonación real —observar comportamiento— es el
caso 06 y necesita una microVM. También faltan más formatos, MIME real y checksum
por entrada.

### 04 · Plugins de terceros

**Estado: `planned`. No hay código.**

**Qué resolverá.** Ejecutar extensiones de terceros donde cada una declara sus
capacidades en un manifiesto, el usuario las aprueba una a una y el runtime
concede exactamente eso y nada más.

**Por qué existe este caso.** El modelo habitual es restar: se da acceso a todo y
se quitan permisos. El modelo correcto es sumar: no hay acceso a nada hasta que
alguien concede una capacidad concreta —una carpeta, una API, el reloj, un
secreto con nombre.

**Qué hará falta.** Manifiesto de capacidades con esquema, flujo de aprobación en
el panel, compilación de la concesión a controles reales, y plugins de ejemplo:
uno correcto, uno excesivamente permisivo, uno que intenta leer de más, uno que
intenta salir a internet, uno que modifica datos y uno con dependencia vulnerable
simulada.

### 05 · Custodia de claves y firma

**Qué resuelve.** Firmar dentro del sandbox sin que la clave privada sea legible
desde fuera, y sin que el proceso que firma tenga por dónde sacarla.

**Por qué existe este caso.** Una clave de firma es el activo que no se puede
rotar barato. Que el proceso que la usa tenga red es suficiente para perderla.

**La idea que enseña.** El secreto se inyecta **solo si manifiesto, política y
entorno coinciden**; la firma ocurre dentro; la comunicación es por socket Unix;
la red es `none`; y hay una prueba de exfiltración que intenta sacarla y falla.

**Qué hace falta.** Python 3.11+, `bubblewrap`.

**Cómo se ejecuta.** `cargo run -p sandboxctl -- service up smart-contracts`, en
`:8805`.

**Qué falta.** Dividirlo: la parte de custodia y firma pasa a
`05-key-custody-and-signing`, y la ejecución determinista se va al caso 07, que
es otra idea distinta. Faltan también límites de monto, rotación y revocación.

### 06 · Detonación en microVM · `planned`

**Qué resolverá.** Observar el comportamiento de una muestra —procesos, ficheros
creados, intentos de persistencia, conexiones— en una máquina virtual desechable
con snapshot y destrucción posterior. **Con muestras sintéticas e inofensivas
únicamente; el repositorio no contiene ni contendrá malware real.**

**Por qué existe.** Un namespace comparte kernel. Para observar algo que intenta
activamente escapar, la frontera tiene que ser una máquina, no un namespace.

**Qué hará falta.** Firecracker o Kata, KVM, un kernel y un rootfs, e
instrumentación con línea de tiempo.

### 07 · Runtime determinista de contratos · `planned`

**Qué resolverá.** Ejecutar lógica con resultado reproducible: presupuesto por
instrucciones en vez de tiempo, sin reloj, sin red, estado inicial explícito,
serialización canónica, rollback ante fallo y registros deterministas.

**Por qué existe.** El determinismo es un control de aislamiento distinto de
todos los demás: no restringe lo que el código puede tocar, restringe **lo que
puede saber**. Un reloj o un número aleatorio bastan para que dos ejecuciones no
coincidan.

**Qué hará falta.** WASI (`wasmtime`) o una máquina virtual mínima con contador
de instrucciones.

### 08 · Sandbox de herramientas de agente IA · `planned`

**Qué resolverá.** Un agente con herramientas limitadas —ficheros, web, correo
simulado, terminal, base de datos, secretos, aprobación humana— donde **un prompt
inyectado no puede ampliar sus propias capacidades**.

**Por qué existe.** El contenido que un agente lee es dato, no instrucción. Si la
herramienta que concede permisos está al alcance del texto que el agente procesa,
el aislamiento es decorativo.

### 09 · Runner de CI con pull request externo · `planned`

**Qué resolverá.** Ejecutar el código de un pull request de un desconocido con
checkout aislado, sin secretos en el entorno, red limitada y —sobre todo— sin
acceso al token de CI.

**Por qué existe.** Es el caso donde ejecutar código ajeno con privilegios está
institucionalizado, y donde una fuga no compromete un equipo sino un repositorio
entero y todo lo que despliega.

### 10 · Construcción de paquetes de terceros · `planned`

**Qué resolverá.** Compilar dependencias de terceros con red **abierta mientras
se resuelven** y **cerrada mientras se compila**, con caché y SBOM.

**Por qué existe.** Un script de construcción corre con tus permisos y con la red
abierta. Cerrar la red después de resolver es un control barato que casi nadie
aplica.

### 11 · Renderizado de documentos · `planned`

**Qué resolverá.** Procesar PDF, documentos ofimáticos, imágenes, tipografías y
metadatos evitando exploits de parser, travesía de rutas y consumo excesivo.

**Por qué existe.** Los parsers de formatos complejos están escritos en C y
llevan décadas siendo la vía de entrada favorita. El documento no tiene que ser
malicioso a la vista: basta con que el parser tenga un fallo.

### 12 · Notebooks de ciencia de datos · `planned`

**Qué resolverá.** Ejecutar notebooks con datasets montados de solo lectura,
salida separada, red configurable, cuotas y limpieza al terminar.

**Por qué existe.** Un notebook es código arbitrario con acceso a los datos de
producción, ejecutado por gente que no lo escribió.

### 13 · Migraciones de base de datos · `planned`

**Qué resolverá.** Ejecutar migraciones no confiables contra una base simulada
con snapshot, rollback, presupuesto, detección de consultas peligrosas y
comparación de esquema antes y después.

### 14 · Análisis de binarios de terceros · `planned`

**Qué resolverá.** Ejecutar binarios desconocidos en una microVM desechable, con
muestras sintéticas.

### 15 · Instalación de cadena de suministro · `planned`

**Qué resolverá.** Simular paquetes comprometidos, scripts `postinstall`,
typosquatting y dependencias transitivas.

**Por qué existe.** `npm install` ejecuta código de cientos de personas que nunca
verás, con tus permisos, antes de que hayas escrito una línea.

---

## Familia mercado de capitales — 21 casos

Probar modelos Fintech con **dinero, instrumentos y participantes simulados**.

> [!WARNING]
> **Sin dinero real. Sin valores reales. Sin credenciales reales. Sin
> conectividad de producción.** Este simulador **no es una autorización
> regulatoria** de la CMF ni de ninguna otra autoridad, y nada de lo que produzca
> es una recomendación de inversión.

Por defecto, todo caso de esta familia corre con:

```json
{ "moneyMode": "simulated", "realMoney": false, "realSecurities": false, "productionConnectivity": false }
```

| # | Ficha del caso | Carpeta | Estado | Qué prueba |
|---|---|---|:--:|---|
| CM-00 | [Entrada al sandbox regulatorio](casos/cm-00-entrada-al-sandbox-regulatorio.md) | — | 🔴 `planned` | Clasificar un modelo de negocio y emitir aprobación, condicionada o rechazo |
| CM-01 | [Financiamiento colectivo](casos/cm-01-financiamiento-colectivo.md) | — | 🔴 `planned` | Campañas, meta mínima, sobredemanda, asignación y devolución |
| CM-02 | [Sistema alternativo de transacción](casos/cm-02-sistema-alternativo-de-transaccion.md) | `domains/capital-markets/cases/02-alternative-trading-system` | 🟠 `prototype` | Libro de órdenes con prioridad precio-tiempo |
| CM-03 | [Custodia y segregación de activos](casos/cm-03-custodia-y-segregacion-de-activos.md) | `domains/capital-markets/cases/03-asset-custody` | 🟢 `functional` | Que los activos de clientes cuadren con los custodiados |
| CM-04 | [Enrutamiento inteligente de órdenes](casos/cm-04-enrutamiento-inteligente-de-ordenes.md) | — | 🔴 `planned` | Decidir dónde ejecutar y **poder explicar por qué** |
| CM-05 | [Intermediación financiera](casos/cm-05-intermediacion-financiera.md) | — | 🔴 `planned` | Agente frente a principal, y detectar front-running |
| CM-06 | [Asesoría crediticia](casos/cm-06-asesoria-crediticia.md) | — | 🔴 `planned` | Capacidad de pago, costo total y conflictos comerciales |
| CM-07 | [Robo-advisor](casos/cm-07-robo-advisor.md) | — | 🔴 `planned` | Perfil, cartera y recomendación explicable y versionada |
| CM-08 | [Tokenización de instrumentos](casos/cm-08-tokenizacion-de-instrumentos.md) | — | 🔴 `planned` | Que las unidades emitidas no superen el respaldo |
| CM-09 | [Vigilancia de abuso de mercado](casos/cm-09-vigilancia-de-abuso-de-mercado.md) | — | 🔴 `planned` | Wash trading, spoofing, layering, manipulación del cierre |
| CM-10 | [Compensación y liquidación](casos/cm-10-compensacion-y-liquidacion.md) | — | 🔴 `planned` | Netting y entrega contra pago, con fallas y reversas |
| CM-11 | [Finanzas abiertas y consentimiento](casos/cm-11-finanzas-abiertas-y-consentimiento.md) | — | 🔴 `planned` | Alcance, renovación, revocación y trazabilidad del consentimiento |
| CM-12 | [Reportería regulatoria y SupTech](casos/cm-12-reporteria-regulatoria.md) | — | 🔴 `planned` | Consolidar, validar, firmar y corregir sin alterar el histórico |
| CM-13 | [Salida ordenada](casos/cm-13-salida-ordenada.md) | — | 🔴 `planned` | Cerrar la operación devolviendo todo lo que no es tuyo |
| CM-14 | [Resiliencia operacional](casos/cm-14-resiliencia-operacional.md) | — | 🔴 `planned` | Kill switch, degradación controlada, replay y post mortem |
| CM-15 | [KYC, AML y sanciones](casos/cm-15-kyc-aml-y-sanciones.md) | — | 🔴 `planned` | Riesgo, PEP y sanciones **simuladas**, sin datos personales reales |
| CM-16 | [Integridad de datos de mercado](casos/cm-16-integridad-de-datos-de-mercado.md) | — | 🔴 `planned` | Precio cero, moneda incorrecta, timestamp futuro, dato obsoleto |
| CM-17 | [Eventos corporativos](casos/cm-17-eventos-corporativos.md) | — | 🔴 `planned` | Dividendos, splits y canjes que actualizan posiciones y costo |
| CM-18 | [Margen, garantías y riesgo](casos/cm-18-margen-garantias-y-riesgo.md) | — | 🔴 `planned` | Haircut, margen inicial y de variación, liquidación forzada |
| CM-19 | [Fraude y toma de cuentas](casos/cm-19-fraude-y-toma-de-cuentas.md) | — | 🔴 `planned` | Dispositivo nuevo, retiro anómalo, sesión imposible |
| CM-20 | [Gobierno de modelos e IA financiera](casos/cm-20-gobierno-de-modelos-e-ia-financiera.md) | — | 🔴 `planned` | Versión, métricas, sesgo, drift, rollback y supervisión humana |

### La base compartida: dinero y libro contable

Antes que cualquier caso está el crate [`sandbox-markets`](../crates/sandbox-markets),
porque todos los casos financieros se apoyan en dos piezas y las dos son fáciles
de hacer mal:

- **`Money`** — enteros en unidades mínimas, nunca coma flotante, con la moneda
  pegada al importe. `Money` de pesos y `Money` de dólares no se suman: no
  compila. Un `f64` para dinero es un error que aparece tarde y en producción.
- **`Ledger`** — partida doble, solo se añade, con reversas y con idempotencia.
  Una transacción que no cuadra no se registra.

Esto **sí está construido y probado**.

### CM-02 · Sistema alternativo de transacción — `prototype`

**Qué resuelve.** Un libro de órdenes con prioridad precio-tiempo: la orden que
descansa fija el precio, y el libro nunca queda cruzado.

**Por qué existe.** La prioridad precio-tiempo es la regla que hace que un
mercado sea un mercado y no una cola arbitraria. Romperla es la forma más simple
de dar ventaja a alguien.

**Qué hay.** 11 invariantes en Rust sobre `OrderBook`, `Order` y `Trade`.

**Qué falta.** Los escenarios que pide el prompt maestro —volatilidad, falta de
liquidez, precio anómalo, duplicación, latencia, órdenes fuera de banda,
interrupción de mercado— y la reconstrucción completa de la sesión. Por eso es
`prototype` y no `functional`.

### CM-03 · Custodia y segregación de activos — `functional`

**Qué resuelve.** Comprobar que se cumple el invariante que define la custodia:

```text
Activos de clientes registrados = Activos custodiados + Operaciones pendientes justificadas
```

**Por qué existe.** Cuando un custodio quiebra, la pregunta no es cuánto dinero
había, es **de quién era**. Si los activos de los clientes se mezclaron con los
de la casa, esa pregunta ya no tiene respuesta. La segregación no es papeleo: es
lo que decide si un cliente recupera lo suyo.

**Qué detecta.** Faltantes, sobrantes, posiciones negativas de cliente, cuentas
mezcladas y movimientos pendientes sin justificación.

**Cómo se comprueba que funciona.**

```bash
cargo run -p sandboxctl -- markets reconcile
```

Seis escenarios, cada uno con el hallazgo que **debe** producir. Corre en cada
commit: si la conciliación deja de detectar lo que declara, CI se pone rojo.

**Qué falta.** Dividendos, bloqueos, garantías y el escenario de insolvencia del
custodio.

---

## Estados de madurez

Ningún caso se describe como «listo». Se usan estos cinco estados y ninguno más:

| Estado | Qué significa exactamente |
|---|---|
| `planned` | Está especificado y **no hay código**. |
| `prototype` | Hay código que corre, sin prueba de comportamiento que lo respalde. |
| `functional` | Se ejecuta y **existe una prueba concreta** que demuestra lo que declara. |
| `verified` | Además genera evidencia firmada, y CI la valida en un entorno compatible. |
| `production-research` | Investigación, no algo que se pueda desplegar. |
| `deprecated` | Se conserva por compatibilidad, no se usa. |

Los criterios completos para subir de `prototype` a `functional` y de
`functional` a `verified` están en
[el backlog de implementación](IMPLEMENTATION_BACKLOG.md).

---

## Y también

- **[Estado del proyecto](ESTADO.md)** — qué se construyó, qué lo demuestra y qué falta
- [Referencia de políticas](POLICY_REFERENCE.md) — cada control y a qué mecanismo del kernel se traduce
- [Formato de evidencia](EVIDENCE_FORMAT.md) — qué se firma y cómo se verifica
- [Suite de contención](CONTAINMENT_SUITE.md) — las sondas que intentan escaparse
- [Glosario](GLOSARIO.md) — el vocabulario, sin dar nada por sabido
