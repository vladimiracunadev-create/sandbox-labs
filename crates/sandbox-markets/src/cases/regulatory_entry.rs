//! CM-00 · Entrada al sandbox regulatorio.
//!
//! La puerta de la familia. No pregunta si una empresa es buena: pregunta **de
//! quién es el dinero en cada momento** y qué pasa si la empresa desaparece
//! mañana. De esa respuesta salen la clasificación, los límites y las
//! obligaciones que los demás casos tienen que respetar.
//!
//! El resultado nunca es una autorización. Está en el tipo: [`Resolution`]
//! lleva `not_an_authorization` y siempre vale `true`.

use super::{CaseReport, Check, Maturity};
use crate::money::{Currency, Money};
use serde::{Deserialize, Serialize};

/// Qué hace la empresa con el dinero y con las órdenes de sus clientes.
///
/// La clasificación es lo que decide el régimen entero, y casi todos los
/// problemas regulatorios serios empiezan por hacerla mal: «solo conectamos
/// inversionistas con proyectos» es intermediación en cuanto se toca el dinero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Service {
    /// Guarda efectivo o instrumentos de clientes.
    Custody,
    /// Ejecuta o enruta órdenes por cuenta de clientes.
    OrderRouting,
    /// Recomienda productos concretos a personas concretas.
    Advisory,
    /// Emite o representa instrumentos.
    Issuance,
    /// Servicio auxiliar sin contacto con dinero ni órdenes.
    Auxiliary,
}

impl Service {
    /// El caso de esta misma familia que prueba la actividad.
    pub const fn proving_case(self) -> &'static str {
        match self {
            Self::Custody => "CM-03",
            Self::OrderRouting => "CM-02",
            Self::Advisory => "CM-07",
            Self::Issuance => "CM-08",
            Self::Auxiliary => "—",
        }
    }
}

/// Por dónde pasa cada peso. **La pregunta central de la postulación.**
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoneyFlow {
    pub from: String,
    pub to: String,
    /// Si el dinero se queda en una cuenta a nombre de la empresa, es custodia
    /// aunque nadie la llame así.
    pub held_by_applicant: bool,
    /// ¿Está segregado del dinero propio?
    pub segregated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindDownPlan {
    pub documented: bool,
    /// Días declarados para devolver todo. Sin ensayo, un plan es un documento.
    pub max_days: u32,
    pub rehearsed: bool,
}

/// La postulación. Lo que la empresa dice que hace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Application {
    pub applicant: String,
    /// Jurisdicción declarada. Chile es la implementación inicial; el modelo
    /// admite otras porque **las reglas cambian y no se codifican como verdad
    /// permanente**.
    pub jurisdiction: String,
    pub declared_services: Vec<Service>,
    pub money_flows: Vec<MoneyFlow>,
    pub retail_clients: bool,
    pub governance_documented: bool,
    pub wind_down: WindDownPlan,
}

/// Un riesgo con el control que lo mitiga y el caso que lo prueba.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Risk {
    pub id: &'static str,
    pub risk: &'static str,
    pub severity: Severity,
    pub control: &'static str,
    pub proving_case: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    Alta,
    Media,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Approved,
    /// Sí, pero con límites y obligaciones escritas. Es la salida más
    /// interesante: convierte una opinión en política como código.
    Conditional,
    Rejected,
}

impl Outcome {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Conditional => "conditional",
            Self::Rejected => "rejected",
        }
    }
}

/// La resolución. Con límites que los demás casos pueden leer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub outcome: Outcome,
    /// Lo que la empresa hace **de verdad**, que puede no coincidir con lo que
    /// declaró.
    pub classification: Vec<Service>,
    pub risks: Vec<Risk>,
    pub limits: Option<Limits>,
    pub obligations: Vec<&'static str>,
    pub rationale: Vec<String>,
    /// Obligatorio y siempre `true`. Ninguna salida de este simulador puede
    /// presentarse como autorización de ninguna autoridad.
    pub not_an_authorization: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    pub max_client_funds: Money,
    pub max_clients: u32,
}

