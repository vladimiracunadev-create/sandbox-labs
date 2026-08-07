//! CM-19 · Fraude y toma de cuentas.
//!
//! En una toma de cuenta **la autenticación funcionó**: quien entra tiene las
//! credenciales. Todos los controles de acceso dicen que sí. La única señal
//! disponible es el comportamiento.
//!
//! Dos ideas que el caso defiende. Autenticar no es autorizar: cada acción
//! sensible se evalúa por su propio riesgo en ese momento, no por la sesión. Y
//! la respuesta se **gradúa** — **una hora de espera en un cambio de
//! beneficiario no molesta a un cliente legítimo y arruina un fraude**.

use super::{CaseReport, Check, Maturity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Login,
    ChangeBeneficiary,
    Withdraw,
}

/// Un evento de cuenta **sintética**. La ubicación es una etiqueta, no unas
/// coordenadas: basta para detectar una sesión imposible y no modela datos
/// personales ni siquiera de forma simulada.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub account: String,
    pub action: Action,
    pub amount: i128,
    pub device_first_seen: bool,
    pub geo_label: String,
    pub previous_geo_label: String,
    pub minutes_since_previous: u32,
    /// Antigüedad del beneficiario destino, en horas.
    pub beneficiary_age_hours: u32,
    /// Retiro habitual de esta cuenta, para tener con qué comparar.
    pub typical_withdrawal: i128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Response {
    Allow,
    /// Pedir un factor más. Molesta poco y para bastante.
    StepUp,
    /// Retrasar la operación. La medida más subestimada.
    Delay,
    BlockAndReview,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    /// De 0 a 100.
    pub risk_score: u32,
    pub signals: Vec<&'static str>,
    pub response: Response,
    pub human_review_required: bool,
    /// Lo que cuesta equivocarse. Va **junto a la decisión**, a propósito:
    /// obliga a escribir a quién perjudica un bloqueo antes de decidirlo.
    pub false_positive_cost: &'static str,
}

/// Minutos por debajo de los cuales dos ubicaciones distintas son imposibles.
const IMPOSSIBLE_SESSION_MINUTES: u32 = 60;

/// Horas por debajo de las cuales un beneficiario es «reciente».
const RECENT_BENEFICIARY_HOURS: u32 = 24;

