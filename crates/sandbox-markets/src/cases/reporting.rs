//! CM-12 · Reportería regulatoria y SupTech.
//!
//! Un reporte consolida datos de todos los demás sistemas, así que es donde se
//! hacen visibles las inconsistencias que cada uno por separado no notaba.
//!
//! La regla del caso: **corregir sí, reescribir la historia no**. Una
//! corrección genera una versión nueva que apunta a la anterior por su huella.
//! Alterar la anterior rompe el enlace y se detecta.

use super::{CaseReport, Check, Maturity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Un reporte de un periodo. Las secciones llevan sus totales.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub id: String,
    pub period: String,
    /// El esquema tiene fecha de vigencia: los formatos cambian y el pasado no
    /// se reescribe para encajar en el formato nuevo.
    pub schema_version: u32,
    pub version: u32,
    pub previous_version: Option<u32>,
    /// Huella de la versión anterior. Es lo que hace la cadena verificable.
    pub previous_digest: Option<String>,
    pub sections: BTreeMap<String, i128>,
    /// Total declarado aparte, que tiene que cuadrar con la suma de secciones.
    pub declared_total: i128,
    /// Identificadores de las operaciones incluidas, para detectar duplicados.
    pub operation_ids: Vec<String>,
    pub correction_reason: Option<String>,
}

impl Report {
    /// Huella del contenido. Simple y determinista: lo que importa aquí es que
    /// **cambie cuando cambia el contenido**, no la fuerza criptográfica — la
    /// firma real vive en el núcleo del proyecto.
    pub fn digest(&self) -> String {
        let mut accumulator: u64 = 1469598103934665603;
        let mut absorb = |bytes: &[u8]| {
            for byte in bytes {
                accumulator ^= u64::from(*byte);
                accumulator = accumulator.wrapping_mul(1099511628211);
            }
        };
        absorb(self.id.as_bytes());
        absorb(&self.version.to_le_bytes());
        for (name, value) in &self.sections {
            absorb(name.as_bytes());
            absorb(&value.to_le_bytes());
        }
        format!("{accumulator:016x}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Finding {
    /// La suma de secciones no coincide con el total declarado.
    Unbalanced { declared: i128, computed: i128 },
    /// La misma operación contada dos veces.
    Duplicated { operation: String },
    /// Se envía con un esquema que ya no está vigente.
    OutdatedSchema { used: u32, current: u32 },
    /// El enlace con la versión anterior está roto.
    BrokenChain { version: u32 },
    /// Una versión anterior cambió después de emitirse la siguiente.
    HistoryAltered { version: u32 },
}

impl Finding {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Unbalanced { .. } => "unbalanced",
            Self::Duplicated { .. } => "duplicated",
            Self::OutdatedSchema { .. } => "outdated-schema",
            Self::BrokenChain { .. } => "broken-chain",
            Self::HistoryAltered { .. } => "history-altered",
        }
    }
}

/// El esquema vigente hoy. Cambia con el tiempo, y por eso se compara.
pub const CURRENT_SCHEMA: u32 = 2;

/// Valida un reporte antes de firmarlo. Un reporte descuadrado no se envía.
pub fn validate(report: &Report) -> Vec<Finding> {
    let mut findings = Vec::new();

    let computed: i128 = report.sections.values().sum();
    if computed != report.declared_total {
        findings.push(Finding::Unbalanced { declared: report.declared_total, computed });
    }

    let mut seen = Vec::new();
    for operation in &report.operation_ids {
        if seen.contains(operation) {
            findings.push(Finding::Duplicated { operation: operation.clone() });
        } else {
            seen.push(operation.clone());
        }
    }

    if report.schema_version != CURRENT_SCHEMA {
        findings.push(Finding::OutdatedSchema { used: report.schema_version, current: CURRENT_SCHEMA });
    }

    findings
}

