//! CM-05 · Intermediación financiera.
//!
//! El conflicto de interés aquí **no es moral, es estructural**: depende de en
//! qué papel se actúa. Como agente, el intermediario cobra comisión y sus
//! intereses van con los del cliente. Como principal, gana en la diferencia de
//! precio, y entonces cuanto peor el precio del cliente, mejor su margen.
//!
//! Por eso `capacity` es obligatorio en cada operación: sin saber en qué papel
//! se actuó no se puede evaluar nada.

use super::{CaseReport, Check, Maturity};
use serde::{Deserialize, Serialize};

/// En qué papel se actuó.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capacity {
    /// Busca en el mercado por cuenta del cliente. Cobra comisión, declarada.
    Agent,
    /// Vende de su propio inventario. Gana en el spread.
    Principal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    pub id: String,
    pub account: String,
    pub instrument: String,
    /// Número de secuencia, no reloj: el orden tiene que ser el mismo en
    /// cualquier máquina.
    pub sequence: u64,
    pub capacity: Capacity,
    /// Si la cuenta es de la casa. Una orden propia junto a una de cliente es
    /// donde vive el front-running.
    pub house_account: bool,
    pub price: i128,
    /// Precio de referencia del mercado en ese instante.
    pub reference_price: i128,
    pub quantity: i128,
    pub commission: i128,
    /// ¿Se le dijo al cliente lo que pagaba?
    pub disclosed: bool,
    /// Unidades disponibles al vender. Menos de lo vendido es venta sin tener.
    pub available: i128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Finding {
    /// La casa se puso delante de la orden del cliente y el precio se movió en
    /// su contra.
    FrontRunning { house_order: String, client_order: String, sequence_gap: u64, price_impact: i128 },
    /// Se cobró y no se dijo.
    UndisclosedCommission { execution: String },
    /// Como principal, el spread contra el precio de referencia sin declararlo.
    UndisclosedSpread { execution: String, spread: i128 },
    /// Se vendió lo que no se tenía.
    SaleWithoutAvailability { execution: String, missing: i128 },
}

impl Finding {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::FrontRunning { .. } => "front-running",
            Self::UndisclosedCommission { .. } => "undisclosed-commission",
            Self::UndisclosedSpread { .. } => "undisclosed-spread",
            Self::SaleWithoutAvailability { .. } => "sale-without-availability",
        }
    }
}

/// Cuántos números de secuencia caben entre la orden de la casa y la del
/// cliente para seguir considerándolo front-running.
///
/// No es un umbral moral: es la ventana en la que la orden propia todavía puede
/// haber movido el precio que paga el cliente.
const FRONT_RUNNING_WINDOW: u64 = 5;

