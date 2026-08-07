//! CM-07 · Robo-advisor.
//!
//! Automatizar la asesoría no elimina el deber de idoneidad: lo hace auditable
//! a escala. Si el modelo se equivoca, se equivoca con todos los clientes a la
//! vez. Por eso cada recomendación guarda **la versión del modelo que la tomó**,
//! el perfil de ese momento y el razonamiento: sin las tres cosas no se puede
//! responder a quien reclama dos años después.

use super::{CaseReport, Check, Maturity};
use serde::{Deserialize, Serialize};

/// Perfil **sintético** del cliente.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientProfile {
    pub client: String,
    pub horizon_years: u32,
    /// Tolerancia declarada, de 1 a 5. Declarada, que no es lo mismo que real.
    pub risk_tolerance: u8,
    /// Antigüedad del perfilamiento. Un perfil viejo describe a otra persona.
    pub profiled_months_ago: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Holding {
    pub asset: &'static str,
    /// Peso en puntos porcentuales. Enteros: los pesos tienen que sumar 100
    /// exacto, y con coma flotante no lo hacen.
    pub weight: u32,
    pub house_product: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Advice {
    pub portfolio: Vec<Holding>,
    pub equity_weight: u32,
    pub house_product_share: u32,
    pub suitable: bool,
    pub why: String,
    pub findings: Vec<&'static str>,
    /// Sin esto no se puede reconstruir la decisión.
    pub model_version: &'static str,
    pub not_financial_advice: bool,
}

/// Versión del modelo. Va en cada recomendación, no en un fichero aparte.
pub const MODEL_VERSION: &str = "1.4.2";

/// Meses tras los cuales un perfil deja de describir a la persona.
const PROFILE_STALE_MONTHS: u32 = 24;

/// Techo de renta variable según el horizonte.
///
/// Un horizonte corto no admite exposición alta por mucha tolerancia que se
/// declare: quien necesita el dinero en un año no puede esperar a que el
/// mercado se recupere.
fn equity_cap(horizon_years: u32) -> u32 {
    match horizon_years {
        0..=1 => 10,
        2..=3 => 30,
        4..=7 => 60,
        _ => 85,
    }
}

/// Construye la cartera y comprueba que es idónea **antes** de emitirla.
pub fn advise(profile: &ClientProfile, house_bias: u32) -> Advice {
    let cap = equity_cap(profile.horizon_years);
    let desired = u32::from(profile.risk_tolerance.clamp(1, 5)) * 20;
    let equity = desired.min(cap);

    let house = house_bias.min(equity);
    let portfolio = vec![
        Holding { asset: "renta-variable-sim", weight: equity - house, house_product: false },
        Holding { asset: "renta-variable-casa-sim", weight: house, house_product: true },
        Holding { asset: "renta-fija-sim", weight: 100 - equity, house_product: false },
    ];

    let mut findings = Vec::new();
    if desired > cap {
        findings.push("tolerancia declarada por encima de lo que admite el horizonte: se limitó");
    }
    if profile.profiled_months_ago > PROFILE_STALE_MONTHS {
        findings.push("perfil desactualizado: hay que reperfilar antes de recomendar");
    }
    if house > 50 {
        findings.push("sobrepeso de productos propios sin justificación");
    }

    // Un perfil viejo no produce recomendación. La vida del cliente cambió
    // aunque el sistema no se enterase.
    let suitable = equity <= cap && profile.profiled_months_ago <= PROFILE_STALE_MONTHS;

    Advice {
        portfolio,
        equity_weight: equity,
        house_product_share: house,
        suitable,
        why: format!("horizonte de {} años: la exposición variable se limita al {cap} %", profile.horizon_years),
        findings,
        model_version: MODEL_VERSION,
        not_financial_advice: true,
    }
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn profile(horizon: u32, tolerance: u8, months_ago: u32) -> ClientProfile {
    ClientProfile {
        client: "cli-sintetico-1".into(),
        horizon_years: horizon,
        risk_tolerance: tolerance,
        profiled_months_ago: months_ago,
    }
}

pub fn report() -> CaseReport {
    let mut checks = Vec::new();

    // 1. Horizonte corto con tolerancia alta: manda el horizonte.
    let advice = advise(&profile(3, 5, 1), 0);
    checks.push(Check::new(
        "cliente con horizonte de 3 años que se declara muy tolerante al riesgo",
        "el horizonte manda sobre la tolerancia declarada: quien necesita el dinero pronto no puede esperar a que el mercado vuelva",
        "30+idonea",
        format!("{}+{}", advice.equity_weight, if advice.suitable { "idonea" } else { "no-idonea" }),
    ));

    // 2. Horizonte largo: la tolerancia sí se respeta.
    let advice = advise(&profile(20, 5, 1), 0);
    checks.push(Check::new(
        "el mismo cliente con horizonte de 20 años",
        "con tiempo por delante, la tolerancia declarada sí puede respetarse",
        "85",
        advice.equity_weight.to_string(),
    ));

    // 3. Perfil de hace tres años.
    let advice = advise(&profile(10, 3, 36), 0);
    checks.push(Check::new(
        "el perfil se hizo hace tres años y nadie lo revisó",
        "un perfil viejo describe a otra persona: no se recomienda sobre él",
        "no-idonea",
        if advice.suitable { "idonea" } else { "no-idonea" }.to_string(),
    ));

    // 4. Sobrepeso de producto propio.
    let advice = advise(&profile(20, 5, 1), 60);
    checks.push(Check::new(
        "la cartera propuesta es 60 % de productos de la casa",
        "el sesgo hacia productos propios se mide siempre: lo que no se puede es no medirlo",
        "60+con-hallazgo",
        format!(
            "{}+{}",
            advice.house_product_share,
            if advice.findings.is_empty() { "sin-hallazgo" } else { "con-hallazgo" }
        ),
    ));

    // 5. Los pesos suman 100 exacto.
    let advice = advise(&profile(5, 2, 1), 10);
    let total: u32 = advice.portfolio.iter().map(|holding| holding.weight).sum();
    checks.push(Check::new(
        "una cartera cualquiera, sumados sus pesos",
        "con enteros los pesos suman 100 exacto; con coma flotante casi nunca",
        "100",
        total.to_string(),
    ));

    // 6. Toda recomendación lleva la versión que la tomó.
    checks.push(Check::new(
        "cualquier recomendación, mirada de cerca",
        "sin la versión del modelo no se puede reconstruir la decisión años después",
        "1.4.2+true",
        format!("{}+{}", advice.model_version, advice.not_financial_advice),
    ));

    CaseReport::new("CM-07", "Robo-advisor", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn mismos_datos_misma_cartera() {
        let uno = advise(&profile(7, 4, 2), 5);
        let dos = advise(&profile(7, 4, 2), 5);
        assert_eq!(uno.equity_weight, dos.equity_weight);
        assert_eq!(uno.model_version, dos.model_version);
    }

    #[test]
    fn el_horizonte_mas_corto_es_el_mas_conservador() {
        assert!(equity_cap(1) < equity_cap(3));
        assert!(equity_cap(3) < equity_cap(20));
    }
}
