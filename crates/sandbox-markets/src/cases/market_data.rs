//! CM-16 · Integridad de datos de mercado.
//!
//! El caso más aburrido y el más fundacional: todo lo demás se apoya en los
//! precios. Un precio cero aguas arriba dispara márgenes y liquidaciones
//! forzadas aguas abajo, y el sistema entero funciona perfectamente mientras lo
//! hace.
//!
//! Un número suelto no es un precio. Un precio trae **procedencia**: moneda,
//! instrumento, proveedor, marca de tiempo y qué eventos corporativos tiene ya
//! aplicados.

use super::{CaseReport, Check, Maturity};
use crate::money::Currency;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    pub instrument: String,
    /// En unidades mínimas, con su moneda **dentro** y no al lado.
    pub price: i128,
    pub currency: Currency,
    pub provider: String,
    /// Segundo simulado. Reloj simulado para que el caso sea reproducible.
    pub timestamp: u64,
    /// Eventos corporativos ya aplicados a este precio.
    #[serde(default)]
    pub corporate_actions_applied: Vec<String>,
}

/// Lo que el catálogo dice que es cada instrumento.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instrument {
    pub id: String,
    pub currency: Currency,
    /// Evento corporativo pendiente de aplicar, si lo hay.
    #[serde(default)]
    pub pending_corporate_action: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Finding {
    ZeroPrice {
        instrument: String,
        provider: String,
    },
    CurrencyMismatch {
        instrument: String,
        expected: Currency,
        received: Currency,
    },
    FutureTimestamp {
        instrument: String,
        timestamp: u64,
        now: u64,
    },
    StaleData {
        instrument: String,
        age_seconds: u64,
        threshold: u64,
    },
    /// Variación grande **explicada** por un evento sin aplicar. No es un
    /// precio malo: es un precio bueno mal comparado.
    UnappliedCorporateAction {
        instrument: String,
        action: String,
    },
    /// Variación grande sin explicación.
    AnomalousMove {
        instrument: String,
        previous: i128,
        current: i128,
    },
}

impl Finding {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::ZeroPrice { .. } => "zero-price",
            Self::CurrencyMismatch { .. } => "currency-mismatch",
            Self::FutureTimestamp { .. } => "future-timestamp",
            Self::StaleData { .. } => "stale-data",
            Self::UnappliedCorporateAction { .. } => "unapplied-corporate-action",
            Self::AnomalousMove { .. } => "anomalous-move",
        }
    }
}

/// Segundos tras los cuales un dato deja de considerarse actual.
pub const STALE_THRESHOLD_SECONDS: u64 = 300;

/// Variación porcentual a partir de la cual conviene mirar.
const ANOMALOUS_MOVE_PCT: i128 = 30;

