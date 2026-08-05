# 🆘 Soporte

Este es un proyecto educativo y experimental mantenido en tiempo libre. No
lleva SLA — pero sí un camino claro para desatascarse.

---

## 🔎 Antes de abrir nada

Recorre esto en orden; cubre la mayoría de los casos:

1. **[FAQ.md](FAQ.md)** — dudas de concepto (`blocked`, `planned`, `native`…).
2. **[docs/TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md)** — síntoma → causa → arreglo.
3. **[RUNBOOK.md](RUNBOOK.md)** — operación diaria.
4. **[ENVIRONMENT_SETUP.md](ENVIRONMENT_SETUP.md)** — instalación paso a paso.

Y ejecuta el diagnóstico, que suele responder solo:

```bash
cargo run -p sandboxctl -- doctor
node scripts/validate-config.mjs
make check
```

---

## 🐛 Reportar un problema

Abre una *issue* con la
[plantilla de bug](https://github.com/vladimiracunadev-create/sandbox-labs/issues/new?template=bug.yml).

Incluye siempre:

| Dato | Cómo obtenerlo |
|---|---|
| Sistema y kernel | `uname -a` (o versión de Windows y de WSL) |
| Versión del proyecto | `cat version.txt` |
| Sondeo de runtimes | `cargo run -p sandboxctl -- doctor --json` |
| La evidencia del intento | `evidence/runs/<runId>.json` |
| Comando exacto | Copia y pega el que falló |

> [!TIP]
> La evidencia es el mejor reporte posible: lleva hashes, host, controles
> efectivos y el motivo del bloqueo. Adjúntala siempre que exista.

---

## 🔐 Vulnerabilidades

**No** abras una issue pública. Sigue [SECURITY.md](SECURITY.md).

---

## 💡 Proponer una mejora

Abre una issue describiendo el problema antes que la solución. Para runtimes
nuevos o cambios de estado, mira [CONTRIBUTING.md](CONTRIBUTING.md) — la regla
dura es que nada pasa a `ready` sin evidencia y pruebas negativas.

---

## ❌ Qué queda fuera del alcance

| Petición | Por qué no |
|---|---|
| «Hazlo seguro para ejecutar malware» | Ese no es el objetivo; ver [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md) |
| Endpoint para comandos arbitrarios | Convierte el panel en una shell remota |
| Exponer el panel en la red | No tiene autenticación ni multi-tenancy |
| Soporte de aislamiento en Windows nativo | Los namespaces son del kernel Linux |

---

## 🌐 Proyectos hermanos

| Repositorio | Tema |
|---|---|
| [docker-labs](https://github.com/vladimiracunadev-create/docker-labs) | Stacks reales con Docker Compose |
| [wsl-labs](https://github.com/vladimiracunadev-create/wsl-labs) | Contenedores nativos de WSL con `wslc` |
| [unikernel-labs](https://github.com/vladimiracunadev-create/unikernel-labs) | Unikernels y microVMs |

---

## 🔗 Ver también

- [Índice de documentación](docs/DOCUMENTATION_INDEX.md)
- [Código de conducta](CODE_OF_CONDUCT.md)
