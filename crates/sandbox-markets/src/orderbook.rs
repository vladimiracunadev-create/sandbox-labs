//! Libro de órdenes con prioridad precio-tiempo.
//!
//! # La regla que define un mercado justo
//!
//! Entre dos órdenes que quieren lo mismo, se sirve primero **la del mejor
//! precio**; y a igual precio, **la que llegó antes**. Nada más. Sin excepciones
//! por tamaño, por cliente ni por quién paga más comisión.
//!
//! Suena obvio y es exactamente lo que se rompe en los casos reales de abuso:
//! un libro que atiende primero al grande, o al de casa, deja de ser un mercado
//! y pasa a ser un reparto.
//!
//! # Por qué la marca de tiempo es un contador y no un reloj
//!
//! Dos órdenes con el mismo milisegundo tendrían que desempatar por algo, y ese
//! algo acabaría siendo el orden de un mapa — es decir, el azar. Aquí cada orden
//! recibe un número de secuencia que solo crece. Es reproducible, no depende del
//! reloj de la máquina y hace que un escenario dé siempre lo mismo.
//!
//! # Qué NO es
//!
//! Instrumentos y participantes simulados. Ningún valor de este módulo existe en
//! ningún mercado, y esto no es un sistema de negociación autorizado.

use crate::money::Money;

/// De qué lado del libro está una orden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// Quiere comprar. Mejor precio = **más alto**.
    Buy,
    /// Quiere vender. Mejor precio = **más bajo**.
    Sell,
}

impl Side {
    pub const fn opposite(self) -> Self {
        match self {
            Self::Buy => Self::Sell,
            Self::Sell => Self::Buy,
        }
    }
}

/// Una orden en el libro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    pub id: String,
    pub participant: String,
    pub side: Side,
    /// Precio límite. `None` es orden de mercado: acepta el precio que haya.
    pub limit: Option<Money>,
    pub quantity: i128,
    /// Número de secuencia. Lo pone el libro, no quien manda la orden: si lo
    /// pusiera el cliente, podría colarse delante diciendo que llegó antes.
    pub sequence: u64,
}

/// Una ejecución. Guarda las dos órdenes porque un informe que solo dice
/// «se cruzaron 100 a 1.200» no permite reconstruir quién hizo qué.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trade {
    pub buy_order: String,
    pub sell_order: String,
    pub quantity: i128,
    /// Precio de ejecución: el de la orden que **ya estaba** en el libro.
    ///
    /// Es lo justo: quien puso precio primero y esperó tiene derecho a él. Si se
    /// usara el de la orden entrante, llegar tarde sería una ventaja.
    pub price: Money,
}

/// Por qué el libro rechazó una orden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderError {
    /// Cantidad cero o negativa.
    InvalidQuantity(i128),
    /// Precio cero o negativo. Un instrumento no vale cero pesos.
    InvalidPrice(Money),
    /// Ya hay una orden con ese identificador.
    Duplicate(String),
    /// Una orden de mercado sin nadie enfrente no se queda en el libro: no
    /// tiene precio con el que esperar.
    NoLiquidity(String),
}

impl std::fmt::Display for OrderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidQuantity(value) => write!(f, "cantidad inválida: {value}"),
            Self::InvalidPrice(value) => write!(f, "precio inválido: {value}"),
            Self::Duplicate(id) => write!(f, "la orden «{id}» ya está en el libro"),
            Self::NoLiquidity(id) => write!(
                f,
                "la orden de mercado «{id}» no encontró contrapartida y no puede quedarse en el libro: no tiene precio"
            ),
        }
    }
}

impl std::error::Error for OrderError {}

/// El libro de un instrumento.
#[derive(Debug, Default)]
pub struct OrderBook {
    bids: Vec<Order>,
    asks: Vec<Order>,
    trades: Vec<Trade>,
    sequence: u64,
    seen: std::collections::BTreeSet<String>,
}

