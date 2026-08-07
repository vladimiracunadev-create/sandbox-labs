//! CM-01 · Financiamiento colectivo.
//!
//! Mientras la campaña está abierta, **cada peso sigue siendo del
//! inversionista**. La plataforma lo custodia, no lo posee. Y la prueba de que
//! eso es cierto no es una cláusula: es que la devolución funciona y cuadra
//! hasta la última unidad.

use super::{CaseReport, Check, Maturity};
use crate::money::{Currency, Money};
use serde::{Deserialize, Serialize};

/// Cómo se reparte cuando entra más dinero del máximo.
///
/// La regla se publica **antes** de abrir la campaña. Asignar por orden de
/// llegada sin decirlo es lo que genera las reclamaciones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AllocationRule {
    /// A prorrata del monto solicitado.
    ProRata,
    /// Por orden de llegada, publicado como tal.
    FirstComeFirstServed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Campaign {
    pub issuer: String,
    pub min_target: Money,
    pub max_target: Money,
    pub allocation_rule: AllocationRule,
    /// Tope por inversionista. Protege a quien no puede permitirse concentrar.
    pub per_investor_cap: Money,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Commitment {
    pub investor: String,
    pub amount: Money,
    /// Orden de llegada. Un número de secuencia, no el reloj: el reloj haría
    /// que el reparto dependiera de la máquina.
    pub sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Settlement {
    /// Se alcanzó el mínimo: el dinero pasa al emisor.
    Funded,
    /// No se alcanzó: **se devuelve todo**.
    Refunded,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Allocation {
    pub investor: String,
    pub allocated: Money,
    pub refunded: Money,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Outcome {
    pub settlement: Settlement,
    pub raised: Money,
    pub allocations: Vec<Allocation>,
    pub rejected: Vec<Rejection>,
    /// Lo asignado más lo devuelto tiene que ser exactamente lo comprometido.
    /// Si esto es `false`, hay dinero de alguien sin dueño declarado.
    pub balanced: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rejection {
    pub investor: String,
    pub reason: &'static str,
}

/// Cierra la campaña y reparte.
///
/// El reparto a prorrata reparte también **el resto de la división**, en orden
/// de llegada. Perderlo por redondeo sería un descuadre pequeño y permanente,
/// que es la peor clase.
pub fn close(campaign: &Campaign, commitments: &[Commitment]) -> Outcome {
    let currency = campaign.min_target.currency();
    let mut accepted: Vec<&Commitment> = Vec::new();
    let mut rejected = Vec::new();

    let mut ordered: Vec<&Commitment> = commitments.iter().collect();
    ordered.sort_by_key(|commitment| commitment.sequence);

    for commitment in ordered {
        if commitment.amount.currency() != currency {
            rejected.push(Rejection {
                investor: commitment.investor.clone(),
                reason: "moneda distinta a la de la campaña",
            });
        } else if commitment.amount > campaign.per_investor_cap {
            rejected
                .push(Rejection { investor: commitment.investor.clone(), reason: "supera el tope por inversionista" });
        } else {
            accepted.push(commitment);
        }
    }

    let total: i128 = accepted.iter().map(|commitment| commitment.amount.minor_units()).sum();
    let raised = Money::new(total, currency);

    if total < campaign.min_target.minor_units() {
        // No se alcanzó el mínimo: vuelve todo, sin excepciones.
        let allocations = accepted
            .iter()
            .map(|commitment| Allocation {
                investor: commitment.investor.clone(),
                allocated: Money::zero(currency),
                refunded: commitment.amount,
            })
            .collect();
        return Outcome { settlement: Settlement::Refunded, raised, allocations, rejected, balanced: true };
    }

    let cap = campaign.max_target.minor_units();
    let allocations: Vec<Allocation> = if total <= cap {
        accepted
            .iter()
            .map(|commitment| Allocation {
                investor: commitment.investor.clone(),
                allocated: commitment.amount,
                refunded: Money::zero(currency),
            })
            .collect()
    } else {
        match campaign.allocation_rule {
            AllocationRule::FirstComeFirstServed => {
                let mut remaining = cap;
                accepted
                    .iter()
                    .map(|commitment| {
                        let take = commitment.amount.minor_units().min(remaining);
                        remaining -= take;
                        Allocation {
                            investor: commitment.investor.clone(),
                            allocated: Money::new(take, currency),
                            refunded: Money::new(commitment.amount.minor_units() - take, currency),
                        }
                    })
                    .collect()
            }
            AllocationRule::ProRata => {
                let mut assigned = 0_i128;
                let mut result: Vec<Allocation> = accepted
                    .iter()
                    .map(|commitment| {
                        let share = commitment.amount.minor_units() * cap / total;
                        assigned += share;
                        Allocation {
                            investor: commitment.investor.clone(),
                            allocated: Money::new(share, currency),
                            refunded: Money::new(commitment.amount.minor_units() - share, currency),
                        }
                    })
                    .collect();
                // El resto de la división se reparte por orden de llegada, una
                // unidad a cada uno, hasta agotarlo. Nada se pierde.
                let mut leftover = cap - assigned;
                for allocation in result.iter_mut() {
                    if leftover == 0 {
                        break;
                    }
                    if allocation.refunded.minor_units() > 0 {
                        allocation.allocated = Money::new(allocation.allocated.minor_units() + 1, currency);
                        allocation.refunded = Money::new(allocation.refunded.minor_units() - 1, currency);
                        leftover -= 1;
                    }
                }
                result
            }
        }
    };

    let balanced = accepted.iter().zip(&allocations).all(|(commitment, allocation)| {
        allocation.allocated.minor_units() + allocation.refunded.minor_units() == commitment.amount.minor_units()
    });

    Outcome { settlement: Settlement::Funded, raised, allocations, rejected, balanced }
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn clp(units: i128) -> Money {
    Money::new(units, Currency::Clp)
}

fn campaign(rule: AllocationRule) -> Campaign {
    Campaign {
        issuer: "emisor-simulado-1".into(),
        min_target: clp(10_000_000),
        max_target: clp(50_000_000),
        allocation_rule: rule,
        per_investor_cap: clp(20_000_000),
    }
}

fn commitments(amounts: &[i128]) -> Vec<Commitment> {
    amounts
        .iter()
        .enumerate()
        .map(|(index, amount)| Commitment {
            investor: format!("inversionista-{index}"),
            amount: clp(*amount),
            sequence: index as u64,
        })
        .collect()
}

pub fn report() -> CaseReport {
    let mut checks = Vec::new();

    // 1. No se alcanza el mínimo: vuelve todo.
    let outcome = close(&campaign(AllocationRule::ProRata), &commitments(&[3_000_000, 4_000_000]));
    let devuelto: i128 = outcome.allocations.iter().map(|allocation| allocation.refunded.minor_units()).sum();
    checks.push(Check::new(
        "la campaña cierra sin alcanzar el mínimo",
        "mientras la campaña está abierta el dinero sigue siendo del inversionista",
        "refunded+7000000",
        format!("{:?}+{devuelto}", outcome.settlement).to_lowercase(),
    ));

    // 2. Sobredemanda a prorrata: se reparte el tope y nada se pierde.
    let outcome = close(&campaign(AllocationRule::ProRata), &commitments(&[20_000_000, 20_000_000, 20_000_000]));
    let asignado: i128 = outcome.allocations.iter().map(|allocation| allocation.allocated.minor_units()).sum();
    checks.push(Check::new(
        "entra más dinero del máximo, con reparto a prorrata",
        "el resto de la división se reparte en vez de perderse: un descuadre pequeño y permanente es la peor clase",
        "50000000+cuadra",
        format!("{asignado}+{}", if outcome.balanced { "cuadra" } else { "descuadra" }),
    ));

    // 3. Por orden de llegada, publicado como tal.
    let outcome =
        close(&campaign(AllocationRule::FirstComeFirstServed), &commitments(&[20_000_000, 20_000_000, 20_000_000]));
    let ultimo = outcome.allocations.last().map(|allocation| allocation.allocated.minor_units());
    checks.push(Check::new(
        "sobredemanda por orden de llegada",
        "una regla de reparto solo es justa si se publicó antes de abrir la campaña",
        "Some(10000000)",
        format!("{ultimo:?}"),
    ));

    // 4. Un inversionista supera su tope.
    let outcome = close(&campaign(AllocationRule::ProRata), &commitments(&[30_000_000, 5_000_000, 6_000_000]));
    checks.push(Check::new(
        "un inversionista compromete más de su tope",
        "el tope por inversionista protege a quien no puede permitirse concentrar",
        "1 rechazo",
        format!("{} rechazo", outcome.rejected.len()),
    ));

    // 5. Lo asignado más lo devuelto es siempre lo comprometido.
    let outcome = close(&campaign(AllocationRule::ProRata), &commitments(&[7_000_000, 11_000_000, 13_000_000]));
    checks.push(Check::new(
        "una campaña cualquiera, sumada al céntimo",
        "asignado + devuelto tiene que ser exactamente lo comprometido, o hay dinero sin dueño",
        "true",
        outcome.balanced.to_string(),
    ));

    CaseReport::new("CM-01", "Financiamiento colectivo", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn la_devolucion_es_integra() {
        let outcome = close(&campaign(AllocationRule::ProRata), &commitments(&[1_000_000, 2_000_000]));
        assert_eq!(outcome.settlement, Settlement::Refunded);
        for allocation in &outcome.allocations {
            assert!(allocation.allocated.is_zero(), "no se puede asignar nada si no se alcanzó el mínimo");
        }
    }

    #[test]
    fn el_prorrateo_no_pierde_unidades() {
        // Cantidades que no dividen exacto: aquí es donde se pierde el resto si
        // nadie lo reparte.
        let outcome = close(&campaign(AllocationRule::ProRata), &commitments(&[17_000_001, 17_000_001, 17_000_001]));
        let asignado: i128 = outcome.allocations.iter().map(|allocation| allocation.allocated.minor_units()).sum();
        assert_eq!(asignado, 50_000_000);
        assert!(outcome.balanced);
    }

    #[test]
    fn una_moneda_distinta_no_entra_en_la_campana() {
        let mixed = vec![Commitment { investor: "x".into(), amount: Money::new(100, Currency::Usd), sequence: 0 }];
        let outcome = close(&campaign(AllocationRule::ProRata), &mixed);
        assert_eq!(outcome.rejected.len(), 1);
    }
}
