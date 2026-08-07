# CM-09 · Vigilancia de abuso de mercado

> **En una frase, para cualquiera:** hay formas de mover un precio sin comprar
> ni vender de verdad. Consisten en aparentar interés y retirarlo justo antes de
> que alguien lo acepte.

**Estado real:** 🔴 `planned` · **Carpeta prevista:** `domains/capital-markets/cases/09-market-surveillance`

> [!WARNING]
> **Órdenes, cuentas y operaciones simuladas. Sin datos personales reales.** No es
> una autorización regulatoria.

---

## Por qué se realiza este caso

Cada operación por separado puede ser perfectamente legal. El abuso está **en el
patrón**, y el patrón solo se ve mirando muchas operaciones juntas, a menudo de
cuentas distintas que actúan coordinadas.

| Patrón | En qué consiste |
|---|---|
| **Wash trading** | Comprar y vender contra uno mismo: volumen falso, precio inventado |
| **Spoofing** | Poner órdenes grandes sin intención de ejecutarlas, para mover el precio, y cancelarlas |
| **Layering** | Varias capas de órdenes falsas a distintos precios, para simular profundidad |
| Manipulación del cierre | Concentrar operaciones en los últimos segundos, cuando el precio de cierre se fija |
| Cuentas coordinadas | Varias cuentas actuando como una, para no superar límites individuales |
| Volumen anómalo | Actividad inexplicable justo antes de una noticia |
| Uso de información privilegiada | Operar sabiendo algo que el mercado no sabe |

## La idea que enseña, y que ningún otro caso enseña

**Detectar es solo el principio.** Una alerta no es una conclusión: es el
comienzo de un expediente. Y el expediente exige **reconstruir la sesión
completa** —qué se veía en el libro en cada instante— porque sin eso no se puede
distinguir una orden cancelada de mala fe de una cancelada por un cambio legítimo.

De ahí la dependencia: este caso necesita que
[CM-02](cm-02-sistema-alternativo-de-transaccion.md) sepa reconstruir su sesión.

## Casos de uso reales

- El área de vigilancia de un mercado o de un intermediario.
- Una investigación sobre actividad sospechosa en un instrumento.
- Formación de analistas con patrones etiquetados.
- Probar si un algoritmo propio produce, sin querer, patrones sospechosos.

## Cómo funcionará

```mermaid
flowchart LR
  O["📊 Flujo de órdenes<br/>y operaciones"] --> D["🕵️ Detectores"]
  D --> A["🚨 Alerta"]
  A --> AN["🔍 Análisis"]
  AN --> RC["⏪ Reconstrucción<br/>del libro instante a instante"]
  RC --> RE["🕸️ Relación entre cuentas"]
  RE --> CL["🏷️ Clasificación"]
  CL --> IN["📁 Investigación"]
  IN --> ME["⚖️ Medidas"]
  ME --> EX["📚 Expediente"]
```

```mermaid
flowchart TB
  A["Órdenes de una cuenta"] --> B{"¿Tasa de cancelación<br/>muy alta?"}
  B -- sí --> C{"¿Las cancelaciones ocurren<br/>justo antes de ejecutarse?"}
  C -- sí --> D{"¿El precio se movió<br/>en su beneficio?"}
  D -- sí --> E["🚨 Spoofing probable"]
  D -- no --> F["📣 Anómalo, sin beneficio claro"]
  B -- no --> G{"¿Compra y vende<br/>contra sí misma?"}
  G -- sí --> H["🚨 Wash trading"]
```

## Esquemas

```json
{
  "alert": {
    "kind": "Spoofing",
    "account": "acc-sim-12",
    "instrument": "ACME-SIM",
    "window": { "fromSeq": 10420, "toSeq": 10980 },
    "evidence": {
      "ordersPlaced": 42,
      "ordersCancelled": 40,
      "cancelRate": 0.95,
      "avgTimeToCancelMs": 180,
      "priceImpact": { "minorUnits": 120, "currency": "CLP" }
    },
    "severity": "alta"
  }
}
```

```json
{
  "case": {
    "id": "exp-2026-001",
    "alerts": ["alert-1", "alert-2"],
    "relatedAccounts": [{ "a": "acc-sim-12", "b": "acc-sim-31", "reason": "mismo patrón temporal y contraparte recurrente" }],
    "classification": "manipulación de mercado",
    "measures": ["suspensión cautelar de la cuenta"],
    "reconstructedBook": true
  }
}
```

## Software necesario

| Componente | Para qué |
|---|---|
| **Rust** 1.75+ | Detectores, reconstrucción de sesión y expediente |
| **Node.js** 20+ / **pnpm** 9+ | Visualización de la línea de tiempo del libro (recomendado) |

Sin jaula ni Linux. **Sin datos personales reales**: las cuentas son sintéticas.

## Instalación

```bash
cargo build --release
```

## Procesos que se crearán

```text
sandboxctl markets surveil --session sesion.jsonl
  │
  └─ un proceso determinista, sin red
      ├─ reconstrucción del libro por número de secuencia
      ├─ detectores en paralelo sobre la misma sesión
      └─ expediente append-only
```

## Tiempo de carga estimado

| Operación | Coste esperado |
|---|---|
| Reconstruir una sesión de 100 000 eventos | 1–5 s |
| Ejecutar todos los detectores sobre ella | 1–3 s |
| Generar un expediente | < 100 ms |

## Qué hace falta para construirlo

1. Que [CM-02](cm-02-sistema-alternativo-de-transaccion.md) sepa reconstruir su
   sesión completa. **Es un requisito previo, no una mejora.**
2. Detectores para los ocho patrones, cada uno con escenario etiquetado.
3. Grafo de relación entre cuentas.
4. Expediente append-only con las medidas adoptadas.
5. Escenarios sintéticos con el patrón conocido de antemano, para poder medir
   falsos positivos y falsos negativos.

---

**Ver también:** [Catálogo completo](../CATALOGO.md) · [CM-02 · libro de órdenes](cm-02-sistema-alternativo-de-transaccion.md) · [CM-05 · intermediación](cm-05-intermediacion-financiera.md)
