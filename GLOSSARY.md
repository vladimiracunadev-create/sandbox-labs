# 📖 Glosario

Términos que aparecen en el catálogo, las políticas y la evidencia. Ordenado
por concepto, no alfabéticamente: leerlo de arriba abajo explica el sistema.

---

## 🧱 Conceptos del proyecto

### Carga (workload)

Un directorio con `manifest.json` que declara qué se ejecuta, con qué comando y
qué se espera que pase. **Es lo único ejecutable**: no existe forma de pasarle
un comando arbitrario al sistema. Cada carga tiene un `risk`:

| `risk` | Significado |
|---|---|
| `benign` | Segura para el baseline |
| `resource-abuse` | Presiona memoria o procesos a propósito |
| `adversarial-simulation` | Simula un intento de fuga |

Las dos últimas nunca pueden declarar `allowNative`.

### Política (policy)

Un perfil neutral de aislamiento: filesystem, red, recursos, proceso, syscalls
y dispositivos. No menciona ningún runtime concreto — es el compilador de plan
quien la traduce.

### Control

Una capacidad de contención concreta: `filesystem`, `network`, `processes`,
`memory`, `cpu`, `timeout`, `capabilities`, `syscalls`, `devices`,
`environment`, `output`.

### Controles solicitados / efectivos / no soportados

La distinción central del proyecto.

| Término | Qué es |
|---|---|
| **Solicitados** | Lo que la política exige (`requiredControls`) |
| **Efectivos** | Lo que el runtime aplica de verdad en este host |
| **No soportados** | Lo exigido que el runtime **no** puede aplicar |

Un control jamás se marca como efectivo solo porque fue pedido. Esa es la
diferencia entre documentar aislamiento y tenerlo.

### Modo de aplicación (enforcement)

| Modo | Comportamiento ante un control no soportado |
|---|---|
| `strict` | **Falla cerrado**: no ejecuta y explica qué falta |
| `best-effort` | Ejecuta degradado y lo registra en la evidencia |

### Plan de ejecución

El resultado de cruzar carga + política + runtime **antes** de ejecutar:
controles efectivos, no soportados, límites, y si es ejecutable o no.

### Evidencia

El JSON que queda en `evidence/runs/` tras cada intento — ejecutado o
bloqueado. Incluye hashes SHA-256 de política, carga y binario, datos del host,
límites solicitados y efectivos, resultado y plan. Ver
[docs/EVIDENCE_FORMAT.md](docs/EVIDENCE_FORMAT.md).

### Fail-closed

Ante la duda, no ejecutar. Si una política estricta exige un control que el
runtime no aplica, el trabajo se bloquea. Lo contrario —ejecutar igualmente y
avisar— sería *fail-open*.

---

## 🐧 Aislamiento en Linux

### Namespace

Vista aislada de un recurso del kernel (PID, red, montajes, usuarios, IPC,
UTS, cgroup). Es el ladrillo de todos los contenedores.

### Rootless

Aislamiento sin privilegios de root, apoyado en *user namespaces*. `bwrap` y
`unshare` funcionan así en este proyecto.

### cgroups v2

Jerarquía del kernel que limita CPU, memoria, procesos y E/S. Es la vía
correcta para límites de recursos; `prlimit` solo cubre parte del terreno.

### seccomp

Filtro de llamadas al sistema. Reduce la superficie del kernel expuesta al
proceso. Ver [`profiles/seccomp/strict.json`](profiles/seccomp/strict.json).

### Capabilities

Trozos del privilegio de root concedidos por separado (`CAP_NET_ADMIN`,
`CAP_SYS_ADMIN`…). Una política sana los elimina todos.

### Landlock

LSM sin privilegios que restringe el acceso al filesystem desde el propio
proceso. Cubierto en el laboratorio 08.

### `bubblewrap` (`bwrap`)

Herramienta rootless para montar un filesystem nuevo y entrar en namespaces.
Es la base de Flatpak.

### `unshare`

Utilidad de `util-linux` que crea namespaces. Más simple que `bwrap` y sin jail
completo de filesystem.

### `prlimit`

Aplica `rlimits` (memoria, procesos, archivos) a un proceso. Complemento —no
sustituto— de cgroups.

---

## 📦 Runtimes y fronteras

### gVisor (`runsc`)

Kernel en espacio de usuario que intercepta las syscalls del invitado. Reduce
la superficie del kernel real a costa de compatibilidad y rendimiento.

### Kata Containers

Contenedores respaldados por una máquina virtual ligera: frontera de hardware
con interfaz de contenedor.

### Firecracker

MicroVM minimalista sobre KVM, pensada para multi-tenancy. Requiere jailer,
kernel y rootfs propios.

### WASI

Interfaz de sistema para WebAssembly. El aislamiento es por capacidades: el
módulo solo ve lo que se le concede (`--dir` / preopens).

> [!IMPORTANT]
> Contenedor, namespace, sandbox WASI y microVM **no son fronteras
> equivalentes**. Tratarlas como intercambiables es el error que este
> repositorio existe para evitar.

---

## 🧭 Panel y API

### Control Center

El panel local en `127.0.0.1:9093`. Orquesta trabajos; **no** es la frontera de
aislamiento.

### Trabajo (job)

Una petición de ejecución con identificadores del catálogo. Estados:
`queued` → `running` → `completed` / `failed` / `blocked` / `cancelled` /
`planned` / `timeout`.

### SSE (Server-Sent Events)

Canal unidireccional por el que el panel recibe el estado del trabajo en vivo,
en `/api/jobs/:id/events`.

### DNS rebinding

Ataque en el que una web externa resuelve un dominio a `127.0.0.1` para hablar
con un servicio local. Se bloquea validando la cabecera `Host`.

### Evidencia de reserva (fallback)

Cuando `sandboxctl` no está compilado, el panel genera una evidencia con estado
`planned` o `blocked` en vez de fingir una ejecución.

---

## 🔗 Ver también

- [Referencia de políticas](docs/POLICY_REFERENCE.md)
- [Matriz de controles](docs/CONTROL_ENFORCEMENT_MATRIX.md)
- [Modelo de amenazas](docs/THREAT_MODEL.md)
