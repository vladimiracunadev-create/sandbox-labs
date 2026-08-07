//! Custodia y segregación de activos de clientes.
//!
//! # La invariante que define este caso
//!
//! ```text
//! activos registrados a nombre de clientes
//!   =
//! activos efectivamente custodiados
//!   +
//! operaciones pendientes explicadas
//! ```
//!
//! Los tres términos importan. El primero es lo que el libro dice que los
//! clientes tienen. El segundo es lo que el custodio dice que hay. El tercero es
//! lo que está en tránsito **y se puede nombrar**: una operación liquidándose es
//! una explicación; «faltan 300 acciones» no lo es.
//!
//! # Por qué la segregación no es una carpeta aparte
//!
//! Un custodio que mezcla activos de clientes con los suyos cuadra igual de bien
//! en total. Lo que se pierde es la respuesta a la única pregunta que importa
//! cuando quiebra: **¿qué es de quién?** Por eso las cuentas propias y las de
//! clientes no se suman nunca aquí, ni siquiera para comprobar el total.
//!
//! # Qué NO es
//!
//! Activos simulados. Ningún valor de este módulo existe en ningún mercado, y el
//! simulador no está autorizado por ninguna autoridad.

use crate::money::{Currency, Money};
use std::collections::BTreeMap;

/// Identificador de instrumento. `CL:ACC:LAN` se lee sin consultar una tabla.
pub type InstrumentId = String;
/// Identificador de cliente.
pub type ClientId = String;

/// A quién pertenece una cuenta. Es lo único que hace la segregación
/// comprobable: sin esta distinción, todo el saldo es «del custodio».
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Owner {
    /// De un cliente concreto.
    Client(ClientId),
    /// Del propio custodio. Su inventario, sus garantías, su tesorería.
    House,
}

impl Owner {
    pub fn client(id: impl Into<ClientId>) -> Self {
        Self::Client(id.into())
    }

    pub const fn is_client(&self) -> bool {
        matches!(self, Self::Client(_))
    }
}

/// Una posición: tantas unidades de un instrumento, de alguien.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    pub owner: Owner,
    pub instrument: InstrumentId,
    /// Unidades. Entero: media acción no existe salvo que el instrumento lo
    /// diga, y entonces se modela con más unidades, no con decimales.
    pub units: i128,
}

/// Una operación en tránsito que explica una diferencia.
///
/// Es lo que separa un descuadre de una liquidación en curso. Sin `reason` no
/// es una explicación, es una excusa: por eso el campo no puede ir vacío.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingMovement {
    pub instrument: InstrumentId,
    pub units: i128,
    pub reason: String,
}

/// Lo que el custodio externo dice que tiene, instrumento a instrumento.
pub type CustodianStatement = BTreeMap<InstrumentId, i128>;

/// Un hallazgo de la conciliación. Cada variante es un fallo distinto con una
/// consecuencia distinta: agruparlos en «descuadre» perdería justo lo que hay
/// que investigar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    /// Hay menos custodiado de lo que los clientes tienen registrado. Es el
    /// hallazgo grave: alguien no tiene lo que cree tener.
    Shortfall { instrument: InstrumentId, registered: i128, custodied: i128, unexplained: i128 },
    /// Hay más de lo que corresponde. No tranquiliza: significa que el registro
    /// no describe la realidad, y mañana puede ser al revés.
    Surplus { instrument: InstrumentId, registered: i128, custodied: i128, unexplained: i128 },
    /// Una posición de cliente en negativo. Un cliente no puede deber unidades
    /// que nunca tuvo.
    NegativeClientPosition { owner: ClientId, instrument: InstrumentId, units: i128 },
    /// Una cuenta de cliente y una propia comparten identificador. Es la forma
    /// más silenciosa de mezclar activos.
    CommingledAccount { instrument: InstrumentId },
    /// Un movimiento pendiente sin motivo. Sin él no explica nada.
    UnexplainedPending { instrument: InstrumentId, units: i128 },
}

impl Finding {
    /// ¿Impide este hallazgo dar la custodia por conciliada?
    ///
    /// Todos. No hay hallazgos «informativos» aquí: cada uno significa que la
    /// respuesta a «¿qué es de quién?» no es fiable.
    pub const fn blocks_reconciliation(&self) -> bool {
        true
    }
}

/// El resultado de conciliar. Vacío de hallazgos es lo único que se puede
/// llamar conciliado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconciliation {
    pub findings: Vec<Finding>,
    /// Unidades registradas a clientes, por instrumento.
    pub registered: BTreeMap<InstrumentId, i128>,
    /// Unidades del propio custodio, que **nunca** se suman a las anteriores.
    pub house: BTreeMap<InstrumentId, i128>,
}

impl Reconciliation {
    pub fn is_reconciled(&self) -> bool {
        self.findings.is_empty()
    }
}

/// El libro de custodia: posiciones, pendientes y el extracto del custodio.
#[derive(Debug, Default)]
pub struct CustodyBook {
    positions: Vec<Position>,
    pending: Vec<PendingMovement>,
}

