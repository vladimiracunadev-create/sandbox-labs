//! CM-09 · Vigilancia de abuso de mercado.
//!
//! Cada operación por separado puede ser perfectamente legal. El abuso está en
//! **el patrón**, y el patrón solo se ve mirando muchas juntas.
//!
//! Una alerta no es una conclusión: es el principio de un expediente. Por eso
//! todo hallazgo trae las cifras que lo produjeron, para poder juzgarlo en vez
//! de creerlo.

use super::{CaseReport, Check, Maturity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Place,
    Cancel,
    Execute,
}

/// Un evento del libro. El tiempo es un número de secuencia, no un reloj.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub sequence: u64,
    pub account: String,
    pub instrument: String,
    pub action: Action,
    pub price: i128,
    pub quantity: i128,
    /// Contraparte cuando la acción es una ejecución.
    #[serde(default)]
    pub counterparty: Option<String>,
    /// Si el evento cae en la subasta de cierre.
    #[serde(default)]
    pub closing_auction: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Alert {
    /// Compra y venta contra uno mismo: volumen falso.
    WashTrading { account: String, instrument: String, matches: usize },
    /// Órdenes grandes que se cancelan antes de ejecutarse.
    Spoofing { account: String, placed: usize, cancelled: usize, cancel_rate_pct: u32 },
    /// Varias capas de órdenes falsas a precios distintos.
    Layering { account: String, price_levels: usize },
    /// Concentración de actividad en la subasta de cierre.
    ClosingPriceManipulation { account: String, closing_share_pct: u32 },
}

impl Alert {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::WashTrading { .. } => "wash-trading",
            Self::Spoofing { .. } => "spoofing",
            Self::Layering { .. } => "layering",
            Self::ClosingPriceManipulation { .. } => "closing-price-manipulation",
        }
    }
}

/// Umbrales. Se calibran con escenarios etiquetados —donde se sabe de antemano
/// qué es abuso— para poder medir falsos positivos en vez de adivinarlos.
const SPOOFING_MIN_ORDERS: usize = 10;
const SPOOFING_CANCEL_RATE_PCT: u32 = 80;
const LAYERING_MIN_LEVELS: usize = 3;
const CLOSING_SHARE_PCT: u32 = 50;

