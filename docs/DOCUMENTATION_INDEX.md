# 🗂️ Índice de documentación — sandbox-labs

> **Versión**: 0.7.0
> **Estado**: 🟢 Activo · plataforma educativa de aislamiento con evidencia reproducible
> **Uso recomendado**: 📍 empieza aquí si no sabes qué documento abrir primero.

---

## 🚀 Inicio y operación

| Documento | Qué resuelve | Abrir |
|---|---|---|
| README | Portada, arquitectura y quickstart | [Abrir](../README.md) |
| Environment Setup | De un equipo en blanco a la primera evidencia | [Abrir](../ENVIRONMENT_SETUP.md) |
| Getting Started | Ruta corta: preparar, sondear, planificar | [Abrir](GETTING_STARTED.md) |
| Operating Modes | Panel, CLI, laboratorios y launcher | [Abrir](../OPERATING-MODES.md) |
| Runbook | Operación diaria y respuesta rápida | [Abrir](../RUNBOOK.md) |
| Troubleshooting | Síntoma → causa → arreglo | [Abrir](TROUBLESHOOTING.md) |
| FAQ | Dudas de concepto y de diseño | [Abrir](../FAQ.md) |
| Support | Cómo pedir ayuda y qué queda fuera de alcance | [Abrir](../SUPPORT.md) |

---

## 🛡️ Aislamiento, políticas y evidencia

| Documento | Qué resuelve | Abrir |
|---|---|---|
| Policy Reference | Cada campo de una política y qué significa | [Abrir](POLICY_REFERENCE.md) |
| Control Enforcement Matrix | Qué control aplica de verdad cada runtime | [Abrir](CONTROL_ENFORCEMENT_MATRIX.md) |
| Runtime Adapters | Contrato `RuntimeAdapter` y estado por adaptador | [Abrir](RUNTIME_ADAPTERS.md) |
| Evidence Format | Estructura del JSON de evidencia y sus hashes | [Abrir](EVIDENCE_FORMAT.md) |
| Threat Model | Qué protege el sistema y qué explícitamente no | [Abrir](THREAT_MODEL.md) |
| Security Policy | Reporte de vulnerabilidades y reglas duras | [Abrir](../SECURITY.md) |

---

## 🏗️ Arquitectura y referencia técnica

| Documento | Qué resuelve | Abrir |
|---|---|---|
| Architecture | Capas, contrato de adaptador y flujo de un trabajo | [Abrir](ARCHITECTURE.md) |
| File Architecture | Mapa de carpetas y responsabilidades | [Abrir](../FILE_ARCHITECTURE.md) |
| API | Endpoints del Control Center | [Abrir](API.md) |
| Compatibility | Qué funciona en cada SO y con cada runtime | [Abrir](../COMPATIBILITY.md) |
| Glossary | Vocabulario del proyecto y del aislamiento en Linux | [Abrir](../GLOSSARY.md) |
| Windows y WSL2 | Particularidades del entorno Windows | [Abrir](WINDOWS_WSL2.md) |

---

## 🧪 Laboratorios y cargas

| Documento | Qué resuelve | Abrir |
|---|---|---|
| Labs Catalog | Los 18 laboratorios, nivel y estado | [Abrir](LABS_CATALOG.md) |
| Laboratorios (carpetas) | README por laboratorio | [Abrir](../labs/) |
| Cargas registradas | Manifiestos y riesgo de cada carga | [Abrir](../workloads/) |
| Adaptadores (notas) | Apuntes y ejemplos por runtime | [Abrir](../adapters/) |
| Benchmarks | Matriz de comparación entre runtimes | [Abrir](BENCHMARKS.md) |

---

## ✅ Calidad y proceso

| Documento | Qué resuelve | Abrir |
|---|---|---|
| Testing | Qué prueba cada suite y cómo ejecutarlas | [Abrir](TESTING.md) |
| Validation | Qué se verificó realmente en esta versión | [Abrir](../VALIDATION.md) |
| Project Status | Consolidado, experimental y pendiente | [Abrir](../PROJECT_STATUS.md) |
| Roadmap | Hacia dónde va el proyecto | [Abrir](../ROADMAP.md) |
| Changelog | Historial de versiones | [Abrir](../CHANGELOG.md) |
| Implementation Backlog | Trabajo pendiente por adaptador | [Abrir](IMPLEMENTATION_BACKLOG.md) |
| Contributing | Cómo contribuir y la regla de `ready` | [Abrir](../CONTRIBUTING.md) |
| Code of Conduct | Convivencia en el proyecto | [Abrir](../CODE_OF_CONDUCT.md) |

---

## 🤖 Agentes y automatización

| Documento | Qué resuelve | Abrir |
|---|---|---|
| AGENTS | Reglas para agentes que trabajen en el repo | [Abrir](../AGENTS.md) |
| CODEX | Contexto específico para Codex | [Abrir](../CODEX.md) |
| Codex Handoff | Estado de la entrega y siguientes pasos | [Abrir](../CODEX_HANDOFF.md) |
| Recruiter Guide | Qué demuestra este repositorio | [Abrir](../RECRUITER.md) |

---

## 🧭 Tres rutas de lectura

### Quiero usarlo ya

`README` → [`ENVIRONMENT_SETUP`](../ENVIRONMENT_SETUP.md) →
[`OPERATING-MODES`](../OPERATING-MODES.md) → [`RUNBOOK`](../RUNBOOK.md)

### Quiero entender el aislamiento

[`GLOSSARY`](../GLOSSARY.md) → [`labs/01`](../labs/01-baseline-unrestricted/) →
[`POLICY_REFERENCE`](POLICY_REFERENCE.md) →
[`CONTROL_ENFORCEMENT_MATRIX`](CONTROL_ENFORCEMENT_MATRIX.md) →
[`THREAT_MODEL`](THREAT_MODEL.md)

### Quiero contribuir código

[`ARCHITECTURE`](ARCHITECTURE.md) → [`FILE_ARCHITECTURE`](../FILE_ARCHITECTURE.md) →
[`RUNTIME_ADAPTERS`](RUNTIME_ADAPTERS.md) → [`TESTING`](TESTING.md) →
[`CONTRIBUTING`](../CONTRIBUTING.md)
