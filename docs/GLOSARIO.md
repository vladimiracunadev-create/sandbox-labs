# 📖 Glosario

Ordenado por concepto, no alfabéticamente: leerlo de arriba abajo explica el
sistema entero.

---

## 🧱 Del proyecto

### Caso

Un **producto en su propio `localhost`** que se levanta, admite tareas dentro y
se apaga. No es un tema que se explica. Cada caso declara la `idea` que enseña,
y una prueba de contrato falla si dos casos comparten idea o puerto.

### Política

Un archivo aparte del código que declara **qué puede tocar** un caso:
filesystem, red, recursos, privilegios y entorno. Ni el programa negocia sus
permisos ni quien lo escribió decide sus límites.

### Control

Una capacidad de contención concreta: `filesystem`, `network`, `processes`,
`memory`, `cpu`, `timeout`, `capabilities`, `syscalls`, `devices`,
`environment`, `output`.

### Solicitado · efectivo · no soportado

La distinción central del proyecto.

| Término | Qué es |
|---|---|
| **Solicitado** | Lo que la política exige |
| **Efectivo** | Lo que el runtime aplica de verdad **en este host** |
| **No soportado** | Lo exigido que el runtime **no** puede aplicar aquí |

Un control jamás se marca como efectivo solo porque fue pedido. Esa es la
diferencia entre documentar aislamiento y tenerlo.

### Fail-closed

Ante la duda, no ejecutar. Si una política `strict` exige un control que el
runtime no aplica, el trabajo se bloquea. Lo contrario —ejecutar igual y
avisar— sería *fail-open*.

### Evidencia

El JSON que queda tras cada intento, ejecutado o bloqueado, con hashes SHA-256
de política, carga y binario, datos del host, controles efectivos y resultado.

### Falsa garantía

El hallazgo más grave que puede producir la suite: el runtime **declara** un
control y la sonda demuestra que no lo aplica. Es peor que no declararlo, porque
invita a confiar.

---

## 🐧 Del kernel de Linux

### Namespace

Vista aislada de un recurso del kernel: PID, red, montajes, usuarios, IPC, UTS,
cgroup. Es el ladrillo de todos los contenedores y de todos los sandboxes.

> Un PID namespace **sin remontar `/proc`** no se nota: el proceso sigue viendo
> los PIDs del host. Es el fallo silencioso más común.

### Rootless

Aislamiento sin privilegios de root, apoyado en *user namespaces*. Ser `uid=0`
dentro de uno **no** es ser root fuera: ese cero está mapeado.

### cgroups v2

Jerarquía del kernel que limita CPU, memoria, procesos y E/S. Es la vía correcta
para límites de recursos, y la que usa bubblewrap en este proyecto:
`memory.max`, `pids.max` y `cpu.max`, puestos a través de un scope de
`systemd-run --user`.

> `RLIMIT_NPROC` **no** es un sustituto: cuenta los procesos del UID en todo el
> host, no los de tu carga. `RLIMIT_AS` tampoco lo es de memoria: acota el
> espacio de direcciones virtual, no la residente.

### Capabilities

Trozos del privilegio de root concedidos por separado (`CAP_SYS_ADMIN`,
`CAP_NET_RAW`…). Una política sana los elimina todos: `CapEff: 0000000000000000`.

### seccomp

Filtro de llamadas al sistema. Reduce la superficie del kernel expuesta al
proceso, que es por donde se explotan sus vulnerabilidades.

### Landlock

LSM sin privilegios con el que un proceso se restringe **a sí mismo** el acceso
al filesystem, sin necesitar root ni montar nada.

---

## 🔧 Herramientas

| Herramienta | Qué hace |
|---|---|
| **`bubblewrap`** (`bwrap`) | Monta un filesystem nuevo y entra en namespaces, sin privilegios. Es la base de Flatpak y el runtime que más contiene aquí |
| **`unshare`** | Crea namespaces. Más simple que `bwrap` y **sin jaula de filesystem** |
| **`prlimit`** | Aplica límites de recursos al proceso. Complemento, no sustituto, de cgroups |
| **`wasmtime`** | Ejecuta módulos WebAssembly con aislamiento por capacidades |

---

## 📦 Otras fronteras

| Frontera | Qué separa | Coste |
|---|---|---|
| **gVisor** (`runsc`) | Un kernel en espacio de usuario intercepta las syscalls | decenas de ms |
| **Kata Containers** | Contenedor respaldado por una VM ligera | cientos de ms |
| **Firecracker** | MicroVM minimalista sobre KVM | cientos de ms |
| **WASI** | Capacidades: el módulo solo ve lo que se le concede | microsegundos |

> [!IMPORTANT]
> Contenedor, namespace, sandbox WASI y microVM **no son fronteras
> equivalentes**. Tratarlas como intercambiables es el error que este
> repositorio existe para evitar. Ver [Comparativa](COMPARATIVA.md).

---

## 🧭 Del entorno raíz

### Entorno raíz

El panel en `127.0.0.1:9093` que levanta, apaga y vigila los casos. **No es la
frontera de aislamiento**: la frontera es el runtime efectivo.

### Servicio y transporte

Un caso levantado. Se alcanza por `tcp` (un puerto del loopback) o por
`unix-socket` — necesario cuando la política es `network: none` y no hay pila de
red por donde hablar.

### Modo plan y modo real

Un caso que necesita una credencial arranca igual sin ella: en **modo plan**
muestra la petición exacta que haría; con la variable presente pasa a **modo
real** y la ejecuta. Un secreto que la política no declara **no entra** al
sandbox aunque exista en el host.

---

## 🔗 Ver también

- [Qué es un sandbox](QUE-ES-UN-SANDBOX.md) · [Comparativa](COMPARATIVA.md)
- [Referencia de políticas](POLICY_REFERENCE.md) · [Suite de contención](CONTAINMENT_SUITE.md)
