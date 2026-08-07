# CM-03 · Custodia y segregación de activos

> ⚠️ **Activos, clientes y saldos simulados.** Ningún valor de este caso existe
> en ningún mercado. Sin autorización de la CMF ni de ninguna autoridad, y nada
> de esto es una recomendación de inversión.

**Estado: `functional`.** Se ejecuta, hace una tarea real y falla en cerrado
cuando un escenario deja de detectar lo que venía a detectar.

```bash
cargo run -p sandboxctl -- markets reconcile
```

---

## 🎯 La idea única que enseña

**Un custodio puede cuadrar perfectamente y aun así no saber qué es de quién.**

Sumar todo y ver que da el total correcto no dice nada sobre segregación. La
pregunta que hay que poder responder cuando un custodio quiebra no es «¿cuánto
hay?», es «¿cuánto es de cada cliente?».

## 📐 La invariante

```text
activos registrados a nombre de clientes
  =
activos efectivamente custodiados
  +
operaciones pendientes explicadas
```

Los tres términos importan:

| Término | Qué es | De dónde sale |
|---|---|---|
| registrados | lo que el libro dice que los clientes tienen | del propio sistema |
| custodiados | lo que el custodio dice que hay | **dato externo** |
| pendientes explicados | lo que está en tránsito **y se puede nombrar** | del sistema, con motivo |

El extracto del custodio es un dato de fuera a propósito: si se derivara del
registro, la conciliación no compararía nada consigo misma.

Y **los activos propios del custodio nunca entran en la comparación**. Sumarlos
taparía cualquier faltante de clientes, que es justo el fallo que este caso
existe para detectar.

## 🧪 Escenarios

| Escenario | Qué provoca | Hallazgo esperado |
|---|---|---|
| `CM-03-NORMAL-001` | Día normal | ninguno |
| `CM-03-LIQUIDACION-002` | Venta liquidando a T+2 | ninguno — un pendiente con motivo explica el hueco |
| `CM-03-FALTANTE-003` | Faltan 200 títulos de clientes, sobran 1.000 propios | `shortfall` |
| `CM-03-EXCUSA-004` | Pendiente sin motivo | `unexplained-pending` + `shortfall` |
| `CM-03-SOBRANTE-005` | El custodio tiene algo que nadie registró | `surplus` |
| `CM-03-NEGATIVA-006` | Un cliente con posición negativa | `negative-client-position` |

**Cada escenario declara lo que espera.** Uno adverso que deja de provocar su
hallazgo se marca como roto y devuelve código 1 — un escenario que aprueba pase
lo que pase es decoración.

También se marca lo contrario: un hallazgo que aparece sin estar declarado
significa que el escenario ya no describe lo que ocurre.

## 🛡️ De qué protege, y de qué no

| Detecta | No detecta |
|---|---|
| Faltantes de activos de clientes | Fraude con documentación coherente |
| Sobrantes no registrados | Un custodio que miente en su extracto |
| Posiciones negativas de cliente | Riesgo de crédito o de mercado |
| Pendientes sin justificar | Nada relacionado con dinero real |

El extracto del custodio se toma como verdad sobre lo que hay. Un custodio que
declara mal es un problema distinto, y este caso no lo resuelve.

## ✅ Criterios de aceptación

- [x] Se ejecuta con un comando documentado
- [x] Tiene escenario normal y escenarios adversos
- [x] Cada escenario declara lo que espera y se comprueba
- [x] Falla en cerrado si un escenario deja de detectar lo suyo
- [x] Pruebas automatizadas (31 en `crates/sandbox-markets`)
- [x] Documenta sus límites
- [x] No necesita secretos ni datos reales
- [ ] Panel: todavía no aparece en el Control Center
- [ ] Evidencia firmada como la de los sandboxes técnicos

## 🔗 Relacionado

- [Familia de mercado de capitales](../../README.md)
- [Libro mayor](../../../../crates/sandbox-markets/src/ledger.rs) — el dinero exacto que sostiene esto
