# Project Status

## v0.6.0

### Consolidado

- Contratos de policy, workload, job, catalog y evidence.
- Modelos tipados con Serde y hashes SHA-256 de políticas, workloads y runner.
- CLI con doctor, catálogo, validación, planificación, ejecución y generación de evidencias.
- Supervisor con timeout, cancelación por proceso y truncado de salida.
- Control Center local sin comandos arbitrarios, con cancelación, SSE, logs y protección anti DNS-rebinding.
- Validación JSON Schema, pruebas de API y pruebas negativas declarativas.

### Experimental

- Bubblewrap, unshare, WASI y native baseline.
- Uso de `prlimit` como control complementario.

### Pendiente de validación real

- Compilación Rust en este entorno de entrega.
- cgroups v2, seccomp efectivo y métricas de recursos.
- gVisor, Kata y Firecracker.
- Persistencia completa y multi-tenancy.

No se debe cambiar un estado a `ready` sin evidencias y pruebas negativas.
