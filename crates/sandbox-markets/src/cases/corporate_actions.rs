//! CM-17 · Eventos corporativos.
//!
//! La principal fuente de descuadres que **no viene de una operación**. El
//! sistema está preparado para «alguien compró, alguien vendió»; un split no
//! encaja en ese molde y hay que aplicarlo a todas las posiciones a la vez.
//!
//! Dos detalles que producen la mayoría de los errores: cuenta **quién tenía el
//! instrumento en la fecha de registro**, no quien lo tiene el día del pago; y
//! las **fracciones** —un split 3:2 sobre 5 unidades da 7,5— se compensan en
//! efectivo, porque media unidad no existe y redondear en silencio descuadra.

use super::{CaseReport, Check, Maturity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// `from:to` — 1:2 duplica las unidades y parte el precio.
    Split,
    /// Efectivo por unidad.
    Dividend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorporateAction {
    pub id: String,
    pub instrument: String,
    pub kind: Kind,
    pub ratio_from: i128,
    pub ratio_to: i128,
    /// Efectivo por unidad, para dividendos.
    #[serde(default)]
    pub cash_per_unit: i128,
    /// Día en que se mira quién tiene qué. **La fecha que cuenta.**
    pub record_day: u32,
    pub payment_day: u32,
}

/// Una posición con su historia: cuántas unidades tenía en cada día.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub holder: String,
    /// Unidades por día simulado. El snapshot se toma del día de registro.
    pub units_by_day: BTreeMap<u32, i128>,
}

