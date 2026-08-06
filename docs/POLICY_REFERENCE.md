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
  servicio con este modo **no puede publicar un puerto en el host**: el puerto
  nace dentro del sandbox. `sandboxctl service up` falla en cerrado y dice que
  el servicio tiene que pasar a `transport: unix-socket`.
- `allowlist`: hoy **no hay nada que haga cumplir la lista**. No existe proxy de
  salida, ni reglas de firewall, ni resolución DNS controlada. `hosts` se valida
  y después se ignora, así que el control nunca sale efectivo y una política
  estricta que lo exija no ejecuta. Ver
  [B-04](IMPLEMENTATION_BACKLOG.md) — es un hueco conocido, no un modo listo.
- `unrestricted`: la red del host, escrito con todas sus letras. Es lo que
  necesita cualquier servicio que publique un puerto, y por eso las políticas
  `service-sandbox` y `web-application` lo usan. Ninguna de las dos exige el
  control `network`, porque ahí no hay ninguno que exigir.

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