/// Evalúa **una acción**, no una sesión.
///
/// Ninguna señal decide por sí sola: el riesgo se compone. Un dispositivo nuevo
/// es la vida normal de cualquiera que cambie de teléfono; un dispositivo nuevo
/// más un beneficiario de hace dos horas más un retiro fuera de lo habitual, no.
pub fn assess(event: &Event) -> Assessment {
    let mut signals = Vec::new();
    let mut score = 0;

    if event.device_first_seen {
        signals.push("NewDevice");
        score += 20;
    }
    if event.geo_label != event.previous_geo_label && event.minutes_since_previous < IMPOSSIBLE_SESSION_MINUTES {
        signals.push("ImpossibleSession");
        // Pesa más que las demás: un dispositivo nuevo se explica cambiando de
        // teléfono, pero nadie está en dos sitios a la vez.
        score += 40;
    }
    if event.action != Action::Login && event.beneficiary_age_hours < RECENT_BENEFICIARY_HOURS {
        signals.push("RecentBeneficiary");
        score += 25;
    }
    if event.action == Action::Withdraw && event.typical_withdrawal > 0 && event.amount > event.typical_withdrawal * 5 {
        signals.push("AmountAnomaly");
        score += 25;
    }

    let score = score.min(100);

    // La respuesta se gradúa por riesgo **y por lo que la acción puede
    // deshacer**: un login se puede revisar después; un retiro, no.
    let response = match (event.action, score) {
        (_, 0..=19) => Response::Allow,
        (Action::Login, 20..=49) => Response::StepUp,
        (Action::Login, _) => Response::BlockAndReview,
        (Action::ChangeBeneficiary, 20..=44) => Response::StepUp,
        (Action::ChangeBeneficiary, 45..=74) => Response::Delay,
        (Action::ChangeBeneficiary, _) => Response::BlockAndReview,
        (Action::Withdraw, 20..=39) => Response::StepUp,
        (Action::Withdraw, 40..=69) => Response::Delay,
        (Action::Withdraw, _) => Response::BlockAndReview,
    };

    Assessment {
        risk_score: score,
        signals,
        response,
        human_review_required: response == Response::BlockAndReview,
        false_positive_cost: match response {
            Response::Allow => "ninguno",
            Response::StepUp => "el cliente hace un paso más",
            Response::Delay => "el cliente espera para operar",
            Response::BlockAndReview => "el cliente no puede retirar hasta la revisión",
        },
    }
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn event(action: Action) -> Event {
    Event {
        account: "cta-sintetica-1".into(),
        action,
        amount: 100_000,
        device_first_seen: false,
        geo_label: "zona-A".into(),
        previous_geo_label: "zona-A".into(),
        minutes_since_previous: 600,
        beneficiary_age_hours: 5_000,
        typical_withdrawal: 200_000,
    }
}

pub fn report() -> CaseReport {
    let mut checks = Vec::new();

    // 1. Operación normal.
    let assessment = assess(&event(Action::Withdraw));
    checks.push(Check::new(
        "un retiro normal desde el dispositivo de siempre",
        "sin línea base, cualquier sistema de fraude molesta a todo el mundo",
        "allow+0",
        format!("{:?}+{}", assessment.response, assessment.risk_score).to_lowercase(),
    ));

    // 2. Dispositivo nuevo, solo.
    let mut nuevo = event(Action::Withdraw);
    nuevo.device_first_seen = true;
    let assessment = assess(&nuevo);
    checks.push(Check::new(
        "el mismo retiro desde un teléfono nuevo",
        "cambiar de teléfono es la vida normal: una señal sola no puede bloquear a nadie",
        "stepup",
        format!("{:?}", assessment.response).to_lowercase(),
    ));

    // 3. Sesión imposible.
    let mut imposible = event(Action::Withdraw);
    imposible.geo_label = "zona-B".into();
    imposible.minutes_since_previous = 12;
    let assessment = assess(&imposible);
    checks.push(Check::new(
        "dos accesos en zonas distintas separados por doce minutos",
        "nadie se mueve tan rápido: la señal es geométrica, no sospechosa",
        "delay+impossiblesession",
        format!("{:?}+{}", assessment.response, assessment.signals.join(",")).to_lowercase(),
    ));

    // 4. La secuencia clásica: beneficiario reciente + dispositivo nuevo + monto alto.
    let mut ataque = event(Action::Withdraw);
    ataque.device_first_seen = true;
    ataque.beneficiary_age_hours = 2;
    ataque.amount = 4_500_000;
    let assessment = assess(&ataque);
    checks.push(Check::new(
        "dispositivo nuevo, beneficiario de hace dos horas y un retiro de veinte veces lo habitual",
        "el riesgo se compone: son las tres juntas las que significan algo",
        "blockandreview+true",
        format!("{:?}+{}", assessment.response, assessment.human_review_required).to_lowercase(),
    ));

    // 5. Cambio de beneficiario: se retrasa antes que bloquear.
    let mut cambio = event(Action::ChangeBeneficiary);
    cambio.device_first_seen = true;
    cambio.beneficiary_age_hours = 1;
    let assessment = assess(&cambio);
    checks.push(Check::new(
        "cambio de beneficiario desde un dispositivo nuevo",
        "una hora de espera no molesta a un cliente legítimo y arruina un fraude",
        "delay",
        format!("{:?}", assessment.response).to_lowercase(),
    ));

    // 6. El coste del falso positivo viaja con la decisión.
    checks.push(Check::new(
        "cualquier decisión, mirada de cerca",
        "escribir a quién perjudica un bloqueo antes de decidirlo cambia la decisión",
        "true",
        (!assessment.false_positive_cost.is_empty()).to_string(),
    ));

    CaseReport::new("CM-19", "Fraude y toma de cuentas", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn ninguna_senal_sola_bloquea() {
        for mutate in [
            (|event: &mut Event| event.device_first_seen = true) as fn(&mut Event),
            |event: &mut Event| event.beneficiary_age_hours = 1,
            |event: &mut Event| event.amount = 99_999_999,
        ] {
            let mut single = event(Action::Withdraw);
            mutate(&mut single);
            assert_ne!(assess(&single).response, Response::BlockAndReview, "una señal sola no puede bloquear");
        }
    }

    #[test]
    fn un_login_limpio_no_pide_nada() {
        assert_eq!(assess(&event(Action::Login)).response, Response::Allow);
    }
}
