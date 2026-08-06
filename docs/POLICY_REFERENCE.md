# Referencia de políticas

## Enforcement

- `strict`: falla cerrado si falta cualquier `requiredControl`.
- `best-effort`: ejecuta y registra controles no soportados.

Controles válidos: `filesystem`, `network`, `processes`, `memory`, `cpu`, `timeout`, `capabilities`, `syscalls`, `devices`, `environment`, `output`.

## Filesystem

- `root`: `ephemeral`, `host-readonly` o `custom`.
- `readOnly`: rutas esperadas como solo lectura.
- `writable`: rutas con escritura explícita.
- `maxWorkspaceMb`: cuota lógica; requiere implementación del runtime para considerarse efectiva.
- `followSymlinks`: debe permanecer `false` para cargas no confiables.

## Network

Solo dos de los cuatro modos crean un namespace de red propio. Esa es la línea
que decide si el control `network` puede declararse efectivo:

| `mode` | Namespace propio | Salida al exterior | Puerto TCP publicable | Control `network` |
|---|---|---|---|---|
| `none` | sí | no | no | efectivo |
| `loopback` | sí | no | no — solo socket Unix | efectivo |
| `allowlist` | **no** | **sí, sin filtrar** | sí | **nunca efectivo** |
| `unrestricted` | no | sí | sí | nunca efectivo |

- `none`: sin interfaz de red utilizable hacia fuera.
- `loopback`: la carga habla consigo misma dentro de su propio namespace. Un
  servicio con este modo **no puede enlazar él mismo un puerto del host**: el
  que abra nace dentro del sandbox. Sí puede publicarlo el supervisor por él
  —ver `publish: proxy` más abajo—, y si no hace ninguna de las dos cosas,
  `sandboxctl service up` falla en cerrado explicando ambas salidas.
- `allowlist`: hoy **no hay nada que haga cumplir la lista**. No existe proxy de
  salida, ni reglas de firewall, ni resolución DNS controlada. `hosts` se valida
  y después se ignora, así que el control nunca sale efectivo y una política
  estricta que lo exija no ejecuta. Ver
  [B-04](IMPLEMENTATION_BACKLOG.md) — es un hueco conocido, no un modo listo.
- `unrestricted`: la red del host, escrito con todas sus letras. Ninguna
  política que lo use puede exigir el control `network`, porque ahí no hay
  ninguno que exigir.

### Un servicio puede contener la red y seguir publicando un puerto

Parecen incompatibles y no lo son. La clave es separar **cómo escucha el
servicio** (`transport`, en su manifiesto) de **cómo llega el host**
(`publish`):

| `publish` | Quién enlaza el puerto del host | `network` puede ser |
|---|---|---|
| `direct` | el propio servicio | `allowlist` o `unrestricted` |
| `proxy` | el supervisor, empalmando con el socket Unix del sandbox | `none` o `loopback` |
| `none` | nadie: la única puerta es el socket | `none` o `loopback` |

Con `proxy`, `sandboxctl service up` levanta un reenviador que escucha en
`127.0.0.1:<puerto>` del host y empalma cada conexión con el socket del sandbox.
El servicio sigue hablando HTTP y se abre en el navegador igual, pero corre en
un namespace de red propio y no tiene por dónde salir.

Es lo que usan `service-isolated` y los casos `02` y `03`. `service-sandbox` y
`web-application` se quedan en `unrestricted` para los servicios que enlacen el
puerto ellos mismos.

## Recursos

`cpu`, `memoryMb`, `processes`, `timeoutSeconds`, `openFiles` y `outputBytes`.

El timeout y el límite de salida los aplica el supervisor común.

`memoryMb`, `processes` y `cpu` los aplica **cgroups v2**, y solo en bubblewrap.
El mecanismo es `systemd-run --user --scope`, que envuelve el árbol entero
—scope → `prlimit` → `bwrap` → carga— y traduce la política así:

| Campo de la política | Propiedad del scope | Fichero del kernel |
|---|---|---|
| `memoryMb` | `MemoryMax=<n>M` | `memory.max` |
| `processes` | `TasksMax=<n>` | `pids.max` |
| `cpu` | `CPUQuota=<n×100>%` | `cpu.max` |

Nada de eso se declara por estar escrito aquí. Antes de la primera ejecución el
sistema **levanta un scope real** con los tres límites puestos y comprueba que
el kernel los acepta; si falla, los controles `memory`, `processes` y `cpu` no
aparecen en `effectiveControls` y una política estricta que los exija no
ejecuta. `sandboxctl doctor` muestra el resultado de ese sondeo.

Dónde suele fallar: hosts sin gestor de usuario de systemd (contenedores, CI,
sesiones no interactivas). Ahí `systemd-run --user` no encuentra el bus.

`prlimit` sigue puesto como defensa adicional —`RLIMIT_AS` y `RLIMIT_NOFILE`—
pero **no** sustituye a cgroups: acota el espacio de direcciones virtual, no la
memoria residente. Cuando los dos están, la evidencia nombra el cgroup, que es
el que manda. `RLIMIT_NPROC` no se usa nunca: cuenta los procesos del UID real
en todo el host, no los de la carga.

## Proceso

- `capabilities`: lista de capabilities a conservar. Vacía en todas las
  políticas del catálogo, y bubblewrap aplica `--cap-drop ALL` tanto en cargas
  breves como en servicios.
- `user` y `group`: la identidad **dentro** del sandbox. Bubblewrap los aplica
  con `--uid`/`--gid`, que exigen user namespace. El mapeo es «uid de dentro →
  uid real», así que los montajes de escritura siguen siendo accesibles aunque
  el número cambie.

  No es cosmético. Sin ellos la carga corre con el uid de quien la lanzó y
  hereda sus grupos suplementarios — es decir, con la identidad que tiene acceso
  al repositorio, al llavero y a la sesión:

  | | uid dentro | grupos |
  |---|---|---|
  | sin `--uid` | `1000` (el tuyo) | `1000`, `65534` |
  | con `--uid 65534` | `65534` (`nobody`) | `65534` |

  `unshare` **no** los aplica: usa `--map-root-user`, que es otro mecanismo. Es
  una de las razones por las que sigue clasificado como runtime parcial.
- `environment`: variables que sí entran. El resto no: bubblewrap limpia el
  entorno con `--clearenv` antes de fijar estas.
- `allowedEnvironment`: nombres de secretos que la política autoriza. Un secreto
  que el servicio pide y la política no declara **no entra**, y eso no es un
  fallo sino la política haciendo su trabajo.
