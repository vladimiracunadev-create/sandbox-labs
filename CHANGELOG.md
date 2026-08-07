# 📋 Changelog

Formato basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/).
Versionado semántico.

---

## [Unreleased]

### Fixed — tres afirmaciones del núcleo que no eran ciertas

- **La evidencia declaraba una red aislada que bubblewrap no aislaba.** El
  adaptador escribía siempre `network → "isolated network namespace"` en los
  límites efectivos, pero `--unshare-net` solo se añade con `network.mode:
  none`. La política de **todos** los servicios del catálogo pedía otra cosa, así
  que justo donde la carga conservaba la red del host la evidencia decía lo
  contrario. El cálculo salió a la función pura `effective_limits`, con pruebas.
- **`loopback` era un sinónimo suave de «sin aislar».** De los cuatro modos de
  red, solo `none` creaba namespace propio; los otros tres conservaban la red
  del host entera. Ahora `none` y `loopback` crean namespace, `allowlist` y
  `unrestricted` no, y la diferencia vive en `NetworkPolicy::isolates_host_network()`
  en vez de en cuatro comparaciones sueltas contra la cadena `"none"`.
- **`ai-agent-restricted` vendía un filtrado de egress inexistente.** No hay
  proxy de salida ni reglas de firewall que hagan cumplir `network.hosts`. La
  política se queda como la frontera que se quiere, pero su descripción avisa de
  que ningún runtime la aplica y de que, al ser estricta, no ejecuta.
- **Un servicio TCP con red aislada esperaba veinte segundos a nada.** El puerto
  nacía dentro del namespace y no era alcanzable desde el host. Ahora
  `sandboxctl service up` falla en cerrado, nombra el modo que lo provoca y da
  las dos salidas.
- **Las variables del bus de systemd se filtraban al PID 1 del sandbox.**
  Regresión introducida al añadir cgroups y encontrada por la suite de
  contención en CI, no por una revisión: `--clearenv` de bubblewrap limpia el
  entorno de la carga, no el del propio `init`, así que la carga leía
  `XDG_RUNTIME_DIR` y `DBUS_SESSION_BUS_ADDRESS` en `/proc/1/environ`. El primer
  arreglo las borraba una a una con `env -u` y falló igual, porque systemd
  inyecta `INVOCATION_ID` por su cuenta; la cadena intercala ahora un `env -i` y
  el runtime arranca con el entorno vacío. El contrato es vaciar, no enumerar.

### Added — CM-02, motor de libro de órdenes con prioridad precio-tiempo

- **Un mercado justo se reduce a una regla**: mejor precio primero, y a igual
  precio quien llegó antes. Once invariantes con una prueba cada una.
- **El tamaño no adelanta la cola.** Es el abuso clásico —servir primero al
  grande— y tiene prueba propia: un vendedor diez veces mayor sigue esperando su
  turno detrás de uno pequeño que llegó antes.
- **El precio lo pone la orden que ya estaba.** Quien puso precio y esperó tiene
  derecho a él; si mandara la orden entrante, llegar tarde sería una ventaja.
- **El libro nunca queda cruzado**: una compra que paga igual o más que la mejor
  venta significa una ejecución que no ocurrió.
- **Una orden de mercado sin contrapartida se rechaza**, no reposa: no tiene
  precio con el que esperar, y dejarla en el libro sería inventarle uno.
- El número de secuencia lo pone **el libro**, no quien manda la orden. Si lo
  pusiera el cliente, podría colarse delante diciendo que llegó antes. Y es un
  contador, no un reloj: dos órdenes en el mismo milisegundo desempatarían por el
  orden interno de un mapa, que es el azar.
- Queda como `prototype` y no `functional` mientras le falten escenarios
  ejecutables como los de CM-03.

### Added — CM-03, el primer caso de mercado de capitales que se ejecuta

- **Custodia y segregación de activos**, sobre la invariante
  `registrado = custodiado + pendiente explicado`. Se ejecuta con
  `sandboxctl markets reconcile`.
- **Los activos propios del custodio nunca entran en la comparación.** Es el
  fallo que la segregación existe para impedir: en el escenario `FALTANTE-003`
  hay 1.300 títulos en total y parece que sobran, pero a los clientes les faltan
  200 — los 1.000 propios no son de ellos.
