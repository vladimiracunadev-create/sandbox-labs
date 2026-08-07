//! CM-20 · Gobierno de modelos e IA financiera.
//!
//! Es el caso **transversal**: no prueba una actividad, prueba cómo se gobierna
//! cualquier modelo que ya aparece en los demás —el robo-advisor de CM-07, el
//! scoring de CM-06, el fraude de CM-19, la vigilancia de CM-09, el
//! enrutamiento de CM-04.
//!
//! Tres preguntas que hay que poder responder años después: qué versión
//! decidió, con qué datos, y quién se hace responsable. El *drift* es el fallo
//! más silencioso de los tres: nada falla, no hay error en ningún registro, y
//! el modelo simplemente acierta cada vez menos porque el mundo se movió.

use super::{CaseReport, Check, Maturity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelVersion {
    pub model: String,
    pub version: String,
    /// Huella del conjunto de entrenamiento. Sin esto no se puede explicar un
    /// sesgo: no se sabe con qué aprendió.
    pub dataset_digest: String,
    pub accuracy_pct: u32,
    /// Diferencia máxima de resultado entre grupos comparables, en puntos.
    pub max_group_disparity_pct: u32,
    pub approved_by: Option<String>,
    /// A qué versión se vuelve si esta falla.
    pub rollback_to: Option<String>,
}

/// Umbral de sesgo. Por encima, no sale a producción.
pub const BIAS_THRESHOLD_PCT: u32 = 5;

/// Umbral de deriva. Índice de estabilidad en centésimas: 20 son 0,20.
pub const DRIFT_THRESHOLD: u32 = 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Finding {
    /// Sale a producción sin que nadie firme.
    NotApproved { version: String },
    /// Trato distinto entre grupos comparables.
    BiasAboveThreshold { version: String, disparity_pct: u32 },
    /// No hay a dónde volver.
    NoRollbackTarget { version: String },
    /// El mundo cambió y el modelo no.
    DriftDetected { version: String, index: u32, threshold: u32 },
    /// Una decisión sin la versión que la tomó no se puede reconstruir.
    DecisionWithoutVersion { decision: String },
}

impl Finding {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::NotApproved { .. } => "not-approved",
            Self::BiasAboveThreshold { .. } => "bias-above-threshold",
            Self::NoRollbackTarget { .. } => "no-rollback-target",
            Self::DriftDetected { .. } => "drift-detected",
            Self::DecisionWithoutVersion { .. } => "decision-without-version",
        }
    }
}

/// Una decisión tomada en producción por algún modelo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Decision {
    pub id: String,
    /// La versión que la tomó. `None` es un hallazgo, no un hueco.
    pub model_version: Option<String>,
}

/// Comprueba si una versión puede salir a producción.
pub fn gate(version: &ModelVersion) -> Vec<Finding> {
    let mut findings = Vec::new();
    if version.approved_by.is_none() {
        findings.push(Finding::NotApproved { version: version.version.clone() });
    }
    if version.max_group_disparity_pct > BIAS_THRESHOLD_PCT {
        findings.push(Finding::BiasAboveThreshold {
            version: version.version.clone(),
            disparity_pct: version.max_group_disparity_pct,
        });
    }
    if version.rollback_to.is_none() {
        findings.push(Finding::NoRollbackTarget { version: version.version.clone() });
    }
    findings
}

/// Vigila una versión ya en producción.
pub fn monitor(version: &ModelVersion, drift_index: u32, decisions: &[Decision]) -> Vec<Finding> {
    let mut findings = Vec::new();
    if drift_index > DRIFT_THRESHOLD {
        findings.push(Finding::DriftDetected {
            version: version.version.clone(),
            index: drift_index,
            threshold: DRIFT_THRESHOLD,
        });
    }
    for decision in decisions {
        if decision.model_version.is_none() {
            findings.push(Finding::DecisionWithoutVersion { decision: decision.id.clone() });
        }
    }
    findings
}

