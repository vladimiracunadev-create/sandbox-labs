# 🧱 Backlog técnico

> **Versión** 0.1.0 · Este documento existe para que ningún hueco del núcleo
> viva solo en un comentario del código.

La regla del proyecto es que **un control solicitado, un control aplicado y un
control reportado sean la misma cosa**. Cuando no lo son, el hueco se escribe
aquí y el control no se declara — nunca al revés.

Cada entrada dice qué falta, por qué importa, qué se hace hoy en su lugar y qué
haría falta para cerrarla.

---

## 🔴 Huecos que limitan el aislamiento

### B-01 · Techo real de PIDs con cgroups v2

| | |
|---|---|
| **Estado** | ✅ Cerrado en bubblewrap · no aplica a `unshare` |
| **Control afectado** | `processes` |
| **Se declara hoy** | Sí, en bubblewrap y **solo si el host lo admite**: el sondeo levanta un scope de systemd real antes de declarar nada. Donde no hay gestor de usuario, el control no se declara. |

`RLIMIT_NPROC` **no** sirve como sustituto: cuenta los procesos del UID real en
todo el host, no los de la carga. Fijarlo al presupuesto de la política mata la
ejecución nada más arrancar —el usuario ya tiene procesos abiertos— y, peor,
haría pasar por control de contención algo que no lo es.

El control real es `pids.max` del controlador `pids` de cgroups v2. Requiere un
cgroup delegado y escribible por el usuario que lanza el sandbox.

**Cómo se cerró:** `systemd-run --user --scope -p TasksMax=N` envuelve el árbol
entero (scope → `prlimit` → `bwrap` → carga). Escribir el cgroup a mano no vale
en la plataforma objetivo: en WSL2 el proceso arranca en `/init.scope`, que
existe y **no es escribible**, así que `mkdir` falla. Pedírselo al gestor de
usuario sí funciona y sin privilegios.

**Lo que queda:** `unshare` no lo recibe a propósito — `systemd-run` necesita
`XDG_RUNTIME_DIR` y `DBUS_SESSION_BUS_ADDRESS` en el entorno, y `unshare` se los
pasaría tal cual a la carga. Bubblewrap no, porque hace su propio `--clearenv`
después. `pids.peak` sí se observa: va en `limits.observed` de la evidencia.

### B-02 · Techo real de memoria con `memory.max`

| | |
|---|---|
| **Estado** | ✅ Cerrado |
| **Control afectado** | `memory` |
| **Se declara hoy** | Sí. Con cgroup disponible la evidencia dice «cgroup memory.max»; sin él, «RLIMIT_AS», que es lo que de verdad se aplicó. |

`RLIMIT_AS` acota el **espacio de direcciones virtual**, que no es lo mismo que
la memoria residente. Un proceso puede quedarse muy por debajo del límite y aun
así presionar la RAM del host, o al revés: fallar por reservar mucho espacio
virtual que nunca toca.

**Aplicado:** `MemoryMax` en el scope, que systemd traduce a `memory.max`.

**Observado:** `limits.observed` de la evidencia lleva `memoryPeakBytes` y
`oomKills`, leídos de `memory.peak` y de `memory.events` **mientras** la carga
corre — systemd retira el cgroup en cuanto el scope termina, así que leer al
final no encuentra nada. Con `oomKills` un código de salida inexplicable pasa a
ser un hecho.

Una prueba envuelve un proceso real, le hace reservar 40 MB y comprueba que el
pico observado los refleja. Se salta donde no hay gestor de usuario de systemd,
que es la misma condición bajo la que el control no se declara.

### B-03 · Cuota de CPU

| | |
|---|---|
| **Estado** | ✅ Cerrado |
| **Control afectado** | `cpu` |
| **Se declara hoy** | Sí, en bubblewrap y solo con cgroup disponible. |