- **Un pendiente sin motivo no explica nada.** La diferencia entre una
  explicación y una excusa: sin `reason` el pendiente no tapa el hueco y además
  se denuncia por sí mismo.
- **Sobrar también es un hallazgo.** No tranquiliza: significa que el registro
  no describe la realidad, y mañana puede ser al revés.
- **Seis escenarios que declaran lo que esperan detectar.** Uno adverso que deje
  de provocar su hallazgo se marca como roto y devuelve código 1 — un escenario
  que aprueba pase lo que pase es decoración. Y un hallazgo que aparece sin
  estar declarado también se marca: el escenario ya no describe lo que ocurre.
- El extracto del custodio es un **dato externo** a propósito. Si se derivara del
  registro, la conciliación no compararía nada.

### Added — cimientos de mercado de capitales

- **Catálogo con dos familias**, `technical` y `capital-markets`, que **no se
  mezclan**: tienen modelos de amenazas distintos, y mezclarlas haría que una
  advertencia de una se leyera como si valiera para la otra. Nada de lo
  existente cambia de sitio.
- **Dinero exacto** (`crates/sandbox-markets`): enteros en la unidad mínima,
  nunca coma flotante. La moneda va pegada al importe, así que sumar CLP y USD
  **falla** en vez de salir mal. El peso chileno tiene cero decimales, que es el
  caso que rompe el código escrito asumiendo «dos decimales siempre».
- **Libro mayor de doble entrada**, solo-añadir, con una prueba por invariante:
  cada transacción cuadra a cero, nada se borra —un error se corrige con una
  reversa y las dos quedan—, una transacción se aplica una vez, y los saldos se
  pueden reconstruir desde el diario.
- **`domains/capital-markets/`** con su estructura y un README que lista los 21
  casos **como plan y no como catálogo**. Ninguno existe todavía, y ninguno
  aparece como disponible en el panel ni en el sitio.

### Added — la evidencia se firma, se encadena y dice qué significó

- **Firma Ed25519** sobre la huella, con clave local generada la primera vez y
  guardada fuera del repositorio con permisos de solo su dueño. Detecta lo que
  la huella sola no veía: que alguien **rehiciera** el documento recalculando su
  SHA-256.
- **Cadena entre evidencias** (`previousEvidenceSha256`): detecta que alguien
  **borre** un informe entero, cosa que una firma no ve — las que quedan siguen
  siendo válidas.
- **`verdict`**, que responde a «¿se puede confiar en esta ejecución?» y no a
  «¿terminó bien?». Una carga que sale con código 0 habiendo perdido un control
  pedido es `controls-missing`.
- **`artifacts`** con el hash de lo que produjo, y **`cleanup`** con lo que
  retiró al terminar.
- `sandboxctl evidence verify` comprueba las cuatro cosas. Probado contra
  manipulación real: rehacer el JSON rompe la firma, borrar una evidencia rompe
  la cadena, y las dos devuelven código 1.
- **No es una notarización, y se dice.** La clave la guarda la misma máquina que
  ejecuta: prueba que el informe no cambió tras escribirse, no que la ejecución
  ocurriera.
- La tolerancia a la caché de DrvFs —`mkdir` dice «existe» y `ls` dice que no—
  pasa a `sandbox_core::dirs`, en un solo sitio. Estaba en el lanzador de
  servicios y volvió a hacer falta para guardar la clave, que sin ella no
  llegaba a existir en la plataforma objetivo.

### Added — `network: allowlist` deja de ser una lista que nadie hacía cumplir

- **La salida se entrega como capacidad, no como propiedad del entorno.** Con
  `allowlist` la carga corre en su propio namespace de red, sin ruta hacia
  fuera. Lo único que cruza la frontera es un socket Unix por el que pide
  `CONNECT host:puerto`; un proxy del supervisor —que vive fuera y por eso sí
  tiene red— aplica la lista y decide.
- **Registro de todos los intentos**, permitidos y denegados, en
  `networkEvents` de la evidencia: destino, veredicto, motivo y bytes movidos.
  Un proxy que filtra y no cuenta lo que dejó pasar no permite auditar nada.
- **Sin comodines.** `*.ejemplo.com` es exactamente cómo una lista de permitidos
  deja de serlo.