/// Clasifica, evalúa y resuelve.
///
/// El orden importa: primero se determina qué hace la empresa **por sus flujos
/// de dinero**, no por cómo se describe. Un solicitante que dice ser «plataforma
/// tecnológica» y retiene fondos de clientes es un custodio.
pub fn evaluate(application: &Application) -> Resolution {
    let mut classification = application.declared_services.clone();
    let mut rationale = Vec::new();

    // La clasificación se corrige mirando los flujos, no la descripción.
    if application.money_flows.iter().any(|flow| flow.held_by_applicant) && !classification.contains(&Service::Custody)
    {
        classification.push(Service::Custody);
        rationale.push(
            "Retiene fondos de clientes en cuentas propias: es custodia aunque la postulación no la declare"
                .to_string(),
        );
    }
    classification.sort();
    classification.dedup();

    let mut risks = Vec::new();
    if classification.contains(&Service::Custody) {
        let commingled = application.money_flows.iter().any(|flow| flow.held_by_applicant && !flow.segregated);
        risks.push(Risk {
            id: "R-01",
            risk: "mezcla de activos de clientes con los propios",
            severity: if commingled { Severity::Alta } else { Severity::Media },
            control: "conciliación con el invariante de custodia",
            proving_case: "CM-03",
        });
        if commingled {
            rationale.push(
                "Hay un flujo retenido y sin segregar: si la empresa cae, ese dinero entra en la masa de la quiebra"
                    .to_string(),
            );
        }
    }
    if classification.contains(&Service::OrderRouting) {
        risks.push(Risk {
            id: "R-02",
            risk: "ejecución que favorece a la casa antes que al cliente",
            severity: Severity::Alta,
            control: "prioridad precio-tiempo y decisión explicable",
            proving_case: "CM-02",
        });
    }
    if application.retail_clients {
        risks.push(Risk {
            id: "R-03",
            risk: "inversionistas no calificados con menos capacidad de juicio",
            severity: Severity::Alta,
            control: "límites de exposición y divulgaciones obligatorias",
            proving_case: "CM-01",
        });
    }

    // Los tres motivos de rechazo. Ninguno es formal: los tres significan que
    // no se puede responder a «¿de quién es el dinero?».
    let mut blockers = Vec::new();
    if !application.wind_down.documented {
        blockers.push("Sin plan de salida ordenada. Se pide al entrar, no al salir".to_string());
    }
    if !application.governance_documented {
        blockers.push("Sin gobierno documentado: no hay a quién exigir".to_string());
    }
    if application.money_flows.is_empty() {
        blockers.push("No declara por dónde pasa el dinero, que es la pregunta central".to_string());
    }

    if !blockers.is_empty() {
        rationale.extend(blockers);
        return Resolution {
            outcome: Outcome::Rejected,
            classification,
            risks,
            limits: None,
            obligations: Vec::new(),
            rationale,
            not_an_authorization: true,
        };
    }

    // Condicionada cuando hay algo que arreglar pero nada que impida empezar
    // con límites. Es la salida útil, y la que más se parece a la realidad.
    // Custodiar dinero ajeno **siempre** lleva condiciones: la conciliación no
    // es un extra para los casos dudosos, es lo que hace verificable la
    // segregación. Sin ella, «los activos están segregados» es una afirmación
    // sin forma de comprobarse.
    let needs_conditions = classification.contains(&Service::Custody)
        || risks.iter().any(|risk| risk.severity == Severity::Alta)
        || !application.wind_down.rehearsed;

    let mut obligations = Vec::new();
    if needs_conditions {
        if classification.contains(&Service::Custody) {
            obligations.push("conciliación de custodia con cada cierre · CM-03");
        }
        if !application.wind_down.rehearsed {
            obligations.push("ensayar el plan de salida antes de superar los límites · CM-13");
            rationale.push(
                "El plan de salida está documentado pero no ensayado: un plan sin ensayar es un documento".to_string(),
            );
        }
        obligations.push("reporte periódico · CM-12");
    }

    Resolution {
        outcome: if needs_conditions { Outcome::Conditional } else { Outcome::Approved },
        classification,
        risks,
        limits: needs_conditions
            .then(|| Limits { max_client_funds: Money::new(50_000_000_000, Currency::Clp), max_clients: 500 }),
        obligations,
        rationale,
        not_an_authorization: true,
    }
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn flow(held: bool, segregated: bool) -> MoneyFlow {
    MoneyFlow { from: "cliente".into(), to: "cuenta".into(), held_by_applicant: held, segregated }
}

fn base() -> Application {
    Application {
        applicant: "Fintech de ejemplo SpA".into(),
        jurisdiction: "CL".into(),
        declared_services: vec![Service::Auxiliary],
        money_flows: vec![flow(false, true)],
        retail_clients: false,
        governance_documented: true,
        wind_down: WindDownPlan { documented: true, max_days: 30, rehearsed: true },
    }
}

pub fn report() -> CaseReport {
    let mut checks = Vec::new();

    // 1. Una plataforma que no toca el dinero y lo tiene todo en regla.
    let clean = evaluate(&base());
    checks.push(Check::new(
        "servicio auxiliar sin contacto con el dinero",
        "el régimen ligero existe, y se llega a él no tocando el dinero de nadie",
        "approved",
        clean.outcome.label(),
    ));

    // 2. La que se describe como tecnológica y retiene fondos.
    let mut disguised = base();
    disguised.money_flows = vec![flow(true, true)];
    let resolved = evaluate(&disguised);
    checks.push(Check::new(
        "se declara auxiliar pero retiene fondos de clientes",
        "la clasificación sale de los flujos de dinero, no de cómo se describe la empresa",
        "custody+conditional",
        format!(
            "{}+{}",
            if resolved.classification.contains(&Service::Custody) { "custody" } else { "sin-custody" },
            resolved.outcome.label()
        ),
    ));

    // 3. Retiene y no segrega: el riesgo sube a alto y se dice por qué.
    let mut commingled = base();
    commingled.declared_services = vec![Service::Custody];
    commingled.money_flows = vec![flow(true, false)];
    let resolved = evaluate(&commingled);
    let severity = resolved.risks.iter().find(|risk| risk.id == "R-01").map(|risk| risk.severity);
    checks.push(Check::new(
        "custodia sin segregar",
        "no segregar convierte el dinero del cliente en un crédito contra una empresa insolvente",
        "Some(Alta)",
        format!("{severity:?}"),
    ));

    // 4. Sin plan de salida: no se aprueba.
    let mut no_exit = base();
    no_exit.wind_down.documented = false;
    checks.push(Check::new(
        "sin plan de salida ordenada",
        "quien no sabe explicar cómo devolvería el dinero probablemente no lo ha separado bien",
        "rejected",
        evaluate(&no_exit).outcome.label(),
    ));

    // 5. Plan documentado pero sin ensayar: condicionada con obligación.
    let mut unrehearsed = base();
    unrehearsed.wind_down.rehearsed = false;
    let resolved = evaluate(&unrehearsed);
    checks.push(Check::new(
        "plan de salida documentado pero nunca ensayado",
        "un plan sin ensayar es un documento: se aprueba con la obligación de ensayarlo",
        "conditional+con-obligaciones",
        format!(
            "{}+{}",
            resolved.outcome.label(),
            if resolved.obligations.is_empty() { "sin-obligaciones" } else { "con-obligaciones" }
        ),
    ));

    // 6. Ninguna resolución puede presentarse como autorización.
    let all_resolutions = [clean, evaluate(&disguised), evaluate(&commingled), evaluate(&no_exit)];
    checks.push(Check::new(
        "cualquier resolución, mirada de cerca",
        "un simulador no autoriza nada, y eso va en el tipo y no en una nota al pie",
        "true",
        all_resolutions.iter().all(|resolution| resolution.not_an_authorization).to_string(),
    ));

    CaseReport::new("CM-00", "Entrada al sandbox regulatorio", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn el_rechazo_explica_su_motivo() {
        let mut application = base();
        application.governance_documented = false;
        let resolution = evaluate(&application);
        assert_eq!(resolution.outcome, Outcome::Rejected);
        assert!(!resolution.rationale.is_empty(), "un rechazo sin motivo no se puede recurrir");
    }

    #[test]
    fn la_condicionada_trae_limites_que_otros_casos_pueden_leer() {
        let mut application = base();
        application.retail_clients = true;
        let resolution = evaluate(&application);
        assert_eq!(resolution.outcome, Outcome::Conditional);
        let limits = resolution.limits.expect("una condicionada sin límites no condiciona nada");
        assert_eq!(limits.max_client_funds.currency(), Currency::Clp);
    }

    #[test]
    fn cada_riesgo_apunta_al_caso_que_lo_prueba() {
        let mut application = base();
        application.declared_services = vec![Service::Custody, Service::OrderRouting];
        application.money_flows = vec![flow(true, false)];
        let resolution = evaluate(&application);
        assert!(!resolution.risks.is_empty());
        for risk in &resolution.risks {
            assert_ne!(risk.proving_case, "", "un riesgo sin caso que lo pruebe es una frase");
        }
    }
}
