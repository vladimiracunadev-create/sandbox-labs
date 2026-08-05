# Adaptadores de runtime

## Matriz actual

| Control | Native | Bubblewrap | Unshare | WASI |
|---|---:|---:|---:|---:|
| Filesystem | No | Sí, experimental | No | Preopens |
| Network none | No | Namespace | Namespace | Sin sockets habilitados |
| Procesos | No | `prlimit`, si existe | No | N/A |
| Memoria | No | `RLIMIT_AS`, si existe | No | Pendiente configuración Wasmtime |
| Timeout | Sí | Sí | Sí | Sí |
| Capabilities | No | `--cap-drop ALL` | User namespace | Modelo WASI |
| Output | Sí | Sí | Sí | Sí |

## Regla de implementación

Un adaptador debe devolver `EffectivePolicy` basado en el comando realmente preparado. Los controles no verificables se mantienen en `unsupported`.

## Avanzados

- gVisor: construir bundle OCI efímero, ejecutar `runsc`, destruir bundle y registrar versión/configuración.
- Kata: integrar containerd/CRI con runtime handler explícito.
- Firecracker: usar `jailer`, UID/GID sin privilegios, cgroups, seccomp, kernel/rootfs firmados y red dedicada.
