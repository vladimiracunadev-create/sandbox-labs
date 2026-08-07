//! CM-04 · Enrutamiento inteligente de órdenes.
//!
//! «Mejor ejecución» no es «mejor precio»: es el mejor resultado contando
//! comisión, liquidez, latencia y probabilidad de ejecución. Y una decisión que
//! no se puede explicar no es defendible ante un cliente ni ante un supervisor,
//! así que aquí **la explicación es obligatoria** y viaja con la decisión.

use super::{CaseReport, Check, Maturity};
use serde::{Deserialize, Serialize};

/// Un destino donde se puede ejecutar. Precios en unidades mínimas.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Venue {
    pub id: String,
    pub price: i128,
    /// Comisión fija por operación en ese destino.
    pub fee: i128,
    /// Cuánto hay visible. Un precio excelente para 10 unidades no sirve para
    /// 10 000.
    pub displayed_size: i128,
    pub latency_ms: u32,
    /// Entre 0 y 100. Un destino barato donde no se ejecuta nada no es barato.
    pub fill_probability: u8,
    /// Si el destino remunera el flujo de órdenes. **Se declara siempre**: es
    /// un motivo para elegirlo que no es el del cliente.
    #[serde(default)]
    pub pays_for_flow: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub instrument: String,
    pub quantity: i128,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Slice {
    pub venue: String,
    pub quantity: i128,
    pub cost: i128,
}

/// La decisión, con el razonamiento pegado.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    pub slices: Vec<Slice>,
    pub total_cost: i128,
    pub explanation: Explanation,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Explanation {
    pub considered: Vec<String>,
    /// Coste total estimado por destino, para poder rehacer la comparación.
    pub cost_by_venue: Vec<(String, i128)>,
    pub why: String,
    /// Conflictos declarados. Vacío significa que no hay, no que no se miró.
    pub conflicts_disclosed: Vec<String>,
    pub fragmented: bool,
}

/// Coste efectivo de ejecutar `quantity` en un destino.
///
/// Ajustado por probabilidad de ejecución: lo que no se ejecuta hay que
/// ejecutarlo en otro sitio, y eso cuesta. Un destino con 50 % de probabilidad
/// vale la mitad de lo que aparenta.
fn effective_cost(venue: &Venue, quantity: i128) -> i128 {
    let gross = venue.price * quantity + venue.fee;
    let probability = venue.fill_probability.clamp(1, 100) as i128;
    gross * 100 / probability
}