impl Position {
    /// Unidades en un día: el último valor conocido en ese día o antes.
    pub fn units_on(&self, day: u32) -> i128 {
        self.units_by_day.range(..=day).next_back().map(|(_, units)| *units).unwrap_or(0)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    pub positions_affected: usize,
    pub units_before: i128,
    pub units_after: i128,
    pub cash_paid: i128,
    pub cash_in_lieu: i128,
    /// Un split no hace más rico a nadie: el valor total no cambia.
    pub total_value_changed: bool,
    /// O a todos, o a ninguno.
    pub atomic: bool,
}

/// Aplica el evento a todas las posiciones, de una vez.
///
/// Devuelve `None` si el evento no es aplicable —una razón imposible, un
/// dividendo negativo—: aplicarlo a medias sería peor que no aplicarlo.
pub fn apply(action: &CorporateAction, positions: &[Position], price_before: i128) -> Option<Applied> {
    if action.ratio_from <= 0 || action.ratio_to <= 0 || action.cash_per_unit < 0 {
        return None;
    }

    let mut units_before = 0;
    let mut units_after = 0;
    let mut cash_paid = 0;
    let mut cash_in_lieu = 0;
    let mut affected = 0;

    for position in positions {
        // El snapshot se toma en la fecha de registro, no hoy.
        let held = position.units_on(action.record_day);
        if held == 0 {
            continue;
        }
        affected += 1;
        units_before += held;

        match action.kind {
            Kind::Split => {
                let converted = held * action.ratio_to;
                let whole = converted / action.ratio_from;
                let remainder = converted % action.ratio_from;
                units_after += whole;
                if remainder > 0 {
                    // Media unidad no existe: se paga su valor.
                    let price_after = price_before * action.ratio_from / action.ratio_to;
                    cash_in_lieu += remainder * price_after / action.ratio_from;
                }
            }
            Kind::Dividend => {
                units_after += held;
                cash_paid += held * action.cash_per_unit;
            }
        }
    }

    let value_before = units_before * price_before;
    let price_after = match action.kind {
        Kind::Split => price_before * action.ratio_from / action.ratio_to,
        Kind::Dividend => price_before,
    };
    let value_after = units_after * price_after + cash_in_lieu;

    Some(Applied {
        positions_affected: affected,
        units_before,
        units_after,
        cash_paid,
        cash_in_lieu,
        // Se admite la diferencia de una unidad mínima por posición, que es lo
        // que puede dejar la división entera del precio.
        total_value_changed: (value_after - value_before).abs() > affected as i128,
        atomic: true,
    })
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn position(holder: &str, day: u32, units: i128) -> Position {
    Position { holder: holder.into(), units_by_day: BTreeMap::from([(day, units)]) }
}

fn split(from: i128, to: i128) -> CorporateAction {
    CorporateAction {
        id: "ca-2026-05".into(),
        instrument: "ACME-SIM".into(),
        kind: Kind::Split,
        ratio_from: from,
        ratio_to: to,
        cash_per_unit: 0,
        record_day: 15,
        payment_day: 20,
    }
}

pub fn report() -> CaseReport {
    let mut checks = Vec::new();

    // 1. Split 1:2 sobre posiciones redondas.
    let positions = vec![position("cli-1", 0, 500), position("cli-2", 0, 300)];
    let applied = apply(&split(1, 2), &positions, 10_000).expect("evento aplicable");
    checks.push(Check::new(
        "un split 1:2 sobre 800 unidades",
        "las unidades se duplican y el precio se parte: el valor total no cambia",
        "1600+false",
        format!("{}+{}", applied.units_after, applied.total_value_changed),
    ));

    // 2. Quien compró después de la fecha de registro no cobra.
    let positions = vec![position("cli-1", 0, 500), position("tardio", 18, 1_000)];
    let applied = apply(&split(1, 2), &positions, 10_000).expect("evento aplicable");
    checks.push(Check::new(
        "alguien compra el día 18, después del registro del día 15",
        "cuenta quién tenía el instrumento en la fecha de registro, no quien lo tiene al pagar",
        "1+1000",
        format!("{}+{}", applied.positions_affected, applied.units_after),
    ));

    // 3. Fracciones: split 2:3 sobre 5 unidades.
    let positions = vec![position("cli-1", 0, 5)];
    let applied = apply(&split(2, 3), &positions, 10_000).expect("evento aplicable");
    checks.push(Check::new(
        "un split 2:3 sobre 5 unidades deja 7,5",
        "media unidad no existe: se compensa en efectivo con la regla publicada",
        "7+con-efectivo",
        format!("{}+{}", applied.units_after, if applied.cash_in_lieu > 0 { "con-efectivo" } else { "sin-efectivo" }),
    ));

    // 4. Dividendo: aparece efectivo sin que nadie venda.
    let dividend = CorporateAction {
        id: "ca-div".into(),
        instrument: "ACME-SIM".into(),
        kind: Kind::Dividend,
        ratio_from: 1,
        ratio_to: 1,
        cash_per_unit: 250,
        record_day: 15,
        payment_day: 20,
    };
    let positions = vec![position("cli-1", 0, 500), position("cli-2", 0, 300)];
    let applied = apply(&dividend, &positions, 10_000).expect("evento aplicable");
    checks.push(Check::new(
        "un dividendo de 250 por unidad sobre 800 unidades",
        "aparece efectivo sin que nadie haya comprado ni vendido nada",
        "200000+800",
        format!("{}+{}", applied.cash_paid, applied.units_after),
    ));

    // 5. Una razón imposible no se aplica a medias.
    checks.push(Check::new(
        "un split con razón 0:2",
        "aplicar un evento imposible a la mitad de las posiciones es peor que no aplicarlo",
        "None",
        format!("{:?}", apply(&split(0, 2), &positions, 10_000).map(|_| ())),
    ));

    // 6. Consolidación 2:1.
    let positions = vec![position("cli-1", 0, 400)];
    let applied = apply(&split(2, 1), &positions, 10_000).expect("evento aplicable");
    checks.push(Check::new(
        "una consolidación 2:1 sobre 400 unidades",
        "la consolidación es el split al revés, y el valor tampoco cambia",
        "200+false",
        format!("{}+{}", applied.units_after, applied.total_value_changed),
    ));

    CaseReport::new("CM-17", "Eventos corporativos", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn el_snapshot_es_del_dia_de_registro() {
        let mut position = position("cli", 0, 100);
        position.units_by_day.insert(20, 999);
        assert_eq!(position.units_on(15), 100);
        assert_eq!(position.units_on(20), 999);
    }

    #[test]
    fn sin_posiciones_no_hay_nada_que_aplicar() {
        let applied = apply(&split(1, 2), &[], 10_000).expect("evento aplicable");
        assert_eq!(applied.positions_affected, 0);
        assert!(!applied.total_value_changed);
    }
}
