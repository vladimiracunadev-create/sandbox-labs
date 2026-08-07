# 🏛️ Mercado de capitales

> **Estado: 1 de 21 casos.** Existen el dinero exacto, el libro mayor y
> **[CM-03 · Custodia y segregación](cases/03-asset-custody/README.md)**, que se
> ejecuta con `sandboxctl markets reconcile`. Los otros veinte **no existen**:
> están listados abajo como lo que son, un plan.

---

## ⚠️ Lo primero, y no es un formalismo

| | |
|---|---|
| **Dinero** | Simulado. Sin conexión a ningún banco, medio de pago ni cuenta real |
| **Instrumentos** | Simulados. Ningún valor de este simulador existe en ningún mercado |
| **Autorización** | **Ninguna.** Este proyecto no representa a la CMF ni a ninguna autoridad, y usarlo no es participar en ningún sandbox regulatorio |
| **Asesoría** | Nada de lo que salga de aquí es una recomendación de inversión |
| **Datos** | Sintéticos. Ningún dato personal real entra aquí, tampoco de prueba |

Un simulador que se confundiera con cualquiera de esas cosas haría daño. Por eso
el aviso va antes que la descripción.

---

## 🧱 Por qué esto vive separado de los sandboxes técnicos

El catálogo tiene dos familias y **no se mezclan**:

| Familia | Qué contiene | De qué protege |
|---|---|---|
| `technical` | Código, archivos, plugins, agentes, secretos | De que un programa toque lo que no le corresponde |
| `capital-markets` | Operación, custodia, negociación, cumplimiento | De que un modelo de negocio se pruebe con dinero de verdad |

Tienen modelos de amenazas distintos. Mezclarlos haría que una advertencia de
uno se leyera como si valiera para el otro — «esto está contenido» significa
cosas muy diferentes en cada lado.

---

## ✅ Qué existe hoy

### Dinero exacto — `crates/sandbox-markets/src/money.rs`

Enteros en la unidad mínima, nunca coma flotante. `0.1 + 0.2` no es `0.3` en
binario, y un céntimo perdido en una cuenta de clientes es un descuadre que hay
que explicar.

La moneda va pegada al importe: sumar 100 CLP y 100 USD **falla**, no sale mal.
Y el peso chileno tiene **cero decimales**, que es justo el caso que rompe todo
el código escrito asumiendo «dos decimales siempre».

### Custodia y segregación — [CM-03](cases/03-asset-custody/README.md)

El primer caso del dominio, y se ejecuta:

```bash
cargo run -p sandboxctl -- markets reconcile
```

Seis escenarios —uno normal, uno de liquidación en curso y cuatro adversos—
sobre la invariante `registrado = custodiado + pendiente explicado`. Cada uno
**declara lo que espera detectar**, así que uno adverso que deje de provocar su
hallazgo se marca como roto y devuelve código 1. Un escenario que aprueba pase
lo que pase es decoración.

### Libro mayor de doble entrada — `crates/sandbox-markets/src/ledger.rs`

Tres reglas, con una prueba cada una:

1. **Cada transacción cuadra a cero.** Si un asiento no tiene contrapartida, el
   dinero apareció de la nada.
2. **Nada se borra.** Un error se corrige con una **reversa** que apunta a la
   original; las dos quedan. Un libro donde se puede borrar es un borrador.
3. **Una transacción se aplica una vez.** La clave de idempotencia evita el
   fallo más caro y más fácil de tener con reintentos: cobrar dos veces.

Y una cuarta que sostiene a las demás: los saldos son **caché** del diario y se
pueden reconstruir desde él. Eso es lo que permite comprobar que no se han
desviado y, en un incidente, levantar el estado desde los hechos.

---

## ⛔ Qué NO existe todavía

Veinte de los veintiún casos. Están planificados y **no construidos** — el
único que existe es CM-03, marcado abajo:

| | Caso | De qué trata |
|---|---|---|
| CM-00 | Entrada al sandbox regulatorio | Postulación, clasificación de servicios, límites y salida ordenada |
| CM-01 | Financiamiento colectivo | Campañas, divulgaciones, sobredemanda, devolución |
| CM-02 | Sistema alternativo de transacción | Libro de órdenes, prioridad precio-tiempo, suspensión |
| **CM-03** | **[Custodia y segregación](cases/03-asset-custody/README.md)** | ✅ **Construido.** Activos de clientes ≠ activos propios |
| CM-04 | Enrutamiento de órdenes | Precio, liquidez, latencia, y por qué se eligió |
| CM-05 | Intermediación | Agente contra principal, y el conflicto de interés |
| CM-06 | Asesoría crediticia | Capacidad de pago, costo total, conflictos comerciales |
| CM-07 | Robo-advisor | Perfil, cartera, rebalanceo, explicabilidad |
| CM-08 | Tokenización | Emisión, respaldo, doble representación |
| CM-09 | Vigilancia de abuso | Wash trading, spoofing, layering |
| CM-10 | Compensación y liquidación | Netting, entrega contra pago, fallas |
| CM-11 | Finanzas abiertas | Consentimiento, alcance, revocación |
| CM-12 | Reportería regulatoria | Consolidación, validación, correcciones |
| CM-13 | Salida ordenada | Detener, liquidar, devolver, notificar |
| CM-14 | Resiliencia operacional | Kill switch, degradación, replay |
| CM-15 | KYC, AML y sanciones | Identificación, riesgo, monitoreo |
| CM-16 | Integridad de datos de mercado | Precio cero, moneda incorrecta, timestamp futuro |
| CM-17 | Eventos corporativos | Dividendos, splits, canjes |
| CM-18 | Margen y garantías | Haircut, llamadas de margen, liquidación forzada |
| CM-19 | Fraude y toma de cuentas | Dispositivo nuevo, retiro anómalo, sesión imposible |
| CM-20 | Gobierno de modelos e IA | Versión, métricas, sesgo, drift, rollback |

También faltan el motor de escenarios con semilla, el reloj simulado, los
participantes, los instrumentos y la política regulatoria como código.

**Ninguno de los veinte pendientes aparece como disponible en el panel ni en el
sitio.**
Un catálogo que promete lo que no tiene es exactamente lo que este repositorio
existe para no hacer.

---

## 🔗 Relacionado

- [Backlog técnico](../../docs/IMPLEMENTATION_BACKLOG.md) — los huecos del núcleo
- [Roadmap](../../ROADMAP.md) — el orden en que se abordan
- [Modelo de amenazas](../../docs/THREAT_MODEL.md) — qué protege el proyecto y qué no
