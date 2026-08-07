//! CM-10 · Compensación y liquidación.
//!
//! Entre cerrar el trato y cumplirlo hay un hueco, y ahí vive el riesgo de
//! principal: si yo entrego primero y tú no pagas, lo pierdo **todo**, no una
//! parte.
//!
//! Los dos controles del caso: el **netting** tiene que sumar cero —si no,
//! alguien estaría cobrando dinero inventado— y la liquidación es **atómica**:
//! las dos patas se mueven o no se mueve ninguna.

use super::{CaseReport, Check, Maturity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Trade {
    pub id: String,
    pub buyer: String,
    pub seller: String,
    pub instrument: String,
    pub units: i128,
    /// Efectivo total de la operación, en unidades mínimas.
    pub cash: i128,
}

/// Lo que cada participante debe o le deben, ya compensado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Obligation {
    pub participant: String,
    /// Negativo: paga. Positivo: cobra.
    pub cash: i128,
    /// Negativo: entrega. Positivo: recibe.
    pub units: i128,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Netting {
    pub obligations: Vec<Obligation>,
    /// Invariante: la suma de todo tiene que ser cero.
    pub nets_to_zero: bool,
}

/// Compensa todas las operaciones del ciclo.
///
/// Si A le debe 100 a B y B le debe 80 a A, solo se mueven 20. Reduce el dinero
/// que hace falta y con él el riesgo — a cambio de que el cálculo sea impecable.
pub fn net(trades: &[Trade]) -> Netting {
    let mut cash: BTreeMap<String, i128> = BTreeMap::new();
    let mut units: BTreeMap<String, i128> = BTreeMap::new();

    for trade in trades {
        *cash.entry(trade.buyer.clone()).or_insert(0) -= trade.cash;
        *cash.entry(trade.seller.clone()).or_insert(0) += trade.cash;
        *units.entry(trade.buyer.clone()).or_insert(0) += trade.units;
        *units.entry(trade.seller.clone()).or_insert(0) -= trade.units;
    }

    let participants: Vec<String> = cash.keys().cloned().collect();
    let obligations: Vec<Obligation> = participants
        .into_iter()
        .map(|participant| Obligation {
            cash: cash.get(&participant).copied().unwrap_or(0),
            units: units.get(&participant).copied().unwrap_or(0),
            participant,
        })
        .collect();

    let nets_to_zero = obligations.iter().map(|obligation| obligation.cash).sum::<i128>() == 0
        && obligations.iter().map(|obligation| obligation.units).sum::<i128>() == 0;

    Netting { obligations, nets_to_zero }
}

/// Lo que cada participante tiene realmente disponible.
#[derive(Debug, Clone, Default)]
pub struct Balances {
    pub cash: BTreeMap<String, i128>,
    pub units: BTreeMap<String, i128>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settlement {
    pub outcome: SettlementOutcome,
    pub reason: Option<String>,
    /// Los dos juntos son la prueba de la atomicidad: en una falla, los dos
    /// tienen que ser `false`.
    pub cash_moved: bool,
    pub instruments_moved: bool,
    pub penalty: i128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SettlementOutcome {
    Settled,
    Failed,
    /// Ya se había liquidado: repetir no la aplica dos veces.
    AlreadySettled,
}

/// Penalización por incumplir. Fija y conocida de antemano.
const PENALTY: i128 = 50_000;

/// Entrega contra pago, atómica.
///
/// Se comprueban **las dos patas antes de mover ninguna**. Comprobar mientras
/// se mueve es exactamente cómo alguien acaba habiendo entregado sin cobrar.
pub fn settle(trade: &Trade, balances: &mut Balances, already_settled: &mut Vec<String>) -> Settlement {
    if already_settled.contains(&trade.id) {
        return Settlement {
            outcome: SettlementOutcome::AlreadySettled,
            reason: Some("esta operación ya se liquidó".into()),
            cash_moved: false,
            instruments_moved: false,
            penalty: 0,
        };
    }

    let buyer_cash = balances.cash.get(&trade.buyer).copied().unwrap_or(0);
    let seller_units = balances.units.get(&trade.seller).copied().unwrap_or(0);

    if buyer_cash < trade.cash {
        return Settlement {
            outcome: SettlementOutcome::Failed,
            reason: Some(format!("{} no tiene fondos suficientes", trade.buyer)),
            cash_moved: false,
            instruments_moved: false,
            penalty: PENALTY,
        };
    }
    if seller_units < trade.units {
        return Settlement {
            outcome: SettlementOutcome::Failed,
            reason: Some(format!("{} no tiene los instrumentos", trade.seller)),
            cash_moved: false,
            instruments_moved: false,
            penalty: PENALTY,
        };
    }

    *balances.cash.entry(trade.buyer.clone()).or_insert(0) -= trade.cash;
    *balances.cash.entry(trade.seller.clone()).or_insert(0) += trade.cash;
    *balances.units.entry(trade.seller.clone()).or_insert(0) -= trade.units;
    *balances.units.entry(trade.buyer.clone()).or_insert(0) += trade.units;
    already_settled.push(trade.id.clone());

    Settlement {
        outcome: SettlementOutcome::Settled,
        reason: None,
        cash_moved: true,
        instruments_moved: true,
        penalty: 0,
    }
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn trade(id: &str, buyer: &str, seller: &str, units: i128, cash: i128) -> Trade {
    Trade { id: id.into(), buyer: buyer.into(), seller: seller.into(), instrument: "ACME-SIM".into(), units, cash }
}

fn balances(cash: &[(&str, i128)], units: &[(&str, i128)]) -> Balances {
    Balances {
        cash: cash.iter().map(|(name, value)| ((*name).to_string(), *value)).collect(),
        units: units.iter().map(|(name, value)| ((*name).to_string(), *value)).collect(),
    }
}

pub fn report() -> CaseReport {
    let mut checks = Vec::new();

    // 1. El netting suma cero.
    let netting = net(&[trade("t1", "P1", "P2", 150, 2_000_000), trade("t2", "P2", "P1", 50, 700_000)]);
    checks.push(Check::new(
        "dos operaciones cruzadas entre los mismos participantes",
        "si las obligaciones netas no suman cero, alguien cobraría dinero que nadie paga",
        "true",
        netting.nets_to_zero.to_string(),
    ));

    // 2. Compensar reduce lo que hay que mover.
    let bruto = 2_000_000 + 700_000;
    let neto: i128 = netting.obligations.iter().map(|obligation| obligation.cash.abs()).sum::<i128>() / 2;
    checks.push(Check::new(
        "lo que hay que mover antes y después de compensar",
        "compensar reduce el dinero en tránsito, y con él el riesgo de que alguien no aparezca",
        "menos",
        if neto < bruto { "menos" } else { "igual-o-mas" }.to_string(),
    ));

    // 3. Liquidación con las dos patas.
    let mut state = balances(&[("P1", 5_000_000), ("P2", 0)], &[("P1", 0), ("P2", 500)]);
    let mut settled = Vec::new();
    let result = settle(&trade("t3", "P1", "P2", 150, 2_000_000), &mut state, &mut settled);
    checks.push(Check::new(
        "el comprador tiene fondos y el vendedor tiene los instrumentos",
        "entrega contra pago: las dos patas se mueven juntas",
        "settled+true+true",
        format!("{:?}+{}+{}", result.outcome, result.cash_moved, result.instruments_moved).to_lowercase(),
    ));

    // 4. Comprador sin fondos: nada se mueve.
    let mut state = balances(&[("P1", 10), ("P2", 0)], &[("P1", 0), ("P2", 500)]);
    let result = settle(&trade("t4", "P1", "P2", 150, 2_000_000), &mut state, &mut Vec::new());
    checks.push(Check::new(
        "el comprador no tiene fondos",
        "la falla no puede dejar a nadie a medias: o las dos patas o ninguna",
        "failed+false+false",
        format!("{:?}+{}+{}", result.outcome, result.cash_moved, result.instruments_moved).to_lowercase(),
    ));

    // 5. Vendedor sin instrumentos.
    let mut state = balances(&[("P1", 5_000_000), ("P2", 0)], &[("P1", 0), ("P2", 0)]);
    let result = settle(&trade("t5", "P1", "P2", 150, 2_000_000), &mut state, &mut Vec::new());
    checks.push(Check::new(
        "el vendedor no tiene los instrumentos",
        "el riesgo es simétrico: falla igual por el otro lado",
        "failed+false",
        format!("{:?}+{}", result.outcome, result.cash_moved).to_lowercase(),
    ));

    // 6. Liquidación duplicada.
    let mut state = balances(&[("P1", 5_000_000), ("P2", 0)], &[("P1", 0), ("P2", 500)]);
    let operation = trade("t6", "P1", "P2", 150, 2_000_000);
    settle(&operation, &mut state, &mut settled);
    let repeated = settle(&operation, &mut state, &mut settled);
    checks.push(Check::new(
        "se procesa dos veces la misma operación",
        "repetir un mensaje pasa constantemente: la idempotencia evita entregar dos veces lo mismo",
        "alreadysettled",
        format!("{:?}", repeated.outcome).to_lowercase(),
    ));

    CaseReport::new("CM-10", "Compensación y liquidación", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn una_falla_no_toca_los_saldos() {
        let mut state = balances(&[("P1", 10)], &[("P2", 500)]);
        let antes = state.cash.clone();
        settle(&trade("t", "P1", "P2", 1, 1_000), &mut state, &mut Vec::new());
        assert_eq!(state.cash, antes, "una falla que mueve saldos no es atómica");
    }

    #[test]
    fn el_netting_de_una_lista_vacia_suma_cero() {
        assert!(net(&[]).nets_to_zero);
    }
}