/// Recorre la sesión y devuelve las alertas con sus cifras.
pub fn surveil(events: &[Event]) -> Vec<Alert> {
    let mut alerts = Vec::new();

    let mut placed: BTreeMap<&str, usize> = BTreeMap::new();
    let mut cancelled: BTreeMap<&str, usize> = BTreeMap::new();
    let mut levels: BTreeMap<&str, Vec<i128>> = BTreeMap::new();
    let mut wash: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    let mut total: BTreeMap<&str, usize> = BTreeMap::new();
    let mut closing: BTreeMap<&str, usize> = BTreeMap::new();

    for event in events {
        let account = event.account.as_str();
        *total.entry(account).or_insert(0) += 1;
        if event.closing_auction {
            *closing.entry(account).or_insert(0) += 1;
        }
        match event.action {
            Action::Place => {
                *placed.entry(account).or_insert(0) += 1;
                levels.entry(account).or_default().push(event.price);
            }
            Action::Cancel => *cancelled.entry(account).or_insert(0) += 1,
            Action::Execute => {
                if event.counterparty.as_deref() == Some(account) {
                    *wash.entry((account, event.instrument.as_str())).or_insert(0) += 1;
                }
            }
        }
    }

    for ((account, instrument), matches) in wash {
        alerts.push(Alert::WashTrading { account: account.to_string(), instrument: instrument.to_string(), matches });
    }

    for (account, placed_count) in &placed {
        let cancelled_count = cancelled.get(account).copied().unwrap_or(0);
        if *placed_count < SPOOFING_MIN_ORDERS {
            continue;
        }
        let rate = (cancelled_count * 100 / placed_count) as u32;
        if rate >= SPOOFING_CANCEL_RATE_PCT {
            alerts.push(Alert::Spoofing {
                account: (*account).to_string(),
                placed: *placed_count,
                cancelled: cancelled_count,
                cancel_rate_pct: rate,
            });

            // Layering es spoofing repartido en varios precios a la vez. Solo
            // se mira cuando ya hay cancelación masiva: por sí solo, poner
            // órdenes a varios precios es hacer mercado.
            let mut prices = levels.get(account).cloned().unwrap_or_default();
            prices.sort_unstable();
            prices.dedup();
            if prices.len() >= LAYERING_MIN_LEVELS {
                alerts.push(Alert::Layering { account: (*account).to_string(), price_levels: prices.len() });
            }
        }
    }

    for (account, closing_count) in closing {
        let all = total.get(account).copied().unwrap_or(0);
        if all == 0 {
            continue;
        }
        let share = (closing_count * 100 / all) as u32;
        if share >= CLOSING_SHARE_PCT && all >= 4 {
            alerts.push(Alert::ClosingPriceManipulation { account: account.to_string(), closing_share_pct: share });
        }
    }

    alerts
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn event(sequence: u64, account: &str, action: Action, price: i128) -> Event {
    Event {
        sequence,
        account: account.into(),
        instrument: "ACME-SIM".into(),
        action,
        price,
        quantity: 100,
        counterparty: None,
        closing_auction: false,
    }
}

fn kinds(alerts: &[Alert]) -> String {
    let mut names: Vec<&str> = alerts.iter().map(Alert::kind).collect();
    names.sort_unstable();
    names.dedup();
    names.join(",")
}

pub fn report() -> CaseReport {
    let mut checks = Vec::new();

    // 1. Actividad normal: poner y ejecutar.
    let normal: Vec<Event> = (0..12)
        .map(|index| event(index, "acc-1", if index % 2 == 0 { Action::Place } else { Action::Execute }, 10_000))
        .collect();
    checks.push(Check::new(
        "una cuenta que pone órdenes y las ejecuta",
        "sin línea base, cualquier detector alerta de todo",
        "",
        kinds(&surveil(&normal)),
    ));

    // 2. Wash trading: se ejecuta contra sí misma.
    let mut wash = event(1, "acc-2", Action::Execute, 10_000);
    wash.counterparty = Some("acc-2".into());
    checks.push(Check::new(
        "una cuenta ejecuta contra sí misma",
        "el volumen que crea no existe, y el precio que fija tampoco",
        "wash-trading",
        kinds(&surveil(&[wash])),
    ));

    // 3. Spoofing y layering: cancelación masiva a varios precios.
    let mut spoof = Vec::new();
    for index in 0..12 {
        spoof.push(event(index * 2, "acc-3", Action::Place, 10_000 + i128::from(index % 4) * 10));
        spoof.push(event(index * 2 + 1, "acc-3", Action::Cancel, 10_000));
    }
    checks.push(Check::new(
        "doce órdenes puestas a cuatro precios y canceladas casi todas",
        "poner sin intención de ejecutar mueve el precio sin arriesgar nada",
        "layering,spoofing",
        kinds(&surveil(&spoof)),
    ));

    // 4. Cancelar mucho sin capas no es layering.
    let mut single_level = Vec::new();
    for index in 0..12 {
        single_level.push(event(index * 2, "acc-4", Action::Place, 10_000));
        single_level.push(event(index * 2 + 1, "acc-4", Action::Cancel, 10_000));
    }
    checks.push(Check::new(
        "la misma cancelación masiva, pero toda a un solo precio",
        "layering es spoofing repartido en capas: sin capas, es otra cosa",
        "spoofing",
        kinds(&surveil(&single_level)),
    ));

    // 5. Pocas órdenes canceladas: no alerta.
    let few: Vec<Event> = (0..4)
        .map(|index| event(index, "acc-5", if index % 2 == 0 { Action::Place } else { Action::Cancel }, 10_000))
        .collect();
    checks.push(Check::new(
        "una cuenta que pone dos órdenes y cancela las dos",
        "cancelar es legítimo: sin volumen mínimo, el detector castigaría a cualquiera que se lo piense",
        "",
        kinds(&surveil(&few)),
    ));

    // 6. Concentración en el cierre.
    let mut closing: Vec<Event> = (0..2).map(|index| event(index, "acc-6", Action::Execute, 10_000)).collect();
    for index in 2..6 {
        let mut last = event(index, "acc-6", Action::Execute, 10_500);
        last.closing_auction = true;
        closing.push(last);
    }
    checks.push(Check::new(
        "dos tercios de la actividad de una cuenta caen en la subasta de cierre",
        "el precio de cierre valoriza carteras enteras: concentrarse ahí mueve mucho más de lo que se opera",
        "closing-price-manipulation",
        kinds(&surveil(&closing)),
    ));

    CaseReport::new("CM-09", "Vigilancia de abuso de mercado", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn toda_alerta_trae_sus_cifras() {
        let mut spoof = Vec::new();
        for index in 0..12 {
            spoof.push(event(index * 2, "acc", Action::Place, 10_000 + i128::from(index % 4)));
            spoof.push(event(index * 2 + 1, "acc", Action::Cancel, 10_000));
        }
        let alerts = surveil(&spoof);
        let spoofing = alerts.iter().find(|alert| matches!(alert, Alert::Spoofing { .. })).expect("alerta");
        if let Alert::Spoofing { cancel_rate_pct, placed, .. } = spoofing {
            assert!(*placed >= SPOOFING_MIN_ORDERS);
            assert!(*cancel_rate_pct >= SPOOFING_CANCEL_RATE_PCT);
        }
    }

    #[test]
    fn una_sesion_vacia_no_alerta() {
        assert!(surveil(&[]).is_empty());
    }
}
