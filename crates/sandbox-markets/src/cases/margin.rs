//! CM-18 · Margen, garantías y riesgo.
//!
//! El margen permite operar por más de lo que se tiene, e impide que la pérdida
//! de uno se convierta en pérdida de otro.
//!
//! Lo que hace este caso distinto de todos los demás: **la acción del sistema
//! cambia las condiciones que la provocaron**. Liquidar por la fuerza hunde el
//! precio, y el precio hundido genera más llamadas de margen. Simularlo sin ese
//! bucle es simular otra cosa, y sale siempre optimista.

use super::{CaseReport, Check, Maturity};
use serde::{Deserialize, Serialize};

/// Tipo de garantía. El descuento no es el mismo para todas: una acción puede
/// caer justo el día en que hace falta venderla; el efectivo no.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CollateralKind {
    Cash,
    GovernmentBond,
    Equity,
}

impl CollateralKind {
    /// Descuento en puntos porcentuales.
    pub const fn haircut_pct(self) -> i128 {
        match self {
            Self::Cash => 0,
            Self::GovernmentBond => 5,
            Self::Equity => 25,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collateral {
    pub kind: CollateralKind,
    pub value: i128,
}

impl Collateral {
    /// Lo que vale de verdad como garantía.
    pub fn after_haircut(&self) -> i128 {
        self.value * (100 - self.kind.haircut_pct()) / 100
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    pub id: String,
    /// Unidades del instrumento.
    pub units: i128,
    pub collateral: Vec<Collateral>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarginCall {
    pub participant: String,
    pub shortfall: i128,
    /// Si no se atiende, se liquida.
    pub forced_liquidation: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayResult {
    pub price: i128,
    pub calls: Vec<MarginCall>,
    /// Unidades liquidadas por la fuerza ese día.
    pub liquidated_units: i128,
    /// Precio después del impacto de esas liquidaciones.
    pub price_after_impact: i128,
}

/// Margen exigido: un porcentaje de la exposición.
const INITIAL_MARGIN_PCT: i128 = 20;

/// Cuánto mueve el precio liquidar una unidad, en milésimas de punto
/// porcentual. Pequeño por unidad y grande cuando todos liquidan a la vez —
/// que es exactamente el fenómeno que interesa.
const IMPACT_PER_UNIT_MILLI_PCT: i128 = 2;

/// Margen exigido a un participante al precio dado.
pub fn required_margin(participant: &Participant, price: i128) -> i128 {
    participant.units * price * INITIAL_MARGIN_PCT / 100
}

/// Garantía disponible tras aplicar los descuentos.
pub fn available_collateral(participant: &Participant) -> i128 {
    participant.collateral.iter().map(Collateral::after_haircut).sum()
}

/// Un día de mercado: revaloriza, llama a margen y liquida a quien no atiende.
///
/// `honoured` son los participantes que sí aportan lo que se les pide. Los
/// demás se liquidan, y esa liquidación mueve el precio.
pub fn run_day(participants: &[Participant], price: i128, honoured: &[&str]) -> DayResult {
    let mut calls = Vec::new();
    let mut liquidated_units = 0;

    for participant in participants {
        let required = required_margin(participant, price);
        let available = available_collateral(participant);
        if available >= required {
            continue;
        }
        let attends = honoured.contains(&participant.id.as_str());
        if !attends {
            liquidated_units += participant.units;
        }
        calls.push(MarginCall {
            participant: participant.id.clone(),
            shortfall: required - available,
            forced_liquidation: !attends,
        });
    }

    // El precio cae en proporción a lo liquidado. Este es el bucle del caso.
    let impact = liquidated_units * IMPACT_PER_UNIT_MILLI_PCT;
    let price_after_impact = (price * (100_000 - impact.min(90_000)) / 100_000).max(1);

    DayResult { price, calls, liquidated_units, price_after_impact }
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn participant(id: &str, units: i128, collateral: Vec<Collateral>) -> Participant {
    Participant { id: id.into(), units, collateral }
}

fn cash(value: i128) -> Collateral {
    Collateral { kind: CollateralKind::Cash, value }
}

fn equity(value: i128) -> Collateral {
    Collateral { kind: CollateralKind::Equity, value }
}

pub fn report() -> CaseReport {
    let mut checks = Vec::new();

    // 1. El haircut cambia lo que vale una garantía.
    checks.push(Check::new(
        "un millón en efectivo y un millón en acciones como garantía",
        "una acción puede caer justo el día en que hay que venderla; el efectivo no",
        "1000000+750000",
        format!("{}+{}", cash(1_000_000).after_haircut(), equity(1_000_000).after_haircut()),
    ));

    // 2. Garantía suficiente: sin llamada.
    let holgado = vec![participant("P1", 1_000, vec![cash(3_000_000)])];
    let day = run_day(&holgado, 10_000, &[]);
    checks.push(Check::new(
        "un participante con garantía de sobra",
        "el caso normal no genera llamadas, o el sistema pediría margen todos los días a todos",
        "0",
        day.calls.len().to_string(),
    ));

    // 3. Garantía justa que deja de serlo al aplicar el haircut.
    let justo = vec![participant("P2", 1_000, vec![equity(2_500_000)])];
    let day = run_day(&justo, 10_000, &[]);
    checks.push(Check::new(
        "garantía en acciones que cubre el margen justo antes del haircut",
        "el haircut no es un detalle contable: decide si hay llamada o no",
        "1",
        day.calls.len().to_string(),
    ));

    // 4. Quien atiende la llamada no se liquida.
    let day = run_day(&justo, 10_000, &["P2"]);
    checks.push(Check::new(
        "el participante aporta lo que se le pide",
        "atender la llamada es lo que separa un mal día de una liquidación forzada",
        "false+0",
        format!("{}+{}", day.calls[0].forced_liquidation, day.liquidated_units),
    ));

    // 5. Quien no atiende, se liquida y el precio se mueve.
    let day = run_day(&justo, 10_000, &[]);
    checks.push(Check::new(
        "el participante no aporta y se liquidan sus 1 000 unidades",
        "liquidar mueve el precio: sin ese bucle la simulación sale siempre optimista",
        "1000+baja",
        format!("{}+{}", day.liquidated_units, if day.price_after_impact < day.price { "baja" } else { "igual" }),
    ));

    // 6. Varios liquidando a la vez mueven el precio mucho más.
    let muchos: Vec<Participant> =
        (0..5).map(|index| participant(&format!("P{index}"), 1_000, vec![equity(1_000_000)])).collect();
    let day = run_day(&muchos, 10_000, &[]);
    let uno = run_day(&justo, 10_000, &[]);
    checks.push(Check::new(
        "cinco participantes liquidan el mismo día",
        "la espiral no es que cada uno venda mucho: es que todos venden a la vez",
        "mayor",
        if day.price - day.price_after_impact > uno.price - uno.price_after_impact { "mayor" } else { "igual-o-menor" }
            .to_string(),
    ));

    CaseReport::new("CM-18", "Margen, garantías y riesgo", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn el_precio_nunca_llega_a_cero() {
        let enormes: Vec<Participant> =
            (0..100).map(|index| participant(&format!("P{index}"), 1_000_000, vec![cash(1)])).collect();
        let day = run_day(&enormes, 10_000, &[]);
        assert!(day.price_after_impact >= 1, "un precio cero rompería todos los cálculos aguas abajo");
    }

    #[test]
    fn el_efectivo_no_tiene_descuento() {
        assert_eq!(CollateralKind::Cash.haircut_pct(), 0);
        assert!(CollateralKind::Equity.haircut_pct() > CollateralKind::GovernmentBond.haircut_pct());
    }
}