- El control `network` pasa a ser efectivo con `allowlist`, porque el namespace
  se crea igual que con `none`.
- Medido con bubblewrap 0.9.0 contra un destino local: desde dentro,
  `sin-canal=ConnectionRefusedError`, `permitido=200` con la respuesta real del
  destino y `denegado=403`.
- La contrapartida, dicha entera: un cliente HTTP corriente no usa el canal
  solo, tiene que abrir el socket a propósito. `unshare` no lo monta, así que
  con él `allowlist` deja a la carga sin salida ninguna.

### Fixed — un solo compilador de política, y tres controles que se perdían

- **`sandbox_core::compiler`** produce los argumentos de bubblewrap para las
  cargas que terminan y para los servicios. Antes cada camino tenía su lista
  escrita a mano, y al de los servicios le faltaban `--cap-drop ALL` —aunque su
  política exige el control `capabilities`—, `--uid`/`--gid`, `--new-session`
  (lo que impide inyectar en el terminal con `TIOCSTI`), `--unshare-cgroup-try`,
  el filtro seccomp y los límites de cgroups.
- **El registro del servicio declaraba controles que nadie aplicaba.** Copiaba
  `runtime.supported_controls()`, que describe lo que bubblewrap puede aplicar a
  una carga, así que la tarjeta del panel prometía `memory`, `processes` y `cpu`
  sin que existieran. Ahora los servicios reciben el mismo scope de cgroups y se
  registra lo que ese camino aplicó.
- **Un servicio con bubblewrap moría al terminar `service up`.** Llevaba
  `--die-with-parent`, correcto para una carga supervisada y letal para un
  proceso que debe sobrevivir al CLI. No se había visto porque los servicios se
  probaban con `unshare`, que no tiene esa opción. Ahora es un campo explícito
  de la petición.
- **`service-isolated` deniega llamadas al sistema**, así que los casos `02` y
  `03` corren también con filtro seccomp.
- El programa envuelto en el scope de systemd se resuelve a **ruta absoluta**:
  detrás del `env -i` no hay `PATH`, y `execvp` solo miraba la ruta por defecto
  del sistema. Un runtime instalado en `/usr/local/bin` no se encontraba.

### Added — los perfiles seccomp pasan de fichero a filtro real

- **`policy.syscalls.deny` se compila a un programa BPF** con `seccompiler`
  —Rust puro, frente a `libseccomp`, que exigiría la biblioteca C en cada host y
  en CI— y bubblewrap lo recibe por descriptor con `--seccomp`. Las llamadas
  denegadas devuelven `EPERM`.
- **Denegación y no lista de permitidos**, porque es lo que declaran las
  políticas y porque una lista de permitidos incompleta no es más segura: es un
  sandbox que no arranca, y se acaba ampliando hasta que ya no contiene nada.
- **`EPERM` en vez de matar el proceso**, para que la carga siga viva y pueda
  contarlo — que es lo que permite medirlo.
- **Sonda `seccomp-filter`**, octava de la suite. Mide con `getcpu`, que tiene
  éxito siempre y para cualquiera: éxito = ningún filtro la bloqueó, `EPERM` = el
  filtro la denegó. Medir con llamadas peligrosas no vale —ya fallan con `EPERM`
  sin privilegios— y medir con `perf_event_open` tampoco: su error sin filtro
  depende de `perf_event_paranoid` del host, y falló en CI por eso. Medido con
  bubblewrap 0.9.0: sin sandbox escapa, bubblewrap sin filtro escapa, bubblewrap
  con filtro contiene.
- `syscalls` entra en el contrato de contención que CI exige a bubblewrap.
- Una prueba unitaria aplica el BPF a un hilo real y comprueba el salto de
  `EFAULT` a `EPERM`, así que la compilación se verifica sin bubblewrap.

### Added — el README y la portada dicen qué se aplica

- Tabla de **control → mecanismo del kernel → estado**, con los huecos marcados
  como tales. `allowlist` aparece con «sin enforcement: no se declara».
- La portada del sitio deja de describir un sandbox genérico y enseña lo mismo.

### Added — la evidencia se puede verificar

- **`integrity.evidenceSha256`**: cada evidencia se sella con un SHA-256 de su
  propio contenido, calculado con ese campo vacío.