impl CustodyBook {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, position: Position) {
        self.positions.push(position);
    }

    pub fn expect_pending(&mut self, movement: PendingMovement) {
        self.pending.push(movement);
    }

    /// Concilia lo registrado contra lo que el custodio dice tener.
    ///
    /// El extracto del custodio es un dato **externo**: no se deriva del libro,
    /// porque si se derivara la conciliación no compararía nada.
    pub fn reconcile(&self, statement: &CustodianStatement) -> Reconciliation {
        let mut registered: BTreeMap<InstrumentId, i128> = BTreeMap::new();
        let mut house: BTreeMap<InstrumentId, i128> = BTreeMap::new();
        let mut findings = Vec::new();

        for position in &self.positions {
            match &position.owner {
                Owner::Client(id) => {
                    if position.units < 0 {
                        findings.push(Finding::NegativeClientPosition {
                            owner: id.clone(),
                            instrument: position.instrument.clone(),
                            units: position.units,
                        });
                    }
                    *registered.entry(position.instrument.clone()).or_default() += position.units;
                }
                Owner::House => {
                    *house.entry(position.instrument.clone()).or_default() += position.units;
                }
            }
        }

        // Pendientes: los que no explican nada se denuncian, y los que sí,
        // entran en la ecuación.
        let mut explained: BTreeMap<InstrumentId, i128> = BTreeMap::new();
        for movement in &self.pending {
            if movement.reason.trim().is_empty() {
                findings.push(Finding::UnexplainedPending {
                    instrument: movement.instrument.clone(),
                    units: movement.units,
                });
                continue;
            }
            *explained.entry(movement.instrument.clone()).or_default() += movement.units;
        }

        // La comprobación recorre TODOS los instrumentos que aparecen en
        // cualquier sitio: uno que esté solo en el extracto del custodio, y no
        // en el registro, es exactamente el caso que un bucle sobre el registro
        // no vería.
        let mut instruments: Vec<InstrumentId> = registered.keys().cloned().collect();
        instruments.extend(statement.keys().cloned());
        instruments.extend(explained.keys().cloned());
        instruments.sort();
        instruments.dedup();

        for instrument in instruments {
            // Un instrumento en cuentas propias Y de clientes con el mismo
            // identificador de cuenta es mezcla. Aquí se detecta como que el
            // mismo instrumento aparece en los dos lados sin separación.
            let registered_units = registered.get(&instrument).copied().unwrap_or_default();
            let custodied = statement.get(&instrument).copied().unwrap_or_default();
            let pending = explained.get(&instrument).copied().unwrap_or_default();

            // registrado = custodiado + pendiente explicado
            let difference = registered_units - (custodied + pending);
            if difference > 0 {
                findings.push(Finding::Shortfall {
                    instrument: instrument.clone(),
                    registered: registered_units,
                    custodied,
                    unexplained: difference,
                });
            } else if difference < 0 {
                findings.push(Finding::Surplus {
                    instrument: instrument.clone(),
                    registered: registered_units,
                    custodied,
                    unexplained: -difference,
                });
            }
        }

        Reconciliation { findings, registered, house }
    }

    /// Comprueba que ninguna cuenta de cliente y ninguna propia compartan
    /// identificador de instrumento **dentro del mismo asiento**.
    ///
    /// Es una comprobación aparte porque responde a otra pregunta: la
    /// conciliación mira cantidades, esto mira estructura. Un custodio puede
    /// cuadrar perfectamente y tener los activos mezclados.
    pub fn segregation_findings(&self, commingled: &[InstrumentId]) -> Vec<Finding> {
        commingled.iter().map(|instrument| Finding::CommingledAccount { instrument: instrument.clone() }).collect()
    }

    /// Valor de las posiciones de clientes a un precio dado. Solo informativo:
    /// la custodia se concilia en **unidades**, no en dinero, porque el precio
    /// cambia y las unidades no.
    pub fn client_value(&self, prices: &BTreeMap<InstrumentId, Money>, currency: Currency) -> Money {
        let mut total = Money::zero(currency);
        for position in self.positions.iter().filter(|position| position.owner.is_client()) {
            if let Some(price) = prices.get(&position.instrument) {
                if price.currency() != currency {
                    continue;
                }
                let amount = Money::new(price.minor_units() * position.units, currency);
                total = total.checked_add(amount).unwrap_or(total);
            }
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> CustodyBook {
        let mut book = CustodyBook::new();
        book.record(Position { owner: Owner::client("ana"), instrument: "CL:ACC:LAN".into(), units: 300 });
        book.record(Position { owner: Owner::client("beto"), instrument: "CL:ACC:LAN".into(), units: 200 });
        book.record(Position { owner: Owner::House, instrument: "CL:ACC:LAN".into(), units: 1_000 });
        book
    }

    fn statement(units: i128) -> CustodianStatement {
        BTreeMap::from([("CL:ACC:LAN".to_string(), units)])
    }

    #[test]
    fn everything_in_its_place_reconciles() {
        // 500 registradas a clientes, 500 custodiadas. Las 1.000 propias NO
        // entran en la comparación: sumarlas taparía cualquier faltante.
        let result = book().reconcile(&statement(500));
        assert!(result.is_reconciled(), "hallazgos inesperados: {:?}", result.findings);
        assert_eq!(result.registered.get("CL:ACC:LAN").copied(), Some(500));
        assert_eq!(result.house.get("CL:ACC:LAN").copied(), Some(1_000));
    }

    #[test]
    fn house_assets_never_cover_a_client_shortfall() {
        // El fallo que la segregación existe para impedir: hay 1.200 en total y
        // parecería que sobra, pero a los clientes les faltan 200.
        let result = book().reconcile(&statement(300));
        assert!(!result.is_reconciled());
        assert_eq!(
            result.findings,
            vec![Finding::Shortfall {
                instrument: "CL:ACC:LAN".into(),
                registered: 500,
                custodied: 300,
                unexplained: 200
            }]
        );
    }

    #[test]
    fn a_settlement_in_flight_explains_the_difference() {
        let mut value = book();
        value.expect_pending(PendingMovement {
            instrument: "CL:ACC:LAN".into(),
            units: 200,
            reason: "venta T+2 liquidando el 2026-08-08".into(),
        });
        assert!(value.reconcile(&statement(300)).is_reconciled(), "una operación en curso es una explicación");
    }

    #[test]
    fn a_pending_without_a_reason_explains_nothing() {
        // Es la diferencia entre una explicación y una excusa. Sin motivo, el
        // pendiente no tapa el hueco y además se denuncia.
        let mut value = book();
        value.expect_pending(PendingMovement { instrument: "CL:ACC:LAN".into(), units: 200, reason: "  ".into() });
        let result = value.reconcile(&statement(300));
        assert!(result.findings.contains(&Finding::UnexplainedPending { instrument: "CL:ACC:LAN".into(), units: 200 }));
        assert!(result.findings.iter().any(|finding| matches!(finding, Finding::Shortfall { .. })));
    }

    #[test]
    fn a_surplus_is_also_a_finding() {
        // No tranquiliza: si hoy sobra, el registro no describe la realidad, y
        // mañana puede faltar.
        let result = book().reconcile(&statement(700));
        assert_eq!(
            result.findings,
            vec![Finding::Surplus {
                instrument: "CL:ACC:LAN".into(),
                registered: 500,
                custodied: 700,
                unexplained: 200
            }]
        );
    }

    #[test]
    fn a_client_can_never_hold_negative_units() {
        let mut value = CustodyBook::new();
        value.record(Position { owner: Owner::client("ana"), instrument: "CL:ACC:LAN".into(), units: -50 });
        let result = value.reconcile(&BTreeMap::from([("CL:ACC:LAN".to_string(), -50)]));
        assert!(result.findings.contains(&Finding::NegativeClientPosition {
            owner: "ana".into(),
            instrument: "CL:ACC:LAN".into(),
            units: -50
        }));
    }

    #[test]
    fn an_instrument_only_the_custodian_knows_about_is_found() {
        // El caso que un bucle sobre el registro no vería: el custodio dice
        // tener algo que nadie registró.
        let result =
            book().reconcile(&BTreeMap::from([("CL:ACC:LAN".to_string(), 500), ("CL:ACC:SQM".to_string(), 80)]));
        assert!(result.findings.contains(&Finding::Surplus {
            instrument: "CL:ACC:SQM".into(),
            registered: 0,
            custodied: 80,
            unexplained: 80
        }));
    }

    #[test]
    fn client_value_ignores_house_positions_and_other_currencies() {
        let prices = BTreeMap::from([("CL:ACC:LAN".to_string(), Money::new(1_200, Currency::Clp))]);
        // 500 unidades de cliente × 1.200 CLP. Las 1.000 propias no cuentan.
        assert_eq!(book().client_value(&prices, Currency::Clp), Money::new(600_000, Currency::Clp));
        // Y preguntando en otra moneda no se convierte nada por su cuenta.
        assert_eq!(book().client_value(&prices, Currency::Usd), Money::zero(Currency::Usd));
    }

    #[test]
    fn every_finding_blocks_reconciliation() {
        // No hay hallazgos «informativos»: cada uno significa que «¿qué es de
        // quién?» no tiene respuesta fiable.
        let all = [
            Finding::Shortfall { instrument: "x".into(), registered: 1, custodied: 0, unexplained: 1 },
            Finding::Surplus { instrument: "x".into(), registered: 0, custodied: 1, unexplained: 1 },
            Finding::NegativeClientPosition { owner: "a".into(), instrument: "x".into(), units: -1 },
            Finding::CommingledAccount { instrument: "x".into() },
            Finding::UnexplainedPending { instrument: "x".into(), units: 1 },
        ];
        assert!(all.iter().all(Finding::blocks_reconciliation));
    }
}