/// Valida un dato entrante contra el catálogo y el precio anterior.
///
/// El orden importa en un sitio: **la variación anómala se mira al final**, y
/// antes se comprueba si hay un evento corporativo pendiente que la explique.
/// Alertar de una caída del 50 % que en realidad es un split es la forma más
/// rápida de que nadie haga caso de las alertas.
pub fn validate(quote: &Quote, instrument: &Instrument, previous: Option<i128>, now: u64) -> Vec<Finding> {
    let mut findings = Vec::new();

    if quote.price <= 0 {
        findings.push(Finding::ZeroPrice { instrument: quote.instrument.clone(), provider: quote.provider.clone() });
        return findings; // Sin precio válido, el resto de comprobaciones no dice nada.
    }
    if quote.currency != instrument.currency {
        findings.push(Finding::CurrencyMismatch {
            instrument: quote.instrument.clone(),
            expected: instrument.currency,
            received: quote.currency,
        });
    }
    if quote.timestamp > now {
        findings.push(Finding::FutureTimestamp {
            instrument: quote.instrument.clone(),
            timestamp: quote.timestamp,
            now,
        });
    } else {
        let age = now - quote.timestamp;
        if age > STALE_THRESHOLD_SECONDS {
            findings.push(Finding::StaleData {
                instrument: quote.instrument.clone(),
                age_seconds: age,
                threshold: STALE_THRESHOLD_SECONDS,
            });
        }
    }

    if let Some(previous_price) = previous {
        if previous_price > 0 {
            let change = (quote.price - previous_price).abs() * 100 / previous_price;
            if change >= ANOMALOUS_MOVE_PCT {
                match &instrument.pending_corporate_action {
                    Some(action) if !quote.corporate_actions_applied.contains(action) => {
                        findings.push(Finding::UnappliedCorporateAction {
                            instrument: quote.instrument.clone(),
                            action: action.clone(),
                        });
                    }
                    _ => findings.push(Finding::AnomalousMove {
                        instrument: quote.instrument.clone(),
                        previous: previous_price,
                        current: quote.price,
                    }),
                }
            }
        }
    }

    findings
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn instrument(pending: Option<&str>) -> Instrument {
    Instrument { id: "ACME-SIM".into(), currency: Currency::Clp, pending_corporate_action: pending.map(str::to_string) }
}

fn quote(price: i128, currency: Currency, timestamp: u64) -> Quote {
    Quote {
        instrument: "ACME-SIM".into(),
        price,
        currency,
        provider: "proveedor-sim-A".into(),
        timestamp,
        corporate_actions_applied: Vec::new(),
    }
}

fn kinds(findings: &[Finding]) -> String {
    let mut names: Vec<&str> = findings.iter().map(Finding::kind).collect();
    names.sort_unstable();
    names.dedup();
    names.join(",")
}

pub fn report() -> CaseReport {
    let now = 10_000;

    // 1. Dato correcto.
    let mut checks = vec![Check::new(
        "un precio en la moneda correcta, reciente y sin salto",
        "sin línea base, cualquier validación es un filtro que rechaza todo",
        "",
        kinds(&validate(&quote(10_250, Currency::Clp, now - 10), &instrument(None), Some(10_200), now)),
    )];

    // 2. Precio cero.
    checks.push(Check::new(
        "llega un precio cero",
        "un cero aguas arriba valoriza carteras a cero y dispara liquidaciones forzadas",
        "zero-price",
        kinds(&validate(&quote(0, Currency::Clp, now - 10), &instrument(None), Some(10_200), now)),
    ));

    // 3. Moneda incorrecta.
    checks.push(Check::new(
        "un instrumento en pesos llega marcado en dólares",
        "la moneda va dentro del precio: un importe sin moneda no es un importe",
        "currency-mismatch",
        kinds(&validate(&quote(10_250, Currency::Usd, now - 10), &instrument(None), Some(10_200), now)),
    ));

    // 4. Timestamp futuro.
    checks.push(Check::new(
        "un dato con marca de tiempo en el futuro",
        "un dato del futuro gana siempre la comparación y bloquea las actualizaciones legítimas",
        "future-timestamp",
        kinds(&validate(&quote(10_250, Currency::Clp, now + 500), &instrument(None), Some(10_200), now)),
    ));

    // 5. Dato obsoleto: se marca, no se oculta.
    checks.push(Check::new(
        "el último precio es de hace una hora",
        "servir un dato viejo como si fuera actual es peor que decir que no hay dato",
        "stale-data",
        kinds(&validate(&quote(10_250, Currency::Clp, now - 3_600), &instrument(None), Some(10_200), now)),
    ));

    // 6. Caída del 50 % explicada por un split sin aplicar.
    checks.push(Check::new(
        "el precio cae a la mitad y hay un split pendiente de aplicar",
        "el precio es correcto y la caída es falsa: alertar aquí entrena a la gente a ignorar alertas",
        "unapplied-corporate-action",
        kinds(&validate(&quote(5_100, Currency::Clp, now - 10), &instrument(Some("split-2026-05")), Some(10_200), now)),
    ));

    // 7. La misma caída sin evento que la explique.
    checks.push(Check::new(
        "el precio cae a la mitad y no hay ningún evento corporativo",
        "la misma variación significa cosas opuestas según haya o no un evento detrás",
        "anomalous-move",
        kinds(&validate(&quote(5_100, Currency::Clp, now - 10), &instrument(None), Some(10_200), now)),
    ));

    CaseReport::new("CM-16", "Integridad de datos de mercado", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn un_precio_cero_corta_el_resto_de_comprobaciones() {
        // Sin precio válido, decir además que la moneda no cuadra es ruido.
        let findings = validate(&quote(0, Currency::Usd, 99_999), &instrument(None), Some(100), 10);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn sin_precio_anterior_no_hay_variacion_que_juzgar() {
        let findings = validate(&quote(10_000, Currency::Clp, 9_990), &instrument(None), None, 10_000);
        assert!(findings.is_empty());
    }
}