- **`sandboxctl evidence verify`** recalcula la huella y vuelve a hashear la
  política y la carga que la evidencia dice haber ejecutado. Distingue dos cosas
  que se confunden: que alguien editara el informe (huella rota, hashes bien) y
  que el código haya cambiado desde entonces (huella bien, hash roto). Lo
  segundo no es corrupción; es un informe viejo diciendo con razón que ya no
  describe el código de hoy.
- Comprobado contra manipulación real: cambiar el estado o añadir a mano un
  control efectivo que nunca se aplicó rompen la huella y devuelven código 1.
- Corre como paso de CI: una evidencia que no se verifique tumba el build.
- **No es una firma.** Quien pueda editar el fichero puede recalcular la huella.
  Detecta la alteración accidental o descuidada, que es el caso que se da en la
  práctica. La firma con clave local sigue en el backlog.
- Las comprobaciones que no se pudieron hacer se informan aparte y no cuentan
  como aprobado.

### Added — la evidencia dice también lo que la carga consumió

- **`limits.observed`**: `memoryPeakBytes`, `pidsPeak`, `cpuUsageUsec` y
  `oomKills`, leídos del cgroup **mientras la carga corre**. Hay que muestrear
  porque systemd retira el cgroup en cuanto el scope termina: leer al final no
  encuentra nada. Aplicar un límite y medir el consumo son cosas distintas, y
  hasta ahora solo se hacía lo primero.
- **`oomKills` convierte un código de salida inexplicable en un hecho.** Sin él,
  un proceso que el kernel mató por memoria se parece a uno que falló solo.
- El muestreo solo ocurre cuando hubo cgroup propio. Sin envoltorio,
  `/proc/<pid>/cgroup` apunta al de la sesión del host, y publicar sus cifras
  como consumo de la carga serían números reales de la máquina equivocada.
- Un campo ausente significa «no se pudo leer», nunca «cero»: lo que no se midió
  no se publica.
- Una prueba envuelve un proceso real, le hace reservar 40 MB y comprueba que el
  pico observado los refleja — si el muestreo llegara tarde, no habría nada que
  leer y lo diría.

### Fixed — la carga corría con tu identidad, no con la de la política

- **`--uid`/`--gid` de la política, aplicados.** `process.user` y
  `process.group` valían 65534 en todas las políticas y bubblewrap nunca los
  recibía. Lo que pasaba de verdad era peor que «corre como root mapeado»: la
  carga corría con **el uid real de quien la lanzó** y heredaba sus grupos
  suplementarios. Medido con bubblewrap 0.9.0: `uid=1000(vbav)
  groups=1000,65534` antes, `uid=65534(nobody) groups=65534` después.
- **Los servicios ganan `--cap-drop ALL`**, que las cargas breves ya tenían y a
  ellos les faltaba aunque su política exigía el control `capabilities`.
- La evidencia registra la identidad aplicada en `limits.effective.user`.
- Una prueba de contrato impide que ninguna política vuelva a declarar uid o
  gid 0, que hasta ahora habría sido una anotación ignorada y ahora sería una
  identidad de verdad.

### Added — un servicio puede contener la red y seguir publicando su puerto

- **Reenviador TCP → socket Unix en el supervisor.** Hasta ahora, todo servicio
  que quisiera abrirse en el navegador tenía que conservar la red del host: su
  puerto lo enlazaba él, y dentro de un namespace de red propio ese puerto no es
  alcanzable desde fuera. Ahora el servicio escucha en un socket Unix y el
  supervisor publica el puerto por él, empalmando cada conexión. El servicio
  sigue hablando HTTP; solo cambia el transporte por debajo.
- **`publish` en el manifiesto de servicio**, que separa *cómo escucha el
  servicio* de *cómo llega el host*: `direct` (lo enlaza el servicio, necesita
  la red del host), `proxy` (lo publica el supervisor, el sandbox se queda sin
  red) y `none` (solo el socket). Los manifiestos sin el campo siguen
  significando lo mismo que antes.
- **Política `service-isolated`**: como `service-sandbox` pero con
  `network: loopback` y exigiendo el control `network`.
- **Los casos `02-ai-code-runner` y `03-file-detonation` pasan a red contenida.**
  Eran la frontera abierta más grande que quedaba en el catálogo. Medido
  levantando el caso 03: `curl http://127.0.0.1:8803/health` → 200, con el
  sandbox en `net:[4026532244]` y el host en `net:[4026531833]`.
