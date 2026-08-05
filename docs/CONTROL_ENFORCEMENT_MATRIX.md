# Matriz de aplicación de controles

| Runtime | Estado | Filesystem | Red | Procesos/RAM | Syscalls | Evidencia |
|---|---|---|---|---|---|---|
| dry-run | ready | planificado | planificado | planificado | planificado | real |
| native | experimental | no aislado | no aislada | `prlimit`/timeout cuando aplique | no | real |
| bwrap | experimental | namespaces + binds | namespace sin red | `prlimit` provisional | pendiente seccomp compilado | real |
| unshare | experimental | incompleto | namespace | PID namespace | pendiente | real |
| WASI | experimental | capacidades/preopens | no heredada | timeout; memoria pendiente de versión | N/A | real |
| gVisor | documented | OCI | OCI | OCI/cgroups | Sentry | pendiente integración |
| Kata | manual | VM | VM | VM/cgroups | kernel invitado | pendiente integración |
| Firecracker | manual | rootfs VM | tap/namespace | microVM | seccomp+jailer | pendiente integración |

`strict` rechaza una ejecución cuando falta un control requerido. `best-effort` puede ejecutar, pero la evidencia mantiene la lista `unsupported`.