impl OrderBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mete una orden, la cruza contra lo que haya y devuelve las ejecuciones.
    pub fn submit(
        &mut self,
        id: impl Into<String>,
        participant: impl Into<String>,
        side: Side,
        limit: Option<Money>,
        quantity: i128,
    ) -> Result<Vec<Trade>, OrderError> {
        let id = id.into();
        if quantity <= 0 {
            return Err(OrderError::InvalidQuantity(quantity));
        }
        if let Some(price) = limit {
            if price.minor_units() <= 0 {
                return Err(OrderError::InvalidPrice(price));
            }
        }
        if !self.seen.insert(id.clone()) {
            return Err(OrderError::Duplicate(id));
        }

        self.sequence += 1;
        let mut incoming =
            Order { id: id.clone(), participant: participant.into(), side, limit, quantity, sequence: self.sequence };

        let executed = self.match_against_book(&mut incoming);

        if incoming.quantity > 0 {
            if incoming.limit.is_none() {
                // Una orden de mercado sin contrapartida no puede reposar: no
                // tiene precio al que esperar. Se retira y se dice.
                self.seen.remove(&id);
                return Err(OrderError::NoLiquidity(id));
            }
            self.rest(incoming);
        }
        self.trades.extend(executed.iter().cloned());
        Ok(executed)
    }

    /// Cruza la orden entrante contra el lado contrario, respetando
    /// precio-tiempo.
    fn match_against_book(&mut self, incoming: &mut Order) -> Vec<Trade> {
        let mut executed = Vec::new();
        let book = if incoming.side == Side::Buy { &mut self.asks } else { &mut self.bids };

        while incoming.quantity > 0 {
            // El libro está ordenado, así que la mejor siempre es la primera.
            let Some(best) = book.first_mut() else { break };
            let Some(resting_price) = best.limit else { break };

            if let Some(limit) = incoming.limit {
                let crosses = match incoming.side {
                    Side::Buy => limit.minor_units() >= resting_price.minor_units(),
                    Side::Sell => limit.minor_units() <= resting_price.minor_units(),
                };
                if !crosses {
                    break;
                }
            }

            let quantity = incoming.quantity.min(best.quantity);
            let (buy_order, sell_order) = match incoming.side {
                Side::Buy => (incoming.id.clone(), best.id.clone()),
                Side::Sell => (best.id.clone(), incoming.id.clone()),
            };
            executed.push(Trade { buy_order, sell_order, quantity, price: resting_price });

            incoming.quantity -= quantity;
            best.quantity -= quantity;
            if best.quantity == 0 {
                book.remove(0);
            }
        }
        executed
    }

    /// Coloca la orden en su lado, en el sitio que le toca por precio y tiempo.
    fn rest(&mut self, order: Order) {
        let side = order.side;
        let book = if side == Side::Buy { &mut self.bids } else { &mut self.asks };
        book.push(order);
        // Mejor precio primero; a igual precio, menor secuencia. Ese desempate
        // es la mitad de «precio-tiempo», y sin él el orden lo decidiría el azar.
        book.sort_by(|a, b| {
            let (left, right) =
                (a.limit.map(Money::minor_units).unwrap_or(0), b.limit.map(Money::minor_units).unwrap_or(0));
            let by_price = if side == Side::Buy { right.cmp(&left) } else { left.cmp(&right) };
            by_price.then(a.sequence.cmp(&b.sequence))
        });
    }

    /// Mejor compra y mejor venta. `None` si ese lado está vacío.
    pub fn top_of_book(&self) -> (Option<&Order>, Option<&Order>) {
        (self.bids.first(), self.asks.first())
    }

    pub fn bids(&self) -> &[Order] {
        &self.bids
    }

    pub fn asks(&self) -> &[Order] {
        &self.asks
    }

    pub fn trades(&self) -> &[Trade] {
        &self.trades
    }

    /// Retira una orden que no se ha ejecutado del todo.
    pub fn cancel(&mut self, id: &str) -> bool {
        for book in [&mut self.bids, &mut self.asks] {
            if let Some(index) = book.iter().position(|order| order.id == id) {
                book.remove(index);
                return true;
            }
        }
        false
    }

    /// ¿Está el libro cruzado? La mejor compra nunca puede pagar más que la
    /// mejor venta: si eso pasa, había una ejecución que no ocurrió.
    pub fn is_crossed(&self) -> bool {
        match (self.bids.first().and_then(|o| o.limit), self.asks.first().and_then(|o| o.limit)) {
            (Some(bid), Some(ask)) => bid.minor_units() >= ask.minor_units(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::money::Currency;

    fn clp(units: i128) -> Money {
        Money::new(units, Currency::Clp)
    }

    #[test]
    fn the_best_price_is_served_first() {
        let mut book = OrderBook::new();
        book.submit("v1", "ana", Side::Sell, Some(clp(1_300)), 100).expect("v1");
        book.submit("v2", "beto", Side::Sell, Some(clp(1_200)), 100).expect("v2");

        // El comprador cruza contra la venta MÁS BARATA, no contra la primera.
        let trades = book.submit("c1", "caro", Side::Buy, Some(clp(1_300)), 100).expect("c1");
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].sell_order, "v2");
        assert_eq!(trades[0].price, clp(1_200));
    }

    #[test]
    fn at_the_same_price_the_one_that_arrived_first_wins() {
        // La otra mitad de precio-tiempo, y la que se rompe en los repartos:
        // aquí `v2` es del mismo precio pero llegó después, y espera.
        let mut book = OrderBook::new();
        book.submit("v1", "ana", Side::Sell, Some(clp(1_200)), 100).expect("v1");
        book.submit("v2", "beto", Side::Sell, Some(clp(1_200)), 100).expect("v2");

        let trades = book.submit("c1", "caro", Side::Buy, Some(clp(1_200)), 100).expect("c1");
        assert_eq!(trades[0].sell_order, "v1", "a igual precio manda quien llegó antes");
        assert_eq!(book.asks()[0].id, "v2");
    }

    #[test]
    fn size_never_jumps_the_queue() {
        // El abuso clásico: servir primero al grande. Aquí `v2` es diez veces
        // mayor y sigue esperando su turno.
        let mut book = OrderBook::new();
        book.submit("v1", "pequeño", Side::Sell, Some(clp(1_200)), 10).expect("v1");
        book.submit("v2", "grande", Side::Sell, Some(clp(1_200)), 1_000).expect("v2");

        let trades = book.submit("c1", "caro", Side::Buy, Some(clp(1_200)), 10).expect("c1");
        assert_eq!(trades[0].sell_order, "v1");
    }

    #[test]
    fn the_resting_order_sets_the_price() {
        // Quien puso precio primero y esperó tiene derecho a él. Si mandara el
        // de la orden entrante, llegar tarde sería una ventaja.
        let mut book = OrderBook::new();
        book.submit("v1", "ana", Side::Sell, Some(clp(1_200)), 100).expect("v1");
        let trades = book.submit("c1", "beto", Side::Buy, Some(clp(1_500)), 100).expect("c1");
        assert_eq!(trades[0].price, clp(1_200), "no se cobra al comprador su límite, sino el precio que ya estaba");
    }

    #[test]
    fn a_partial_fill_leaves_the_rest_resting() {
        let mut book = OrderBook::new();
        book.submit("v1", "ana", Side::Sell, Some(clp(1_200)), 40).expect("v1");
        let trades = book.submit("c1", "beto", Side::Buy, Some(clp(1_200)), 100).expect("c1");
        assert_eq!(trades[0].quantity, 40);
        assert_eq!(book.bids()[0].quantity, 60, "lo que no se ejecutó espera en el libro");
        assert!(book.asks().is_empty());
    }

    #[test]
    fn the_book_never_stays_crossed() {
        // Si la mejor compra pagara igual o más que la mejor venta, habría una
        // ejecución que no ocurrió. La invariante que sostiene todo lo demás.
        let mut book = OrderBook::new();
        book.submit("v1", "ana", Side::Sell, Some(clp(1_200)), 100).expect("v1");
        book.submit("c1", "beto", Side::Buy, Some(clp(1_500)), 300).expect("c1");
        assert!(!book.is_crossed());
    }

    #[test]
    fn a_market_order_without_liquidity_is_refused_not_rested() {
        // No tiene precio al que esperar: dejarla en el libro sería inventarle
        // uno.
        let mut book = OrderBook::new();
        assert_eq!(book.submit("m1", "ana", Side::Buy, None, 100), Err(OrderError::NoLiquidity("m1".into())));
        assert!(book.bids().is_empty());
        // Y su identificador queda libre: la orden no llegó a existir.
        book.submit("v1", "beto", Side::Sell, Some(clp(1_200)), 100).expect("v1");
        assert!(book.submit("m1", "ana", Side::Buy, None, 100).is_ok());
    }

    #[test]
    fn a_market_order_sweeps_the_book_in_price_order() {
        let mut book = OrderBook::new();
        book.submit("v1", "ana", Side::Sell, Some(clp(1_300)), 50).expect("v1");
        book.submit("v2", "beto", Side::Sell, Some(clp(1_200)), 50).expect("v2");
        let trades = book.submit("m1", "caro", Side::Buy, None, 100).expect("m1");
        assert_eq!(trades.len(), 2);
        assert_eq!(trades[0].price, clp(1_200), "primero la más barata");
        assert_eq!(trades[1].price, clp(1_300));
    }

    #[test]
    fn nonsense_orders_are_refused() {
        let mut book = OrderBook::new();
        assert_eq!(book.submit("a", "x", Side::Buy, Some(clp(1_200)), 0), Err(OrderError::InvalidQuantity(0)));
        assert_eq!(book.submit("b", "x", Side::Buy, Some(clp(0)), 10), Err(OrderError::InvalidPrice(clp(0))));
        book.submit("c", "x", Side::Buy, Some(clp(1_200)), 10).expect("válida");
        assert_eq!(book.submit("c", "x", Side::Buy, Some(clp(1_200)), 10), Err(OrderError::Duplicate("c".into())));
    }

    #[test]
    fn a_cancelled_order_stops_taking_its_turn() {
        let mut book = OrderBook::new();
        book.submit("v1", "ana", Side::Sell, Some(clp(1_200)), 100).expect("v1");
        book.submit("v2", "beto", Side::Sell, Some(clp(1_200)), 100).expect("v2");
        assert!(book.cancel("v1"));
        let trades = book.submit("c1", "caro", Side::Buy, Some(clp(1_200)), 100).expect("c1");
        assert_eq!(trades[0].sell_order, "v2", "cancelada la primera, el turno pasa a la siguiente");
        assert!(!book.cancel("v1"), "cancelar dos veces no hace nada");
    }

    #[test]
    fn everything_that_was_traded_is_in_the_tape() {
        // Un mercado que no puede reconstruir sus ejecuciones no se puede
        // auditar, y la vigilancia de abuso sería imposible.
        let mut book = OrderBook::new();
        book.submit("v1", "ana", Side::Sell, Some(clp(1_200)), 50).expect("v1");
        book.submit("v2", "ana", Side::Sell, Some(clp(1_300)), 50).expect("v2");
        book.submit("c1", "beto", Side::Buy, Some(clp(1_300)), 100).expect("c1");
        assert_eq!(book.trades().len(), 2);
        assert_eq!(book.trades().iter().map(|t| t.quantity).sum::<i128>(), 100);
    }
}