- El reenviador se registra en `proxyPid` y se baja con el sandbox: si
  sobreviviera, dejaría el puerto ocupado y el siguiente `up` fallaría
  señalando a un servicio que ya no existe. Tiene techo de conexiones
  simultáneas, porque corre **fuera** del sandbox y por tanto fuera del cgroup.

### Added — límites de recursos que existen de verdad

- **cgroups v2 en bubblewrap.** `memoryMb`, `processes` y `cpu` pasan de
  documentación a `memory.max`, `pids.max` y `cpu.max` a través de
  `systemd-run --user --scope`. Escribir el cgroup a mano no vale en la
  plataforma objetivo: en WSL2 el proceso arranca en `/init.scope`, que existe y
  no es escribible.
- **El sondeo usa el mecanismo en vez de suponerlo.** Antes de la primera
  ejecución se levanta un scope real con los tres límites puestos —la misma
  forma de comando que se ejecutará después— y solo si el kernel los acepta se
  declaran los controles. `sandboxctl doctor` muestra el resultado y deja de
  comprobar únicamente que `/sys/fs/cgroup/cgroup.controllers` exista.
- **`docs/IMPLEMENTATION_BACKLOG.md`**: los huecos del núcleo con qué falta, qué
  se hace en su lugar y qué haría falta para cerrarlos. El código lo enlazaba
  desde un comentario y no existía.

### Added — el repositorio deja de describir aislamiento y pasa a medirlo

- **`sandboxctl escape`: suite de contención.** Siete sondas que **intentan
  salirse** del sandbox (red, filesystem, visibilidad de procesos, fuga de
  entorno, privilegios efectivos, memoria y procesos) ejecutadas bajo cada
  runtime, con una matriz de resultados. Cada sonda es una carga registrada
  normal: se ejecuta por el mismo camino que el resto, porque una vía especial
  no mediría el sistema real.
- **Veredicto `❌ DECLARADO`** para el caso más peligroso: el runtime declara el
  control y la sonda demuestra que no lo aplica. Peor que no declararlo, porque
  invita a confiar.
- **`sandboxctl bench`: comparativa entre fronteras.** Misma carga y misma
  política en todos los runtimes, con p50, p95 y sobrecoste contra el más
  rápido. Repetición de calentamiento descartada; se reporta la cola porque una
  media sola esconde justo el caso que hará esperar al usuario.
- **Trabajo `isolation` en CI**: instala bubblewrap y ejecuta la suite de
  verdad. Tres comprobaciones que se sostienen entre sí — bubblewrap debe
  contenerlo todo (`--strict`), unshare debe cortar red y PIDs, y **native
  debe ESCAPAR**. Esta última es una contraprueba deliberada: si sin
  aislamiento saliera todo contenido, las sondas no estarían midiendo nada.
- Política `containment-audit`: `best-effort` a propósito, porque una `strict`
  falla cerrada antes de ejecutar y no mediría nada.
- `docs/CONTAINMENT_SUITE.md` y esquema `escape-suite.schema.json`.

### Fixed — hallazgos de la propia suite

- **PID namespace sin `/proc` remontado.** El adaptador `unshare` pasaba
  `--pid --fork` y creaba el namespace, pero sin `--mount-proc` el proceso
  seguía leyendo el `/proc` del host y enumeraba sus 48 PIDs. El namespace
  existía y no se notaba.
- **`RLIMIT_NPROC` no es un límite de procesos de contenedor.** Los adaptadores
  declaraban el control `processes` porque envolvían la carga con
  `prlimit --nproc`, pero RLIMIT_NPROC cuenta los procesos del UID en **todo el
  host**: fijarlo al presupuesto de la política mataba la ejecución al arrancar
  y hacía pasar por contención algo que no lo era. Se retiró, y el control
  `processes` ya no se declara hasta que exista con cgroups v2.

### Changed — laboratorios profesionales

- Los 18 laboratorios reescritos: de plantillas de 35 líneas a ~105 líneas con
  concepto, motivo, diagrama Mermaid, comandos reales sobre la nueva
  herramienta, salida esperada, verificación, caso de uso y errores comunes.