**Aplicado:** `CPUQuota=N%` en el scope —el porcentaje va sobre **un** núcleo,
así que `cpu: 2.0` son 200%— que systemd traduce a `cpu.max`.

**Observado:** `cpuUsageUsec`, de `usage_usec` en `cpu.stat`.

### B-04 · `network: allowlist` sin enforcement

| | |
|---|---|
| **Estado** | Abierto |
| **Control afectado** | `network` |
| **Se declara hoy** | No. Con `allowlist` el control `network` queda fuera de `effectiveControls`, y una política estricta que lo exija **no ejecuta**. |

`policy.network.hosts` se valida sintácticamente y después se ignora: no hay
proxy de salida, ni reglas de firewall, ni resolución DNS controlada. Una lista
de hosts sin nada que la haga cumplir es peor que no tenerla, porque invita a
confiar.

**Para cerrarlo:** namespace de red propio más un proxy supervisor en el host
que solo abra los destinos de la lista y registre cada conexión. Sin registro de
conexiones no hay control, solo intención.

### B-04b · Un servicio en namespace de red propio no puede publicar un puerto

| | |
|---|---|
| **Estado** | ✅ Cerrado |
| **Control afectado** | `network`, en los servicios |
| **Se declara hoy** | Sí. Los casos `02` y `03` corren con `service-isolated`, que exige el control `network`. |

`loopback` estaba implementado —crea namespace propio— pero un servicio con
`transport: tcp` no podía usarlo: su puerto nacía dentro del sandbox y nadie
fuera lo alcanzaba. Eso dejaba a los dos casos con la red del host.

**Cómo se cerró:** separando *cómo escucha el servicio* de *cómo llega el host*.
El manifiesto gana el campo `publish`:

| `publish` | Quién enlaza el puerto | Red del sandbox |
|---|---|---|
| `direct` | el propio servicio | tiene que ser la del host |
| `proxy` | el supervisor, empalmando con el socket | namespace propio |
| `none` | nadie: solo el socket | namespace propio |

Con `proxy`, `sandboxctl service up` levanta un reenviador —un proceso aparte,
registrado en `proxyPid` y bajado junto al sandbox— que escucha en el loopback
del host y empalma cada conexión con el socket Unix del sandbox. El servicio
sigue hablando HTTP; solo cambia el transporte por debajo.

Medido levantando el caso `03` de verdad:

```text
✅ file-detonation responde en http://127.0.0.1:8803
   (reenviado a unix:/run/user/1000/sandbox-labs/file-detonation.sock)
   contención efectiva: environment, memory, network, output, timeout

curl http://127.0.0.1:8803/health  →  http=200
netns del sandbox : net:[4026532244]
netns del host    : net:[4026531833]   ← distintos
```

**Lo que queda:** el caso `05` sigue con `publish: none` a propósito —una clave
privada no se publica— y ningún caso usa ya `direct`, pero el modo se conserva
porque un servicio que necesite la red del host tiene que poder decirlo.

### B-05 · Perfiles seccomp declarados y no aplicados

| | |
|---|---|
| **Estado** | ✅ Cerrado en bubblewrap · no aplica a `unshare` |
| **Control afectado** | `syscalls` |
| **Se declara hoy** | Sí, en bubblewrap y solo cuando la política deniega llamadas que este kernel conoce y el filtro compila para esta arquitectura. |

`profiles/seccomp/strict.json` existía, `policy.syscalls` se parseaba, y ningún
adaptador compilaba nada ni se lo pasaba al runtime. Un fichero de perfil que
nadie aplica sugiere una capacidad que el sistema no tiene.

