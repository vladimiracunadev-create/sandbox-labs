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
| **Estado** | ✅ Cerrado en bubblewrap · abierto en `unshare` |
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
después. Y falta leer `pids.peak`, que es B-02.

### B-02 · Techo real de memoria con `memory.max`

| | |
|---|---|
| **Estado** | 🟡 Aplicado; falta observar |
| **Control afectado** | `memory` |
| **Se declara hoy** | Sí. Con cgroup disponible la evidencia dice «cgroup memory.max»; sin él, «RLIMIT_AS», que es lo que de verdad se aplicó. |

`RLIMIT_AS` acota el **espacio de direcciones virtual**, que no es lo mismo que
la memoria residente. Un proceso puede quedarse muy por debajo del límite y aun
así presionar la RAM del host, o al revés: fallar por reservar mucho espacio
virtual que nunca toca.

**Aplicado:** `MemoryMax` en el scope, que systemd traduce a `memory.max`.

**Lo que queda:** observar. Aplicar un límite y medir el consumo son cosas
distintas. Leer `memory.peak` y el contador `oom_kill` de `memory.events` exige
muestrear el cgroup **mientras** la carga corre, porque systemd retira el cgroup
en cuanto el scope termina. Sin eso, un proceso matado por OOM deja un código de
salida sin explicar.

### B-03 · Cuota de CPU

| | |
|---|---|
| **Estado** | 🟡 Aplicado; falta observar |
| **Control afectado** | `cpu` |
| **Se declara hoy** | Sí, en bubblewrap y solo con cgroup disponible. |

**Aplicado:** `CPUQuota=N%` en el scope —el porcentaje va sobre **un** núcleo,
así que `cpu: 2.0` son 200%— que systemd traduce a `cpu.max`.

**Lo que queda:** leer `cpu.stat` al terminar, con el mismo problema de
muestreo que B-02.

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
| **Estado** | Abierto |
| **Control afectado** | `syscalls` |
| **Se declara hoy** | No, en ningún runtime. |

`profiles/seccomp/strict.json` existe y `policy.syscalls` se parsea, pero ningún
adaptador compila el perfil ni lo pasa al runtime. El fichero sugiere una
capacidad que el sistema no tiene.

**Para cerrarlo:** compilar el perfil a un programa BPF y pasarlo a bubblewrap
con `--seccomp`, registrando en la evidencia el perfil solicitado, el realmente
aplicado y el resultado de las sondas que intentan las llamadas bloqueadas.

### B-06 · UID/GID de la política ignorados

| | |
|---|---|
| **Estado** | Abierto |
| **Control afectado** | `capabilities` |
| **Se declara hoy** | Sí — bubblewrap sí aplica `--cap-drop ALL` y el user namespace, que es lo que el control nombra. |

`policy.process.user` y `policy.process.group` valen 65534 en todas las
políticas del catálogo, pero bubblewrap nunca recibe `--uid`/`--gid`: la carga
corre como el root mapeado del user namespace. Dentro del namespace ese root no
tiene capabilities sobre el host, así que el impacto es menor que su apariencia,
pero la política dice una cosa y la ejecución hace otra.

**Para cerrarlo:** pasar `--uid`/`--gid` y comprobarlo con una sonda que lea su
propio `id`.

---

## 🟠 Huecos de arquitectura

### B-07 · Dos compiladores de política

| | |
|---|---|
| **Estado** | Abierto |

Las cargas que terminan se planifican en `ExecutionPlan::build`
(`sandbox-core`). Los servicios de larga duración reconstruyen a mano una
versión propia de los argumentos del runtime en `sandboxctl/src/service.rs`. Dos
caminos hacia el mismo kernel significa dos sitios donde un control puede
perderse, y solo uno de ellos está cubierto por la suite de contención.

**Para cerrarlo:** un único compilador que produzca el plan, y dos supervisores
—uno que espera al proceso y otro que lo deja corriendo— sobre el mismo plan.

### B-08 · Evidencia sin cadena de integridad

| | |
|---|---|
| **Estado** | Abierto |

La evidencia ya lleva `schemaVersion`, los hashes de política, carga y binario,
y la partición de controles solicitados/efectivos/no soportados. Le faltan:
encadenado de hash entre eventos, manifiesto de artefactos, bloque de `cleanup`,
`verdict` explícito y un `sandboxctl evidence verify` que compruebe todo eso.

Los campos `violations`, `filesystemEvents`, `networkEvents` y `securityEvents`
todavía no se rellenan en las ejecuciones normales: solo la suite de contención
observa y reporta.

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
