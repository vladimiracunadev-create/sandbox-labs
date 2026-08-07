//! CM-06 · Asesoría crediticia.
//!
//! El crédito se vende por la cuota, y la cuota es el peor indicador posible:
//! se baja alargando el plazo, y alargar el plazo casi siempre encarece el
//! total. Aquí se compara por **costo total**, se prueba un escenario adverso,
//! y se declara quién paga a quien recomienda.
//!
//! Nada de lo que sale de aquí es asesoría financiera real. Está en el tipo.

use super::{CaseReport, Check, Maturity};
use serde::{Deserialize, Serialize};

/// Perfil financiero **sintético**. En este proyecto no hay datos personales
/// reales, tampoco como datos de prueba.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub monthly_income: i128,
    pub monthly_obligations: i128,
    pub stability_months: u32,
}

impl Profile {
    /// Lo que puede destinar a una cuota nueva.
    ///
    /// El 30 % del ingreso libre y no el 100 %: una capacidad de pago sin
    /// margen convierte cualquier imprevisto en impago.
    pub fn capacity(&self) -> i128 {
        ((self.monthly_income - self.monthly_obligations) * 30 / 100).max(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RateType {
    Fixed,
    Variable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    pub id: String,
    pub principal: i128,
    /// Tasa anual en puntos básicos: 1 800 son 18 %. Enteros, nunca coma
    /// flotante: un céntimo por cuota son varios miles a lo largo del crédito.
    pub annual_rate_bps: u32,
    pub months: u32,
    /// Seguro mensual.
    pub insurance: i128,
    /// Comisiones de apertura, en total.
    pub fees: i128,
    pub rate_type: RateType,
    /// Si quien compara cobra del emisor. **Se declara siempre.**
    #[serde(default)]
    pub pays_commission_to_advisor: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub offer: String,
    pub monthly_payment: i128,
    /// Lo que se paga en total. **La comparación honesta.**
    pub total_cost: i128,
    pub affordable: bool,
    /// Cuota si la tasa sube tres puntos. Solo cambia en las variables.
    pub stressed_payment: i128,
    pub survives_stress: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recommendation {
    pub ranked: Vec<Assessment>,
    pub recommended: Option<String>,
    pub why: String,
    pub commercial_conflicts: Vec<String>,
    /// Obligatorio y siempre `true`.
    pub not_financial_advice: bool,
}

/// Cuota de un crédito francés, en unidades mínimas y con enteros.
///
/// Se calcula acumulando en `i128` en vez de con potencias en coma flotante:
/// el error de un `f64` aquí se reparte entre todas las cuotas y no cuadra
/// nunca del todo.
fn monthly_payment(principal: i128, annual_rate_bps: u32, months: u32) -> i128 {
    if months == 0 {
        return principal;
    }
    if annual_rate_bps == 0 {
        return principal / i128::from(months);
    }
    // Tasa mensual en millonésimas. Un punto básico es una diezmilésima, así
    // que pasar a millonésimas es multiplicar por cien; el /12 es el mes.
    // Escrito de otra forma se cuela un factor 100 y las cuotas salen absurdas.
    let monthly_rate_micro = i128::from(annual_rate_bps) * 100 / 12;
    const SCALE: i128 = 1_000_000;

    // (1 + i)^n con aritmética entera escalada.
    let mut compound = SCALE;
    for _ in 0..months {
        compound = compound * (SCALE + monthly_rate_micro) / SCALE;
    }
    let numerator = principal * monthly_rate_micro * compound / SCALE;
    let denominator = compound - SCALE;
    if denominator == 0 {
        return principal / i128::from(months);
    }
    // Se redondea hacia arriba: una cuota corta deja deuda viva al final.
    (numerator + denominator - 1) / denominator
}

/// Evalúa las ofertas contra un perfil y recomienda con explicación.
pub fn advise(profile: &Profile, offers: &[Offer]) -> Recommendation {
    let capacity = profile.capacity();

    let mut ranked: Vec<Assessment> = offers
        .iter()
        .map(|offer| {
            let base = monthly_payment(offer.principal, offer.annual_rate_bps, offer.months);
            let payment = base + offer.insurance;
            let total = payment * i128::from(offer.months) + offer.fees;

            let stressed = match offer.rate_type {
                RateType::Fixed => payment,
                RateType::Variable => {
                    monthly_payment(offer.principal, offer.annual_rate_bps + 300, offer.months) + offer.insurance
                }
            };

            Assessment {
                offer: offer.id.clone(),
                monthly_payment: payment,
                total_cost: total,
                affordable: payment <= capacity,
                stressed_payment: stressed,
                survives_stress: stressed <= capacity,
            }
        })
        .collect();

    // Se ordena por costo total, no por cuota. Esa línea es el caso entero.
    ranked.sort_by_key(|assessment| assessment.total_cost);

    let recommended = ranked
        .iter()
        .find(|assessment| assessment.affordable && assessment.survives_stress)
        .or_else(|| ranked.iter().find(|assessment| assessment.affordable))
        .map(|assessment| assessment.offer.clone());

    let why = match &recommended {
        Some(id) => format!("{id} tiene el menor costo total entre las ofertas que caben en la capacidad de pago y resisten el escenario adverso"),
        None => "ninguna oferta cabe en la capacidad de pago de este perfil".to_string(),
    };

    Recommendation {
        ranked,
        recommended,
        why,
        commercial_conflicts: offers
            .iter()
            .filter(|offer| offer.pays_commission_to_advisor)
            .map(|offer| format!("{} paga comisión a quien compara", offer.id))
            .collect(),
        not_financial_advice: true,
    }
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn offer(id: &str, months: u32, rate_bps: u32, rate_type: RateType) -> Offer {
    Offer {
        id: id.into(),
        principal: 5_000_000,
        annual_rate_bps: rate_bps,
        months,
        insurance: 0,
        fees: 0,
        rate_type,
        pays_commission_to_advisor: false,
    }
}

pub fn report() -> CaseReport {
    let profile = Profile { monthly_income: 2_000_000, monthly_obligations: 300_000, stability_months: 24 };
    let mut checks = Vec::new();

    // 1. Plazo largo, cuota baja, coste total mayor.
    let offers = vec![offer("corto", 24, 1_800, RateType::Fixed), offer("largo", 60, 1_800, RateType::Fixed)];
    let advice = advise(&profile, &offers);
    let cuota_corto = advice.ranked.iter().find(|a| a.offer == "corto").map(|a| a.monthly_payment).unwrap_or(0);
    let cuota_largo = advice.ranked.iter().find(|a| a.offer == "largo").map(|a| a.monthly_payment).unwrap_or(0);
    checks.push(Check::new(
        "dos ofertas, misma tasa, plazos de 24 y 60 meses",
        "el plazo largo baja la cuota y sube el total: por eso se compara por costo total",
        "corto+cuota-mayor",
        format!(
            "{}+{}",
            advice.recommended.clone().unwrap_or_default(),
            if cuota_corto > cuota_largo { "cuota-mayor" } else { "cuota-menor" }
        ),
    ));

    // 2. Ninguna oferta cabe.
    let apretado = Profile { monthly_income: 400_000, monthly_obligations: 350_000, stability_months: 6 };
    let advice = advise(&apretado, &offers);
    checks.push(Check::new(
        "un perfil al que no le cabe ninguna cuota",
        "el resultado correcto es «ninguna de estas ofertas cabe», no relajar el margen hasta que salga una",
        "None",
        format!("{:?}", advice.recommended),
    ));

    // 3. Tasa variable que no resiste el escenario adverso.
    let offers = vec![offer("variable", 36, 3_800, RateType::Variable)];
    let advice = advise(&profile, &offers);
    let assessment = advice.ranked.first().expect("una oferta");
    checks.push(Check::new(
        "una oferta a tasa variable con la tasa subiendo tres puntos",
        "quien decide tiene que ver el escenario malo, no solo el bueno",
        "cuota-sube",
        if assessment.stressed_payment > assessment.monthly_payment { "cuota-sube" } else { "cuota-igual" }.to_string(),
    ));

    // 4. La tasa fija no se mueve en el escenario adverso.
    let offers = vec![offer("fija", 36, 3_800, RateType::Fixed)];
    let advice = advise(&profile, &offers);
    let assessment = advice.ranked.first().expect("una oferta");
    checks.push(Check::new(
        "la misma oferta, pero a tasa fija",
        "el escenario adverso solo mueve lo que puede moverse",
        "cuota-igual",
        if assessment.stressed_payment == assessment.monthly_payment { "cuota-igual" } else { "cuota-sube" }
            .to_string(),
    ));

    // 5. Conflicto comercial declarado.
    let mut paga = offer("paga", 24, 1_500, RateType::Fixed);
    paga.pays_commission_to_advisor = true;
    let advice = advise(&profile, &[paga, offer("no-paga", 24, 1_900, RateType::Fixed)]);
    checks.push(Check::new(
        "gana la oferta de quien paga comisión al comparador",
        "una recomendación sin declarar de qué vive quien recomienda es publicidad",
        "paga+1",
        format!("{}+{}", advice.recommended.clone().unwrap_or_default(), advice.commercial_conflicts.len()),
    ));

    // 6. Nunca es asesoría real.
    checks.push(Check::new(
        "cualquier recomendación, mirada de cerca",
        "un simulador no da asesoría financiera, y eso va en el tipo",
        "true",
        advice.not_financial_advice.to_string(),
    ));

    CaseReport::new("CM-06", "Asesoría crediticia", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn el_costo_total_crece_con_el_plazo() {
        let profile = Profile { monthly_income: 5_000_000, monthly_obligations: 0, stability_months: 60 };
        let advice =
            advise(&profile, &[offer("a", 12, 2_000, RateType::Fixed), offer("b", 48, 2_000, RateType::Fixed)]);
        let corto = advice.ranked.iter().find(|a| a.offer == "a").unwrap().total_cost;
        let largo = advice.ranked.iter().find(|a| a.offer == "b").unwrap().total_cost;
        assert!(largo > corto, "alargar el plazo tiene que encarecer el total");
    }

    #[test]
    fn una_tasa_cero_no_divide_por_cero() {
        assert_eq!(monthly_payment(1_200, 0, 12), 100);
    }

    #[test]
    fn la_cuota_cubre_el_capital() {
        // Doce cuotas al 18 % tienen que devolver más que el capital prestado.
        let cuota = monthly_payment(1_000_000, 1_800, 12);
        assert!(cuota * 12 > 1_000_000);
    }
}