/// Verifica la cadena de versiones de un mismo reporte.
///
/// Recorre en orden y comprueba que cada versión apunta a la huella real de la
/// anterior. Si alguien modificó una versión ya emitida, el enlace deja de
/// coincidir y aparece aquí.
pub fn verify_chain(versions: &[Report]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for window in versions.windows(2) {
        let (previous, current) = (&window[0], &window[1]);
        match (&current.previous_digest, current.previous_version) {
            (Some(digest), Some(version)) if version == previous.version => {
                if *digest != previous.digest() {
                    findings.push(Finding::HistoryAltered { version: previous.version });
                }
            }
            _ => findings.push(Finding::BrokenChain { version: current.version }),
        }
    }
    findings
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn report_v1() -> Report {
    Report {
        id: "rep-2026-07".into(),
        period: "2026-07".into(),
        schema_version: CURRENT_SCHEMA,
        version: 1,
        previous_version: None,
        previous_digest: None,
        sections: BTreeMap::from([
            ("custodia".to_string(), 12_500_000_000_i128),
            ("efectivo".to_string(), 500_000_000),
        ]),
        declared_total: 13_000_000_000,
        operation_ids: vec!["op-1".into(), "op-2".into()],
        correction_reason: None,
    }
}

fn correction_of(previous: &Report) -> Report {
    let mut corrected = previous.clone();
    corrected.version = previous.version + 1;
    corrected.previous_version = Some(previous.version);
    corrected.previous_digest = Some(previous.digest());
    corrected.sections.insert("garantias".into(), 100_000_000);
    corrected.declared_total += 100_000_000;
    corrected.correction_reason = Some("observación OBS-14: faltaba la sección de garantías".into());
    corrected
}

fn kinds(findings: &[Finding]) -> String {
    let mut names: Vec<&str> = findings.iter().map(Finding::kind).collect();
    names.sort_unstable();
    names.dedup();
    names.join(",")
}

pub fn report() -> CaseReport {
    let mut checks = Vec::new();

    // 1. Reporte que cuadra.
    checks.push(Check::new(
        "un reporte cuyas secciones suman el total declarado",
        "el caso normal tiene que pasar, o la validación no distingue nada",
        "",
        kinds(&validate(&report_v1())),
    ));

    // 2. Descuadre entre secciones y total.
    let mut unbalanced = report_v1();
    unbalanced.declared_total += 1;
    checks.push(Check::new(
        "el total declarado no coincide con la suma de secciones",
        "un descuadre significa que dos sistemas internos no coinciden: ahí está el problema real",
        "unbalanced",
        kinds(&validate(&unbalanced)),
    ));

    // 3. Operación duplicada.
    let mut duplicated = report_v1();
    duplicated.operation_ids.push("op-1".into());
    checks.push(Check::new(
        "la misma operación aparece dos veces",
        "consolidar de varias fuentes duplica sin que nadie lo note",
        "duplicated",
        kinds(&validate(&duplicated)),
    ));

    // 4. Esquema antiguo.
    let mut old = report_v1();
    old.schema_version = 1;
    checks.push(Check::new(
        "se envía con el esquema del año pasado",
        "los formatos regulatorios cambian, y enviar con el viejo es un rechazo seguro",
        "outdated-schema",
        kinds(&validate(&old)),
    ));

    // 5. Corrección legítima: cadena intacta.
    let v1 = report_v1();
    let v2 = correction_of(&v1);
    checks.push(Check::new(
        "una corrección que emite la versión 2 apuntando a la 1",
        "corregir es legítimo; lo que no lo es es hacerlo sin dejar rastro",
        "",
        kinds(&verify_chain(&[v1.clone(), v2.clone()])),
    ));

    // 6. Alteración histórica: se toca la v1 después de emitir la v2.
    let mut altered = v1.clone();
    altered.sections.insert("custodia".into(), 9_000_000_000);
    checks.push(Check::new(
        "alguien modifica la versión 1 después de haber emitido la 2",
        "el enlace por huella es lo que delata que el pasado cambió",
        "history-altered",
        kinds(&verify_chain(&[altered, v2])),
    ));

    // 7. Versión sin enlace.
    let mut orphan = correction_of(&v1);
    orphan.previous_digest = None;
    checks.push(Check::new(
        "una versión 2 que no dice de cuál viene",
        "sin enlace no hay historia que reconstruir, solo documentos sueltos",
        "broken-chain",
        kinds(&verify_chain(&[v1, orphan])),
    ));

    CaseReport::new("CM-12", "Reportería regulatoria y SupTech", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn la_huella_cambia_cuando_cambia_el_contenido() {
        let original = report_v1();
        let mut touched = original.clone();
        touched.sections.insert("custodia".into(), 1);
        assert_ne!(original.digest(), touched.digest());
    }

    #[test]
    fn la_huella_es_estable_entre_ejecuciones() {
        assert_eq!(report_v1().digest(), report_v1().digest());
    }

    #[test]
    fn el_envio_es_simulado() {
        // No hay función de envío, y es deliberado: este simulador no tiene
        // conectividad con ninguna autoridad y no la tendrá.
        let validated = validate(&report_v1());
        assert!(validated.is_empty());
    }
}
