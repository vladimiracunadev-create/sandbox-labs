# 📊 Estado del proyecto

> **Versión**: 0.7.0 · **Estado**: 🟢 activo
> El detalle de lo verificado está en [VALIDATION.md](VALIDATION.md).

---

## ✅ Consolidado

- Contratos de policy, workload, job, catalog y evidence con JSON Schema.
- Modelos tipados con Serde y hashes SHA-256 de política, carga y runner.
- CLI con `doctor`, `labs`, `runtimes`, `validate`, `plan` y `run`.
- Supervisor con timeout, cancelación por proceso y truncado de salida.
- Separación estricta entre controles solicitados, efectivos y no soportados.
- Fail-closed verificado: una política `strict` con huecos no ejecuta.
- Control Center local sin comandos arbitrarios, con cancelación, SSE, logs y
  protección anti DNS-rebinding.
- **Suite ejecutada de extremo a extremo**: 18 pruebas Rust, 15 del panel y 5
  validadores de catálogo, en verde en local y en CI.
- Panel verificado en navegador contra el servidor real, en claro y oscuro,
  con previsión de controles antes de ejecutar.
- `Cargo.lock` versionado y CI compilando con `--locked`.

## 🟡 Experimental

Los adaptadores existen, aplican controles y quedan registrados en la
evidencia — pero **no** están validados frente a un atacante.

- `bwrap`: filesystem, namespaces, red cerrada, capabilities y `prlimit`.
- `unshare`: namespaces y red cerrada, sin jail completo de filesystem.
- `wasi`: preopens de Wasmtime sobre módulos registrados.
- `native`: línea base sin aislamiento, con doble opt-in.

## 🚧 Pendiente de validación real

- Ejecución real con `bwrap` y `wasi` en un host que tenga esos binarios.
- cgroups v2 como control efectivo, con métricas de recursos.
- seccomp aplicado (hoy el perfil existe pero no se impone).
- gVisor, Kata y Firecracker: contrato escrito, integración sin construir.
- Persistencia completa y multi-tenancy.

---

## 📈 Cambios desde v0.6.0

La entrega 0.6.0 nunca llegó a ejecutarse: la suite estaba en rojo por rutas
rotas en Windows, formato Rust sin aplicar y pruebas que resolvían mal la raíz
del repositorio. La 0.7.0 corrige eso, multiplica la cobertura y rehace el panel
y la documentación. Detalle en [CHANGELOG.md](CHANGELOG.md).

---

## 🧭 Regla de estados

| Estado | Qué significa | Qué hace falta para subir |
|---|---|---|
| `planned` | Solo idea | Contrato escrito |
| `documented` | Contrato y backlog, no ejecuta | Integración construida |
| `manual` | Requiere preparación específica del host | Automatización reproducible |
| `experimental` | Ejecuta y aplica controles | Ejecución real **y** pruebas negativas |
| `ready` | Verificado con evidencia | — |

> [!IMPORTANT]
> Ningún adaptador cambia a `ready` sin evidencia de ejecución real y pruebas
> negativas que demuestren que el control bloquea lo que debe bloquear.

---

## 🔗 Ver también

- [Roadmap](ROADMAP.md) · [Backlog de implementación](docs/IMPLEMENTATION_BACKLOG.md)