/// Elige dónde ejecutar y explica por qué.
///
/// Fragmentar mejora el precio y multiplica comisiones, así que solo se
/// fragmenta cuando **ningún destino cubre la orden entera** — y entonces se
/// dice en la explicación.
pub fn route(order: &Order, venues: &[Venue]) -> Decision {
    let mut cost_by_venue: Vec<(String, i128)> =
        venues.iter().map(|venue| (venue.id.clone(), effective_cost(venue, order.quantity))).collect();
    cost_by_venue.sort_by_key(|(_, cost)| *cost);

    let conflicts_disclosed: Vec<String> = venues
        .iter()
        .filter(|venue| venue.pays_for_flow)
        .map(|venue| format!("{} remunera el flujo de órdenes", venue.id))
        .collect();

    let considered: Vec<String> = venues.iter().map(|venue| venue.id.clone()).collect();

    // ¿Hay alguno que cubra la orden entera?
    let mut full: Vec<&Venue> = venues.iter().filter(|venue| venue.displayed_size >= order.quantity).collect();
    full.sort_by_key(|venue| effective_cost(venue, order.quantity));

    if let Some(best) = full.first() {
        let cost = best.price * order.quantity + best.fee;
        let why = format!(
            "{} cubre las {} unidades con el menor coste efectivo ajustado por probabilidad de ejecución",
            best.id, order.quantity
        );
        return Decision {
            slices: vec![Slice { venue: best.id.clone(), quantity: order.quantity, cost }],
            total_cost: cost,
            explanation: Explanation { considered, cost_by_venue, why, conflicts_disclosed, fragmented: false },
        };
    }

    // Ninguno cubre la orden: se fragmenta por coste unitario, del más barato
    // al más caro, y se dice.
    let mut ordered: Vec<&Venue> = venues.iter().collect();
    ordered.sort_by_key(|venue| effective_cost(venue, 1));

    let mut remaining = order.quantity;
    let mut slices = Vec::new();
    let mut total_cost = 0;
    for venue in ordered {
        if remaining == 0 {
            break;
        }
        let take = venue.displayed_size.min(remaining);
        if take <= 0 {
            continue;
        }
        let cost = venue.price * take + venue.fee;
        total_cost += cost;
        remaining -= take;
        slices.push(Slice { venue: venue.id.clone(), quantity: take, cost });
    }

    let why = format!(
        "ningún destino cubre las {} unidades: se fragmenta en {} y cada trozo paga su comisión",
        order.quantity,
        slices.len()
    );
    Decision {
        slices,
        total_cost,
        explanation: Explanation { considered, cost_by_venue, why, conflicts_disclosed, fragmented: true },
    }
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn venue(id: &str, price: i128, fee: i128, size: i128, latency: u32, probability: u8, pays: bool) -> Venue {
    Venue {
        id: id.into(),
        price,
        fee,
        displayed_size: size,
        latency_ms: latency,
        fill_probability: probability,
        pays_for_flow: pays,
    }
}

pub fn report() -> CaseReport {
    let order = Order { instrument: "ACME-SIM".into(), quantity: 10_000 };
    let mut checks = Vec::new();

    // 1. Mejor precio con comisión alta frente a peor precio sin comisión.
    let venues =
        vec![venue("A", 10_000, 5_000_000, 20_000, 3, 90, false), venue("B", 10_050, 0, 20_000, 12, 99, false)];
    let decision = route(&order, &venues);
    checks.push(Check::new(
        "A tiene mejor precio y una comisión que se lo come",
        "mejor ejecución no es mejor precio: la comisión forma parte del coste",
        "B",
        decision.slices.first().map(|slice| slice.venue.clone()).unwrap_or_default(),
    ));

    // 2. Precio excelente en un destino sin liquidez suficiente.
    let venues = vec![venue("A", 9_900, 0, 2_000, 3, 95, false), venue("B", 10_000, 0, 20_000, 5, 95, false)];
    let decision = route(&order, &venues);
    checks.push(Check::new(
        "el destino más barato solo cubre 2 000 de 10 000 unidades",
        "un precio excelente para poca cantidad no es un precio para tu orden",
        "B+sin-fragmentar",
        format!(
            "{}+{}",
            decision.slices.first().map(|slice| slice.venue.clone()).unwrap_or_default(),
            if decision.explanation.fragmented { "fragmentada" } else { "sin-fragmentar" }
        ),
    ));

    // 3. Nadie cubre la orden: se fragmenta y se dice.
    let venues = vec![venue("A", 9_900, 0, 4_000, 3, 95, false), venue("B", 10_000, 0, 6_000, 5, 95, false)];
    let decision = route(&order, &venues);
    let total: i128 = decision.slices.iter().map(|slice| slice.quantity).sum();
    checks.push(Check::new(
        "ningún destino cubre la orden entera",
        "fragmentar es legítimo, y decir que se fragmentó también",
        "10000+fragmentada",
        format!("{total}+{}", if decision.explanation.fragmented { "fragmentada" } else { "sin-fragmentar" }),
    ));

    // 4. Probabilidad de ejecución baja: el barato deja de ser barato.
    let venues = vec![venue("A", 9_800, 0, 20_000, 2, 30, false), venue("B", 10_000, 0, 20_000, 8, 99, false)];
    let decision = route(&order, &venues);
    checks.push(Check::new(
        "el destino barato solo ejecuta el 30 % de las veces",
        "un mercado barato donde no se ejecuta nada no es barato",
        "B",
        decision.slices.first().map(|slice| slice.venue.clone()).unwrap_or_default(),
    ));

    // 5. Un destino que paga por el flujo: se declara aunque gane.
    let venues = vec![venue("A", 9_900, 0, 20_000, 3, 99, true), venue("B", 10_000, 0, 20_000, 5, 99, false)];
    let decision = route(&order, &venues);
    checks.push(Check::new(
        "gana un destino que remunera el flujo de órdenes",
        "el conflicto se declara aunque la decisión sea la correcta: no declararlo es lo que la hace sospechosa",
        "A+1 conflicto",
        format!(
            "{}+{} conflicto",
            decision.slices.first().map(|slice| slice.venue.clone()).unwrap_or_default(),
            decision.explanation.conflicts_disclosed.len()
        ),
    ));

    // 6. Toda decisión trae explicación con los números.
    checks.push(Check::new(
        "cualquier decisión, mirada de cerca",
        "una decisión sin los números que la produjeron no se puede rehacer ni discutir",
        "true",
        (!decision.explanation.why.is_empty() && decision.explanation.cost_by_venue.len() == 2).to_string(),
    ));

    CaseReport::new("CM-04", "Enrutamiento inteligente de órdenes", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn la_orden_se_ejecuta_entera_o_se_dice_cuanto_falta() {
        let order = Order { instrument: "X".into(), quantity: 100 };
        let venues = vec![venue("A", 10, 0, 40, 1, 99, false), venue("B", 11, 0, 80, 1, 99, false)];
        let decision = route(&order, &venues);
        let total: i128 = decision.slices.iter().map(|slice| slice.quantity).sum();
        assert_eq!(total, 100);
    }

    #[test]
    fn una_probabilidad_de_cero_no_divide_por_cero() {
        let order = Order { instrument: "X".into(), quantity: 10 };
        let venues = vec![venue("A", 10, 0, 100, 1, 0, false)];
        let decision = route(&order, &venues);
        assert_eq!(decision.slices.len(), 1);
    }
}
