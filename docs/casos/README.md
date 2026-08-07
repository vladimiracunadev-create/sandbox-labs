# 📇 Fichas de los casos

Una ficha por caso. Todas tienen la misma estructura, para que se puedan
comparar sin releer:

**En una frase, para cualquiera** · **Por qué se realiza este caso** · **La idea
que enseña** · **Casos de uso reales** · **Cómo funciona** (con diagramas) ·
**Esquemas** de entrada y salida · **Software necesario** · **Instalación** ·
**Procesos que se crean** · **Tiempo de carga** · **Si algo falla** ·
**Estado real y qué falta**

La sección **Si algo falla** de cada ficha lista los fallos propios de ese caso
con su causa y sus alternativas de solución. Los fallos comunes a todos están en
**[Cuando algo falla](../SOLUCION-DE-PROBLEMAS.md)**.

El estado de cada caso es el real, no el deseado. Los `planned` dicen que no hay
código; los `building` dicen exactamente qué falta.

---

## 🛡️ Familia técnica — 15 casos

Ejecutar código, archivos, plugins, agentes y secretos que no controlas.

| # | Ficha | Estado |
|---|---|:--:|
| 01 | [Contenido web no confiable](01-contenido-web-no-confiable.md) | 🟡 `building` |
| 02 | [Código generado por IA](02-codigo-generado-por-ia.md) | 🟡 `building` |
| 03 | [Procesamiento seguro de archivos comprimidos](03-procesamiento-seguro-de-archivos.md) | 🟡 `building` |
| 04 | [Plugins de terceros](04-plugins-de-terceros.md) | 🔴 `planned` |
| 05 | [Custodia de claves y firma](05-custodia-de-claves-y-firma.md) | 🟡 `building` |
| 06 | [Detonación en microVM](06-detonacion-en-microvm.md) | 🔴 `planned` |
| 07 | [Runtime determinista de contratos](07-runtime-determinista-de-contratos.md) | 🔴 `planned` |
| 08 | [Sandbox de herramientas de agente IA](08-sandbox-de-herramientas-de-agente-ia.md) | 🔴 `planned` |
| 09 | [Runner de CI con pull request externo](09-runner-de-ci-con-pull-request-externo.md) | 🔴 `planned` |
| 10 | [Construcción de paquetes de terceros](10-construccion-de-paquetes.md) | 🔴 `planned` |
| 11 | [Renderizado de documentos](11-renderizado-de-documentos.md) | 🔴 `planned` |
| 12 | [Notebooks de ciencia de datos](12-notebooks-de-ciencia-de-datos.md) | 🔴 `planned` |
| 13 | [Migraciones de base de datos](13-migraciones-de-base-de-datos.md) | 🔴 `planned` |
| 14 | [Análisis de binarios de terceros](14-analisis-de-binarios-de-terceros.md) | 🔴 `planned` |
| 15 | [Instalación de cadena de suministro](15-instalacion-de-cadena-de-suministro.md) | 🔴 `planned` |

## 🏛️ Familia mercado de capitales — 21 casos

Probar modelos Fintech con dinero, instrumentos y participantes **simulados**.

> [!WARNING]
> **Sin dinero real, sin valores reales, sin credenciales reales y sin
> conectividad de producción.** El simulador **no es una autorización
> regulatoria** de la CMF ni de ninguna otra autoridad, y nada de lo que produzca
> es una recomendación de inversión.

| # | Ficha | Estado |
|---|---|:--:|
| CM-00 | [Entrada al sandbox regulatorio](cm-00-entrada-al-sandbox-regulatorio.md) | 🟠 `prototype` |
| CM-01 | [Financiamiento colectivo](cm-01-financiamiento-colectivo.md) | 🟠 `prototype` |
| CM-02 | [Sistema alternativo de transacción](cm-02-sistema-alternativo-de-transaccion.md) | 🟠 `prototype` |
| CM-03 | [Custodia y segregación de activos](cm-03-custodia-y-segregacion-de-activos.md) | 🟢 `functional` |
| CM-04 | [Enrutamiento inteligente de órdenes](cm-04-enrutamiento-inteligente-de-ordenes.md) | 🟠 `prototype` |
| CM-05 | [Intermediación financiera](cm-05-intermediacion-financiera.md) | 🟠 `prototype` |
| CM-06 | [Asesoría crediticia](cm-06-asesoria-crediticia.md) | 🟠 `prototype` |
| CM-07 | [Robo-advisor](cm-07-robo-advisor.md) | 🟠 `prototype` |
| CM-08 | [Tokenización de instrumentos](cm-08-tokenizacion-de-instrumentos.md) | 🟠 `prototype` |
| CM-09 | [Vigilancia de abuso de mercado](cm-09-vigilancia-de-abuso-de-mercado.md) | 🟠 `prototype` |
| CM-10 | [Compensación y liquidación](cm-10-compensacion-y-liquidacion.md) | 🟠 `prototype` |
| CM-11 | [Finanzas abiertas y consentimiento](cm-11-finanzas-abiertas-y-consentimiento.md) | 🟠 `prototype` |
| CM-12 | [Reportería regulatoria y SupTech](cm-12-reporteria-regulatoria.md) | 🟠 `prototype` |
| CM-13 | [Salida ordenada](cm-13-salida-ordenada.md) | 🟠 `prototype` |
| CM-14 | [Resiliencia operacional](cm-14-resiliencia-operacional.md) | 🟠 `prototype` |
| CM-15 | [KYC, AML y sanciones](cm-15-kyc-aml-y-sanciones.md) | 🟠 `prototype` |
| CM-16 | [Integridad de datos de mercado](cm-16-integridad-de-datos-de-mercado.md) | 🟠 `prototype` |
| CM-17 | [Eventos corporativos](cm-17-eventos-corporativos.md) | 🟠 `prototype` |
| CM-18 | [Margen, garantías y riesgo](cm-18-margen-garantias-y-riesgo.md) | 🟠 `prototype` |
| CM-19 | [Fraude y toma de cuentas](cm-19-fraude-y-toma-de-cuentas.md) | 🟠 `prototype` |
| CM-20 | [Gobierno de modelos e IA financiera](cm-20-gobierno-de-modelos-e-ia-financiera.md) | 🟠 `prototype` |

---

## Por dónde empezar

- **Si nunca has usado un sandbox:** [Qué es un sandbox](../QUE-ES-UN-SANDBOX.md),
  y después la ficha del [caso 01](01-contenido-web-no-confiable.md).
- **Si quieres ver algo funcionando hoy:**
  [CM-03 · custodia](cm-03-custodia-y-segregacion-de-activos.md) es el caso con
  prueba automática más completa.
- **Si vienes del mundo financiero:**
  [CM-00 · entrada al sandbox regulatorio](cm-00-entrada-al-sandbox-regulatorio.md)
  es la puerta de la familia.
- **Si quieres saber qué está construido de verdad:** [Estado del proyecto](../ESTADO.md).
- **Si algo te falla:** [Cuando algo falla](../SOLUCION-DE-PROBLEMAS.md).

---

**Ver también:** [Catálogo completo](../CATALOGO.md) · [Estado del proyecto](../ESTADO.md) · [Documentación](../README.md)