- Estado de cada laboratorio sincronizado con el catálogo, **con una prueba de
  contrato que impide que vuelvan a divergir** y que además exige que cada
  README traiga diagrama, práctica y verificación.
- Los adaptadores `bwrap` y `unshare` declaran ahora `memory` (RLIMIT_AS
  verificado en el host) y ya no declaran `processes`.

### Security

- `sha2` actualizado a 0.11 (cambio mayor). La versión 0.11 dejó de
  implementar `LowerHex` sobre la salida del digest, así que la codificación
  hexadecimal se centraliza en `sandbox_core::hash` en lugar de repetirse en
  cada llamador: la próxima actualización de la dependencia deja de ser una
  migración a mano. El módulo trae vectores de prueba del NIST.

- **Todas las acciones de GitHub fijadas a SHA** con el tag en comentario. Un
  tag es mutable: `@v5` puede apuntar mañana a otro código. `zizmor` lo
  verifica en cada ejecución, así que deja de ser una convención olvidable.
- `persist-credentials: false` en todos los checkouts: por defecto el token de
  Actions queda en `.git/config` y cualquier paso posterior puede leerlo.
  Ningún workflow del repositorio necesita empujar commits.
- Permisos reducidos al mínimo por trabajo. `pages: write` e `id-token: write`
  ya no se declaran a nivel de workflow.
- El workflow de release ya no restaura caché de pnpm: no debe consumir una
  caché que otra rama pudo haber escrito.
- `softprops/action-gh-release` sustituido por `gh release create`, que ya
  viene en el runner — una acción de terceros menos en la ruta que firma el
  release.

### Added

- **Trabajo `panel` en CI**: arranca el servidor real y comprueba el contrato
  de la API de extremo a extremo (modo seguro, `403` sin cabecera de
  confianza, `404` en comandos arbitrarios, `421` anti DNS-rebinding y un
  trabajo registrado que llega a estado terminal con evidencia).
- CI verifica que `control-center/dist/` versionado coincide con lo que genera
  el build: el build es determinista, así que una diferencia significa que
  alguien editó `src/` sin regenerar `dist/`.
- **`actionlint`** además de `zizmor` en el workflow de seguridad: corrección
  de sintaxis, expresiones y shell embebido, no solo seguridad.
- Workflow **Pages**, que publica `site/` y comprueba antes que la portada esté
  completa y no cargue recursos externos.
- Workflow **Release** rehecho: valida que la versión cuadre en los cinco
  manifiestos, ejecuta la puerta de calidad completa y después **abre el ZIP y
  cuenta lo que lleva dentro** — un artefacto puede compilar, cuadrar de
  checksum y estar vacío.
- Caché de dependencias de Cargo en CI.
- `timeout-minutes` en todos los trabajos.
- Resúmenes en `$GITHUB_STEP_SUMMARY` para Rust, Pages y Release.
- `docs/CI_WORKFLOWS.md`: qué garantiza cada workflow y cómo reproducirlo.

### Changed

- `docs.yml` se divide en dos trabajos —enlaces y lint— porque fallan por
  motivos distintos y el fallo debe decir qué arreglar.
- `dependabot.yml`: actualizaciones agrupadas por ecosistema, con etiquetas,
  prefijo de commit y límite de PRs abiertos.

---

## [0.7.0] - 2026-08-05

Primera versión **ejecutada de extremo a extremo**. La 0.6.0 se entregó sin que
la suite llegara a correr; esta corrige los fallos que lo impedían, multiplica
la cobertura y rehace panel y documentación.

### Fixed

- Los validadores de Node resolvían la raíz con `new URL("..", import.meta.url).pathname`,
  que en Windows devuelve `/C:/…` y produce rutas `C:\C:\…`. Afectaba a
  `run-negative-tests`, `validate-evidence`, `cleanup-test-state` y
  `generate-file-manifest`.
- `cargo fmt --all -- --check` fallaba en los tres crates: formato aplicado.
- La prueba de API derivaba la raíz del repositorio de `process.cwd()`, que
  apunta a `control-center/` cuando corre vía `pnpm --dir` o `make`.
- La prueba de symlink abortaba en Windows sin privilegios; ahora se salta con
  motivo explícito.
- El watchdog del Control Center mataba el trabajo cuando `sandboxctl` se
  invocaba vía `cargo run`: la compilación consumía el timeout de la política.
  El arranque ya no se descuenta del tiempo de ejecución de la carga.