/// Volver atrás **también es una decisión**, así que también necesita firma.
pub fn rollback(version: &ModelVersion, approved_by: Option<&str>) -> Result<String, Finding> {
    if approved_by.is_none() {
        return Err(Finding::NotApproved { version: version.version.clone() });
    }
    version.rollback_to.clone().ok_or_else(|| Finding::NoRollbackTarget { version: version.version.clone() })
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn version(approved: bool, disparity: u32, rollback_to: Option<&str>) -> ModelVersion {
    ModelVersion {
        model: "robo-advisor".into(),
        version: "1.4.2".into(),
        dataset_digest: "sha256:sintetico-2026-06".into(),
        accuracy_pct: 87,
        max_group_disparity_pct: disparity,
        approved_by: approved.then(|| "comite-simulado".to_string()),
        rollback_to: rollback_to.map(str::to_string),
    }
}

fn kinds(findings: &[Finding]) -> String {
    let mut names: Vec<&str> = findings.iter().map(Finding::kind).collect();
    names.sort_unstable();
    names.dedup();
    names.join(",")
}

pub fn report() -> CaseReport {
    // 1. Versión que puede salir.
    let mut checks = vec![Check::new(
        "una versión aprobada, sin sesgo y con vuelta atrás",
        "el camino correcto tiene que existir, o el gobierno se convierte en un freno",
        "",
        kinds(&gate(&version(true, 3, Some("1.4.1")))),
    )];

    // 2. Sin aprobación humana.
    checks.push(Check::new(
        "una versión que sale a producción sin que nadie firme",
        "si nadie firma, nadie responde ante quien reclame",
        "not-approved",
        kinds(&gate(&version(false, 3, Some("1.4.1")))),
    ));

    // 3. Sesgo por encima del umbral.
    checks.push(Check::new(
        "la versión trata distinto a dos grupos comparables en 9 puntos",
        "un modelo no necesita usar una variable prohibida para discriminar: le basta con una que la aproxime",
        "bias-above-threshold",
        kinds(&gate(&version(true, 9, Some("1.4.1")))),
    ));

    // 4. Sin versión a la que volver.
    checks.push(Check::new(
        "una versión sin destino de rollback",
        "un modelo peor en producción y sin vuelta atrás es el fallo más caro del caso",
        "no-rollback-target",
        kinds(&gate(&version(true, 3, None))),
    ));

    // 5. Drift: nada falla y el modelo acierta menos.
    let production = version(true, 3, Some("1.4.1"));
    checks.push(Check::new(
        "el índice de deriva sube a 0,31 sin ningún error en los registros",
        "el drift es el fallo silencioso: el mundo se movió y el modelo no se enteró",
        "drift-detected",
        kinds(&monitor(&production, 31, &[])),
    ));

    // 6. Una decisión sin versión no se puede reconstruir.
    let decisions = vec![
        Decision { id: "d-1".into(), model_version: Some("1.4.2".into()) },
        Decision { id: "d-2".into(), model_version: None },
    ];
    checks.push(Check::new(
        "una decisión guardada sin la versión que la tomó",
        "sin la versión no hay forma de responder a un cliente que reclama dos años después",
        "decision-without-version",
        kinds(&monitor(&production, 5, &decisions)),
    ));

    // 7. Volver atrás también necesita firma.
    checks.push(Check::new(
        "se intenta hacer rollback sin aprobación",
        "volver atrás también es una decisión: cambiar de modelo a escondidas es el mismo problema al revés",
        "Err(not-approved)",
        match rollback(&production, None) {
            Ok(target) => format!("Ok({target})"),
            Err(finding) => format!("Err({})", finding.kind()),
        },
    ));

    // 8. Con firma, el rollback es inmediato.
    checks.push(Check::new(
        "el mismo rollback, aprobado por el comité",
        "volver atrás es cambiar un puntero: la parte cara es haber guardado la versión anterior",
        "Ok(1.4.1)",
        match rollback(&production, Some("comite-simulado")) {
            Ok(target) => format!("Ok({target})"),
            Err(finding) => format!("Err({})", finding.kind()),
        },
    ));

    CaseReport::new("CM-20", "Gobierno de modelos e IA financiera", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn el_umbral_de_sesgo_es_estricto_no_laxo() {
        // Justo en el umbral pasa; un punto por encima, no.
        assert!(gate(&version(true, BIAS_THRESHOLD_PCT, Some("1.4.1"))).is_empty());
        assert!(!gate(&version(true, BIAS_THRESHOLD_PCT + 1, Some("1.4.1"))).is_empty());
    }

    #[test]
    fn sin_decisiones_ni_deriva_no_hay_hallazgos() {
        assert!(monitor(&version(true, 1, Some("1.4.1")), 0, &[]).is_empty());
    }
}