**Cómo se cerró:** `policy.syscalls.deny` se compila a un programa BPF con
[`seccompiler`](https://crates.io/crates/seccompiler) —Rust puro, frente a
`libseccomp`, que exigiría la biblioteca C en cada host y en CI— y se le pasa a
bubblewrap por descriptor con `--seccomp 63`. El descriptor se duplica en el
hijo con `dup2` entre el `fork` y el `exec`, porque Rust marca `CLOEXEC` en todo
lo que abre y no hay API segura en `std` para heredar uno concreto.

**Denegación, no lista de permitidos.** Es lo que declaran las políticas del
catálogo. Una lista de permitidos contiene mucho más, pero obliga a enumerar
todo lo que un intérprete de Python necesita para arrancar, que cambia entre
versiones de glibc y de Python. Una lista de permitidos incompleta no es «más
segura»: es un sandbox que no arranca, y el arreglo habitual es ampliarla hasta
que funcione, momento en el cual ya no contiene nada. Para un binario conocido y
estable será lo correcto; para código arbitrario, no.

**`EPERM`, no matar el proceso.** Matar deja un proceso muerto sin explicación.
Con `EPERM` la carga recibe un error normal, sigue viva y puede contarlo — que es
lo que permite medirlo.

**Y se mide, con una llamada de calibración.** La sonda `seccomp-filter` ejecuta
`getcpu(NULL, NULL, NULL)`, que **tiene éxito siempre, para cualquiera y en
cualquier host**. Así solo hay dos respuestas posibles y cada una significa una
cosa: éxito = ningún filtro la bloqueó, `EPERM` = el filtro la denegó. Por eso
`containment-audit` —la política cuya única razón de ser es medir— la incluye en
su lista de denegación.

Se llegó ahí después de dos intentos fallidos, y los dos enseñan lo mismo:

1. Medir con `mount` o `ptrace`, que es el instinto. No sirve: ya fallan con
   `EPERM` para cualquier usuario sin privilegios, así que la sonda aprobaría con
   filtro y sin él. Mediría el privilegio del usuario, no el sandbox.
2. Medir con `perf_event_open(NULL, …)`, que devuelve `EFAULT` en una máquina
   normal. **Falló en CI**: el runner devuelve `EACCES` por
   `perf_event_paranoid`, y en un host con ese sysctl en 3 devolvería `EPERM` sin
   que hubiera filtro alguno. Un discriminador que depende de la configuración
   del host no discrimina.

Medido con bubblewrap 0.9.0, las tres filas:

```text
sin sandbox            → escaped   (getcpu tuvo éxito)
bubblewrap sin filtro  → escaped   (bubblewrap por sí solo no filtra)
bubblewrap con filtro  → contained (getcpu → EPERM, perf_event_open → EPERM)
```

La fila del medio es la que da valor a la de abajo: sin ella, «contenido» podría
significar solo «bubblewrap estaba puesto».

La sonda tampoco llama a `ptrace`. Hacerlo con `request=0` es `PTRACE_TRACEME`,
que tiene éxito y deja el proceso **detenido** esperando a su padre: sin filtro
la sonda se colgaba y no llegaba a imprimir su veredicto. Una sonda que se cuelga
justo en el caso que tiene que detectar es peor que no tenerla.

Una prueba unitaria aplica además el BPF a un hilo real y comprueba el mismo
salto, así que la compilación se verifica aunque no haya bubblewrap instalado. Y
otra comprueba el paso del descriptor —`dup2` entre el fork y el exec— usando
`/proc/self/fd`, porque `/bin/sh` es dash y no admite `<&63`.

`syscalls` entra en el contrato de contención que CI exige a bubblewrap.

**No aplica a `unshare`:** no tiene forma de recibir un filtro. Una razón más
por la que sigue clasificado como runtime parcial.

### B-06 · UID/GID de la política ignorados

| | |
|---|---|
| **Estado** | ✅ Cerrado en bubblewrap · no aplica a `unshare` |
| **Control afectado** | `capabilities` |
| **Se declara hoy** | Sí. La evidencia lleva `user: uid=… gid=… (--uid/--gid)` en los límites efectivos. |

`policy.process.user` y `policy.process.group` valen 65534 en todas las
políticas y bubblewrap nunca los recibía. Lo que de verdad pasaba era peor de lo
que decía este documento antes: la carga no corría como el root mapeado del user
namespace sino **con el uid real de quien la lanzó**, y heredaba sus grupos
suplementarios. Medido con bubblewrap 0.9.0:

```text
sin --uid : uid=1000(vbav) gid=1000(vbav) groups=1000(vbav),65534(nogroup)
con --uid : uid=65534(nobody) gid=65534(nogroup) groups=65534(nogroup)
```

**Cómo se cerró:** `--uid`/`--gid` de la política, tanto en las cargas breves
como en los servicios. Estos últimos ganan además el `--cap-drop ALL` que ya
tenían las cargas y a ellos les faltaba, aunque su política exigía el control
`capabilities`. Comprobado levantando el caso `03` dentro de bubblewrap: socket
creado, `HTTP 200` por él, y `uid=65534(nobody)` dentro.

**Por qué no hay sonda que lo verifique.** Se intentó y se retiró: desde dentro
de un user namespace de un solo uid **no se puede** distinguir «soy el usuario
que lanzó esto» de «soy un id distinto mapeado a él». El kernel reetiqueta todo
de forma coherente —`/proc/self/uid_map` leído desde dentro da `1000 0 1` tanto
antes como después—, así que la sonda reportaba `contained` en los dos casos.
Una sonda que aprueba pase lo que pase es precisamente la falsa garantía que
este proyecto persigue.

Lo que sí queda registrado: el `uid` efectivo aparece en el detalle de la sonda
`privilege-caps` de cada informe, y la identidad pedida en los límites efectivos
de la evidencia. Para convertirlo en una comprobación automática haría falta que
la sonda conociera el valor esperado, es decir, que la política se lo inyecte
por entorno. Está sin hacer.

**No aplica a `unshare`:** usa `--map-root-user`, que es otro mecanismo y da
uid 0 dentro del namespace. Es una de las razones por las que sigue clasificado
como runtime parcial.

---

## 🟠 Huecos de arquitectura

### B-07 · Dos compiladores de política

| | |
|---|---|
| **Estado** | ✅ Cerrado |

Las cargas que terminan se compilaban en el adaptador de bubblewrap y los
servicios de larga duración en el lanzador del CLI, cada uno con su lista de
argumentos escrita a mano. Dos caminos hacia el mismo kernel son dos sitios
donde un control puede perderse, y solo uno estaba cubierto por la suite de
contención.

**Cómo se cerró:** `sandbox_core::compiler` produce los argumentos de
bubblewrap para los dos. Fuera queda solo lo que de verdad distingue una
ejecución de otra —qué se monta, dónde se trabaja, qué variables extra y qué se
ejecuta—; todo lo que viene de la política se decide en un único sitio.

**Lo que estaba perdido.** No era teórico. Al camino de los servicios le
faltaban `--cap-drop ALL` —aunque su política exige el control `capabilities`—,
`--uid`/`--gid`, `--new-session` (lo que impide inyectar en el terminal con
`TIOCSTI`), `--unshare-cgroup-try`, el filtro seccomp y los límites de cgroups.

**Y una falsa garantía.** El registro del servicio copiaba
`runtime.supported_controls()`, que describe lo que bubblewrap puede aplicar a
una carga. Así llegó a declarar `memory`, `processes` y `cpu` sin que nadie los
aplicara: la tarjeta del panel prometía tres controles inexistentes. Ahora los
servicios reciben el mismo scope de cgroups, y lo que se registra es lo que ese
camino aplicó.

**Un bug que solo se veía con bubblewrap.** Los servicios llevaban
`--die-with-parent`, que mata el sandbox cuando muere quien lo lanzó — y
`sandboxctl service up` termina en cuanto informa. El servicio arrancaba, decía
que estaba listo y desaparecía. No se había visto porque los servicios se
probaban con `unshare`, que no tiene esa opción. Ahora es un campo explícito de
la petición: sí para una carga supervisada, no para un servicio.

Medido levantando el caso `03` con bubblewrap, mirando el proceso de la carga y
no el envoltorio:

```text
netns   : net:[4026532380]   (host: net:[4026531833])
userns  : user:[4026532439]  (host: user:[4026531837])
Seccomp : 2                  (filtro cargado)
CapEff  : 0000000000000000
memory.max=536870912  pids.max=32  cpu.max=100000 100000
curl http://127.0.0.1:8803/health → 200
tras `service down` → sin respuesta
```

**Lo que queda:** `unshare` conserva su rama propia. No recibe cgroups —
`systemd-run` necesita las variables del bus en el entorno y `unshare` se las
pasaría tal cual a la carga— ni filtro seccomp, y su lista de controles
efectivos lo dice.

### B-08 · Evidencia sin cadena de integridad

| | |
|---|---|
| **Estado** | 🟡 Verificable; sin firmar |

La evidencia lleva `schemaVersion`, los hashes de política, carga y binario, la
partición de controles solicitados/efectivos/no soportados, y ahora los límites
en tres bloques: pedido, aplicado y consumido.

**Cerrado:** cada evidencia se sella con `integrity.evidenceSha256`, un SHA-256
de su propio contenido calculado con ese campo vacío. `sandboxctl evidence
verify` lo recalcula y además vuelve a hashear la política y la carga que la
evidencia dice haber ejecutado. Comprobado contra manipulación real:

```text
tal cual                          → ✅ exit 0
status Planned → Completed        → ✗ huella propia, exit 1
effectiveControls con uno de más  → ✗ huella propia, exit 1
la carga cambia en el repositorio → ✗ carga sin cambios, exit 1
```

El último caso distingue lo que importa: la huella sigue bien —nadie tocó el
informe— pero el código que describe ya no es el que hay. Un informe de hace
tres semanas tiene que poder contar eso.

Corre como paso de CI, así que una evidencia que no se verifique tumba el build.

**Lo que queda:**

- **No es una firma.** Quien pueda editar el fichero puede recalcular la huella.
  Detecta alteración accidental o descuidada —un campo tocado a mano, una copia
  truncada, un informe recortado antes de adjuntarlo—, que es el caso que se da
  en la práctica. Una firma Ed25519 con clave local sigue pendiente.
- **Sin encadenado entre eventos ni manifiesto de artefactos.** Los campos
  `violations`, y los eventos de filesystem, red y seguridad, todavía no se
  rellenan en las ejecuciones normales: solo la suite de contención observa y
  reporta. Faltan también el bloque `cleanup` y un `verdict` explícito.

---

## 🟡 Casos del catálogo

| Caso | Estado real | Hueco |
|---|---|---|
| `01-untrusted-render` | `planned` | Sin implementar. |
| `02-ai-code-runner` | `building` | Ejecuta el fragmento **dentro** del servicio persistente. Debería crear un sandbox efímero por ejecución y destruirlo. |
| `03-file-detonation` | `building` | Lo que hace es extracción segura de archivos, no detonación. El nombre promete una microVM instrumentada que no existe. |
| `04-third-party-plugins` | `planned` | Sin implementar. |
| `05-smart-contracts` | `building` | Lo que hace es custodia de claves y firma. La ejecución determinista con presupuesto de instrucciones es otro caso. |

---

## 🔗 Relacionado

- [Suite de contención](CONTAINMENT_SUITE.md) — cómo se mide lo que sí está
- [Formato de evidencia](EVIDENCE_FORMAT.md) — qué queda escrito hoy
- [Modelo de amenazas](THREAT_MODEL.md) — qué protege y qué explícitamente no
- [Roadmap](../ROADMAP.md) — el orden en que se cierran estos huecos
