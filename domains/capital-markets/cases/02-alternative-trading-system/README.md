# CM-02 · Sistema alternativo de transacción

> ⚠️ **Instrumentos, participantes y órdenes simulados.** Ningún valor de este
> caso existe en ningún mercado. **No es un sistema de negociación autorizado**,
> y nada de esto es una recomendación de inversión.

**Estado: `prototype`.** El motor de casación existe y está probado; le falta
todavía escenarios ejecutables como los de CM-03 y sesión de mercado.

---

## 🎯 La idea única que enseña

**Un mercado justo se reduce a una regla, y esa regla es la que se rompe.**

Entre dos órdenes que quieren lo mismo se sirve primero la del **mejor precio**;
a igual precio, **la que llegó antes**. Nada más: ni por tamaño, ni por cliente,
ni por quién paga más comisión.

Suena obvio, y un libro que atiende primero al grande o al de casa deja de ser un
mercado y pasa a ser un reparto. Por eso hay una prueba llamada
`size_never_jumps_the_queue`.

## 📐 Las invariantes, con una prueba cada una

| Invariante | Por qué importa |
|---|---|
| Mejor precio primero | Sin esto, el precio deja de significar algo |
| A igual precio, quien llegó antes | Es la mitad que se rompe en los repartos |
| El tamaño **no** adelanta la cola | El abuso clásico: servir primero al grande |
| El precio lo pone la orden **que ya estaba** | Si mandara la entrante, llegar tarde sería una ventaja |
| El libro nunca queda cruzado | Una compra que paga más que la mejor venta significa una ejecución que no ocurrió |
| Una orden de mercado sin contrapartida se **rechaza** | No tiene precio con el que esperar; dejarla reposando sería inventarle uno |
| Todo lo ejecutado queda en la cinta | Un mercado que no reconstruye sus ejecuciones no se puede auditar |

## ⏱️ Por qué el tiempo es un contador y no un reloj

Dos órdenes con el mismo milisegundo tendrían que desempatar por algo, y ese algo
acabaría siendo el orden interno de un mapa — es decir, el azar. Cada orden
recibe un número de secuencia que solo crece, **puesto por el libro y no por
quien la manda**: si lo pusiera el cliente, podría colarse delante diciendo que
llegó antes.

## ⛔ Qué falta

- Escenarios ejecutables como los de [CM-03](../03-asset-custody/README.md),
  con `expected` declarado.
- Sesión de mercado: apertura, cierre, subasta y suspensión.
- Reconstrucción completa del libro desde la cinta.
- Detección de abuso (`wash trading`, `spoofing`, `layering`) — eso es CM-09.

Mientras falten los escenarios, este caso es `prototype` y no `functional`.

## 🔗 Relacionado

- [Familia de mercado de capitales](../../README.md)
- [CM-03 · Custodia](../03-asset-custody/README.md) — el caso que ya se ejecuta
- Motor: `crates/sandbox-markets/src/orderbook.rs`