- `native` reportaba «runtime no disponible» en lugar del motivo accionable
  (falta el opt-in `SANDBOX_LABS_ALLOW_NATIVE`).
- El servidor escribía en consola los 4xx del cliente como errores del servidor.
- `pnpm/action-setup` declaraba `version: 9` mientras `package.json` ya fija
  `packageManager`; la acción abortaba y los jobs de Node no llegaban a correr.
- `with: { components: rustfmt, clippy }` es un mapa de flujo YAML: la acción
  descartaba clippy en silencio.
- El panel desbordaba horizontalmente en móvil por el ancho intrínseco de los
  `<select>` con opciones largas.

### Added

- **16 pruebas de contrato del repositorio** en Rust (`tests/repository.rs`):
  catálogo contra `labs/`, carga y validación de todas las políticas y cargas,
  cargas de riesgo sin `allowNative`, fail-closed de políticas estrictas,
  particionado de controles, determinismo de hashes y rutas portables.
- **Pruebas del Control Center** ampliadas de 8 a 16: catálogo sin rutas del
  host, referencias no registradas, argumentos inválidos, anti DNS-rebinding
  con cliente HTTP crudo, traversals que sobreviven a la normalización de URL y
  cabeceras de seguridad.
- Previsión de controles en el panel: antes de crear el trabajo anuncia qué
  controles quedarán efectivos y si la política bloqueará.
- Estado en vivo por **SSE** en la interfaz (antes solo sondeaba cada 3 s).
- Portada estática en `site/` para GitHub Pages.
- Documentación nueva: `FAQ.md`, `GLOSSARY.md`, `SUPPORT.md`,
  `COMPATIBILITY.md`, `ENVIRONMENT_SETUP.md`, `FILE_ARCHITECTURE.md`,
  `OPERATING-MODES.md`, `CODE_OF_CONDUCT.md`, `docs/TROUBLESHOOTING.md` y
  `docs/DOCUMENTATION_INDEX.md`.
- `.gitattributes` que fija LF en todo lo ejecutable: un `.sh` con CRLF rompe CI.
- `.markdownlint.json` y `version.txt`.
- CI verifica que la evidencia generada por el CLI cumple su esquema.
- `security.yml` añade escaneo de secretos con gitleaks sobre el historial y
  auditoría de los propios workflows con zizmor.

### Changed

- Panel rediseñado con el lenguaje visual de los repositorios hermanos
  (`shell`, `hero`, `eyebrow`, `metrics`, `section-head`, `card-grid`, `logs`),
  con soporte de esquema claro y oscuro, enlace de salto, foco visible y
  respeto a `prefers-reduced-motion`.
- `README.md` reescrito: badges, tablas, diagramas Mermaid y rutas de lectura.
- `VALIDATION.md` reescrito para reflejar lo que se ejecutó de verdad, con una
  sección explícita de lo que queda fuera del alcance.
- `Cargo.lock` versionado; CI ya no lo regenera en cada ejecución, de modo que
  `--locked` vuelve a significar algo.
- `check-doc-links.mjs` y `generate-file-manifest.mjs` reescritos legibles; el
  primero ahora reporta todos los enlaces rotos, no solo el primero.
- `actions/checkout` v4 → v5.

---

## [0.6.0] - 2026-08-05

### Added

- Workspace Rust modular con dependencias declaradas y generación reproducible de `Cargo.lock` en CI.
- RuntimeAdapter y adaptadores dry-run, native, Bubblewrap, unshare, WASI y avanzados fail-closed.
- Policies strict/best-effort y controles requested/effective/unsupported.
- Evidencia con hashes SHA-256, host, runtime, límites y resultados.
- API de trabajos, cancelación, SSE y fallback de planificación.
- Esquemas de workload y job request.
- Pruebas negativas, validación profunda de evidencias y seguridad de archivos estáticos.
- Protección anti DNS-rebinding, cancelación con escalamiento a SIGKILL y logs visibles en el panel.
- Handoff específico para Codex.

### Changed

- Estados normalizados a ready, experimental, documented, manual y planned.
- Build del Control Center corregido a `dist/server.js`.
- CI genera `Cargo.lock`, ejecuta Cargo con `--locked` y usa instalación pnpm congelada.