/// Revisa un conjunto de ejecuciones y devuelve lo que no encaja.
///
/// El orden de las comprobaciones no importa: cada una mira una cosa distinta.
/// Lo que importa es que **todas se ejecutan siempre**, para que un hallazgo no
/// tape a otro.
pub fn review(executions: &[Execution]) -> Vec<Finding> {
    let mut findings = Vec::new();

    let mut ordered: Vec<&Execution> = executions.iter().collect();
    ordered.sort_by_key(|execution| execution.sequence);

    for execution in &ordered {
        if execution.commission > 0 && !execution.disclosed {
            findings.push(Finding::UndisclosedCommission { execution: execution.id.clone() });
        }
        if execution.capacity == Capacity::Principal && !execution.disclosed {
            let spread = (execution.price - execution.reference_price).abs();
            if spread > 0 {
                findings.push(Finding::UndisclosedSpread { execution: execution.id.clone(), spread });
            }
        }
        if execution.available < execution.quantity {
            findings.push(Finding::SaleWithoutAvailability {
                execution: execution.id.clone(),
                missing: execution.quantity - execution.available,
            });
        }
    }

    // Front-running: una orden de la casa justo antes de una de cliente en el
    // mismo instrumento, con el precio moviéndose en contra del cliente.
    for house in ordered.iter().filter(|execution| execution.house_account) {
        for client in ordered.iter().filter(|execution| !execution.house_account) {
            if house.instrument != client.instrument || house.sequence >= client.sequence {
                continue;
            }
            let gap = client.sequence - house.sequence;
            if gap > FRONT_RUNNING_WINDOW {
                continue;
            }
            let impact = client.price - client.reference_price;
            if impact > 0 {
                findings.push(Finding::FrontRunning {
                    house_order: house.id.clone(),
                    client_order: client.id.clone(),
                    sequence_gap: gap,
                    price_impact: impact,
                });
            }
        }
    }

    findings
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn execution(id: &str, sequence: u64, house: bool, capacity: Capacity) -> Execution {
    Execution {
        id: id.into(),
        account: if house { "casa".into() } else { "cliente-1".into() },
        instrument: "ACME-SIM".into(),
        sequence,
        capacity,
        house_account: house,
        price: 10_000,
        reference_price: 10_000,
        quantity: 100,
        commission: 0,
        disclosed: true,
        available: 100,
    }
}

fn kinds(findings: &[Finding]) -> String {
    let mut names: Vec<&str> = findings.iter().map(Finding::kind).collect();
    names.sort_unstable();
    names.dedup();
    names.join(",")
}

pub fn report() -> CaseReport {
    let mut checks = Vec::new();

    // 1. Operación limpia como agente.
    let clean = vec![execution("e1", 1, false, Capacity::Agent)];
    checks.push(Check::new(
        "una ejecución como agente, con todo declarado",
        "actuar como agente y decirlo es el caso donde no hay nada que reprochar",
        "",
        kinds(&review(&clean)),
    ));

    // 2. La casa se pone delante y el cliente paga más.
    let mut house = execution("casa-9", 1, true, Capacity::Agent);
    house.price = 10_000;
    let mut client = execution("cli-100", 2, false, Capacity::Agent);
    client.price = 10_050; // pagó por encima de la referencia
    checks.push(Check::new(
        "la casa compra justo antes que el cliente y el precio sube",
        "el front-running se ve comparando tiempos, no intenciones",
        "front-running",
        kinds(&review(&[house, client])),
    ));

    // 3. La casa opera mucho después: no es front-running.
    let house = execution("casa-9", 1, true, Capacity::Agent);
    let mut client = execution("cli-100", 40, false, Capacity::Agent);
    client.price = 10_050;
    checks.push(Check::new(
        "la casa operó cuarenta órdenes antes, en el mismo instrumento",
        "sin ventana temporal cualquier operación de la casa parecería front-running",
        "",
        kinds(&review(&[house, client])),
    ));

    // 4. Comisión no informada.
    let mut hidden = execution("e2", 1, false, Capacity::Agent);
    hidden.commission = 2_500;
    hidden.disclosed = false;
    checks.push(Check::new(
        "se cobra comisión y no se informa",
        "el cliente no puede comparar lo que no sabe que paga",
        "undisclosed-commission",
        kinds(&review(&[hidden])),
    ));

    // 5. Como principal, con spread y sin declararlo.
    let mut principal = execution("e3", 1, false, Capacity::Principal);
    principal.price = 10_250;
    principal.disclosed = false;
    checks.push(Check::new(
        "se vende del inventario propio con spread y sin declararlo",
        "«sin comisión» puede salir más caro que con comisión, y esa es la trampa",
        "undisclosed-spread",
        kinds(&review(&[principal])),
    ));

    // 6. Venta sin disponibilidad.
    let mut short = execution("e4", 1, false, Capacity::Principal);
    short.available = 40;
    checks.push(Check::new(
        "se venden 100 unidades teniendo 40",
        "vender lo que no se tiene crea una obligación que alguien tendrá que cubrir",
        "sale-without-availability",
        kinds(&review(&[short])),
    ));

    CaseReport::new("CM-05", "Intermediación financiera", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn un_hallazgo_no_tapa_a_otro() {
        let mut bad = execution("e5", 1, false, Capacity::Principal);
        bad.commission = 100;
        bad.disclosed = false;
        bad.price = 10_500;
        bad.available = 1;
        let findings = review(&[bad]);
        assert_eq!(findings.len(), 3, "las tres comprobaciones tienen que ejecutarse siempre");
    }

    #[test]
    fn otro_instrumento_no_es_front_running() {
        let mut house = execution("casa", 1, true, Capacity::Agent);
        house.instrument = "OTRO-SIM".into();
        let mut client = execution("cli", 2, false, Capacity::Agent);
        client.price = 10_100;
        assert!(review(&[house, client]).is_empty());
    }
}
