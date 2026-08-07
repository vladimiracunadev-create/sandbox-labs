# CM-01 · Financiamiento colectivo

> **En una frase, para cualquiera:** mucha gente pone poco dinero en un proyecto
> que todavía no existe. La plataforma que junta ese dinero lo tiene en sus manos
> durante semanas, y tiene que devolverlo entero si la campaña no llega a su meta.

**Estado real:** 🔴 `planned` · **Carpeta prevista:** `domains/capital-markets/cases/01-crowdfunding-platform`

> [!WARNING]
> **Dinero, emisores e inversionistas simulados.** No es una autorización
> regulatoria ni una recomendación de inversión.

---

## Por qué se realiza este caso

El financiamiento colectivo pone dinero de mucha gente poco experta en manos de
una empresa pequeña, durante un periodo en el que **el dinero ya no está en el
bolsillo del inversionista pero todavía no es del emisor**. Ese limbo es donde
ocurre casi todo lo que sale mal.

| Momento | Qué puede fallar |
|---|---|
| La campaña se publica | Información incompleta o contradictoria |
| Entra dinero | Se mezcla con el de la plataforma |
| Se supera la meta | ¿Quién entra y quién se queda fuera? |
| No se alcanza el mínimo | Hay que devolverlo **todo**, y a tiempo |
| Cambian las condiciones a mitad | Quien invirtió lo hizo con otras reglas |
| Después del cierre | El emisor deja de informar |

## La idea que enseña

**El dinero en tránsito tiene dueño.** Mientras la campaña está abierta, cada
peso sigue siendo del inversionista. La plataforma lo custodia, no lo posee. Y la
prueba de que eso es cierto es que **la devolución funciona**.

## Casos de uso reales

- Una plataforma de financiamiento colectivo de deuda o capital.
- Preventa de un producto con meta mínima.
- Financiamiento de facturas por varios inversionistas.
- Formación sobre por qué una meta mínima protege a quien invierte.

## Cómo funcionará

```mermaid
flowchart LR
  E["🏢 Emisor"] --> C["📣 Campaña<br/>meta mínima y máxima"]
  I["👥 Inversionistas"] -->|"dinero"| S["🏦 Cuenta segregada"]
  C --> D{"¿Se alcanzó<br/>el mínimo al cierre?"}
  D -- sí --> A["📊 Asignación<br/>y traspaso al emisor"]
  D -- no --> R["↩️ Devolución íntegra"]
  A --> M["📈 Seguimiento<br/>de avances"]
  M --> F{"¿El emisor<br/>incumple?"}
  F -- sí --> G["📣 Alertas e<br/>información a inversionistas"]
```

```mermaid
flowchart TB
  A["Llega una inversión"] --> B{"¿Supera el<br/>límite del inversionista?"}
  B -- sí --> B1["🚫 Rechazada"]
  B -- no --> C{"¿Supera el máximo<br/>de la campaña?"}
  C -- sí --> C1["📊 Prorrateo por<br/>regla publicada"]
  C -- no --> D["✅ Registrada en<br/>cuenta segregada"]
```

## Esquemas

```json
{
  "campaign": {
    "issuer": "emisor-simulado-1",
    "minTarget": { "minorUnits": 10000000, "currency": "CLP" },
    "maxTarget": { "minorUnits": 50000000, "currency": "CLP" },
    "closesAt": "2026-09-30T23:59:59Z",
    "allocationRule": "pro-rata"
  }
}
```

```json
{
  "outcome": "refunded",
  "raised": { "minorUnits": 7400000, "currency": "CLP" },
  "investorsRefunded": 118,
  "refundComplete": true,
  "ledgerBalanced": true
}
```

`ledgerBalanced` no es decorativo: se apoya en el libro de partida doble del
crate [`sandbox-markets`](../../crates/sandbox-markets), que **ya está
construido**, y significa que la devolución cuadra hasta el último peso.

## Escenarios que traerá

Información contradictoria · operación con parte relacionada · exceso de
inversión de un mismo inversionista · cambio de condiciones a mitad de campaña ·
campaña que no alcanza el mínimo · emisor que deja de reportar avances.

## Software necesario

| Componente | Para qué |
|---|---|
| **Rust** 1.75+ | Motor de campañas sobre `Money` y `Ledger` |
| **Node.js** 20+ / **pnpm** 9+ | Interfaz en el panel (opcional) |

No necesita `bubblewrap`: no se ejecuta código ajeno, se prueban reglas de
negocio. Funciona en cualquier sistema con Rust.

## Instalación

```bash
cargo build --release
cargo run -p sandboxctl -- markets --help
```

## Procesos que se crearán

```text
sandboxctl markets crowdfunding --scenario no-alcanza-minimo
  │
  └─ un proceso determinista, sin red
      ├─ reloj simulado (las campañas duran semanas)
      ├─ libro de partida doble
      └─ evidencia firmada
```

## Tiempo de carga estimado

| Operación | Coste esperado |
|---|---|
| Un escenario completo de campaña | < 100 ms |
| Simular 10 000 inversiones | < 1 s |
| Devolución íntegra y cuadre | < 50 ms |

## Qué hace falta para construirlo

1. Modelo de campaña con meta mínima, máxima y regla de asignación publicada.
2. Reloj simulado, para que semanas pasen en milisegundos.
3. Devolución íntegra verificada contra el libro contable.
4. Los seis escenarios listados arriba, cada uno con su hallazgo esperado.

---

**Ver también:** [Catálogo completo](../CATALOGO.md) · [CM-03 · custodia](cm-03-custodia-y-segregacion-de-activos.md) · [CM-00 · entrada](cm-00-entrada-al-sandbox-regulatorio.md)
