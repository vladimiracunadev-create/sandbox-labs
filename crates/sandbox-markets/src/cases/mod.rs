//! Los casos de mercado de capitales, uno por módulo.
//!
//! # Por qué todos tienen la misma forma
//!
//! Cada caso responde a una pregunta distinta —¿cuadra la custodia?, ¿se
//! respeta la prioridad precio-tiempo?, ¿se puede explicar esta decisión?— pero
//! todos comparten la única forma que hace comprobable un modelo de negocio:
//! **una afirmación y un escenario que la pone a prueba**.
//!
//! Un caso que solo se ejecuta dice «pasó algo». Un caso que declara de antemano
//! qué debe salir y luego se compara con lo que salió dice si funciona. Esa
//! comparación es [`Check`], y es lo único que este módulo impone.
//!
//! # Estado
//!
//! Los casos de aquí están en `prototype` salvo que su ficha diga otra cosa:
//! hay código y hay escenarios que se ejecutan, pero **no hay evidencia firmada
//! por ejecución**, que es lo que hace falta para `verified`. La regla completa
//! está en el ROADMAP.
//!
//! # Avisos que no son decorativos
//!
//! Dinero, instrumentos, participantes y datos **simulados**. Sin conexión a
//! ningún banco ni medio de pago, **sin autorización de ninguna autoridad**, y
//! nada de lo que salga de aquí es una recomendación de inversión. Tampoco hay
//! datos personales reales en ningún escenario.

use serde::Serialize;

pub mod clearing;
pub mod consent;
pub mod corporate_actions;
pub mod credit;
pub mod crowdfunding;
pub mod fraud;
pub mod intermediation;
pub mod kyc;
pub mod margin;
pub mod market_data;
pub mod model_governance;
pub mod regulatory_entry;
pub mod reporting;
pub mod resilience;
pub mod robo_advisor;
pub mod routing;
pub mod surveillance;
pub mod tokenization;
pub mod wind_down;

/// Una afirmación puesta a prueba.
///
/// `expected` se escribe **antes** de mirar el resultado. Si se ajustara
/// después, la comprobación no comprobaría nada: diría que el código hace lo
/// que hace.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    /// Qué situación se plantea.
    pub scenario: &'static str,
    /// Qué enseña. Un check sin esto es un test, no un escenario.
    pub teaches: &'static str,
    pub expected: String,
    pub actual: String,
}

impl Check {
    pub fn new(
        scenario: &'static str,
        teaches: &'static str,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self { scenario, teaches, expected: expected.into(), actual: actual.into() }
    }

    pub fn passed(&self) -> bool {
        self.expected == self.actual
    }
}

/// El estado real de un caso, con las mismas palabras que su ficha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Maturity {
    /// Hay código y escenarios que se ejecutan, sin evidencia firmada.
    Prototype,
    /// Se ejecuta y hay una prueba concreta que lo demuestra.
    Functional,
}

impl Maturity {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Prototype => "prototype",
            Self::Functional => "functional",
        }
    }
}

/// Lo que un caso responde cuando se le pregunta si funciona.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseReport {
    /// `CM-04`, `CM-10`…
    pub id: &'static str,
    pub title: &'static str,
    pub maturity: Maturity,
    pub checks: Vec<Check>,
}

impl CaseReport {
    pub fn new(id: &'static str, title: &'static str, maturity: Maturity, checks: Vec<Check>) -> Self {
        Self { id, title, maturity, checks }
    }

    pub fn failures(&self) -> Vec<&Check> {
        self.checks.iter().filter(|check| !check.passed()).collect()
    }

    pub fn passed(&self) -> bool {
        self.checks.iter().all(Check::passed)
    }
}

/// Todos los casos con código, en orden de catálogo.
///
/// Los que no aparecen aquí están en `planned` y su ficha lo dice. No hay
/// entradas vacías «de relleno»: un caso listado es un caso que se ejecuta.
pub fn all() -> Vec<CaseReport> {
    vec![
        regulatory_entry::report(),
        crowdfunding::report(),
        routing::report(),
        intermediation::report(),
        credit::report(),
        robo_advisor::report(),
        tokenization::report(),
        surveillance::report(),
        clearing::report(),
        consent::report(),
        reporting::report(),
        wind_down::report(),
        resilience::report(),
        market_data::report(),
        kyc::report(),
        corporate_actions::report(),
        margin::report(),
        fraud::report(),
        model_governance::report(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_caso_declara_al_menos_un_escenario() {
        for report in all() {
            assert!(!report.checks.is_empty(), "{} no tiene escenarios", report.id);
        }
    }

    #[test]
    fn todos_los_casos_hacen_lo_que_declaran() {
        let mut broken = Vec::new();
        for report in all() {
            for failure in report.failures() {
                broken.push(format!(
                    "{} · {} — esperaba «{}», obtuve «{}»",
                    report.id, failure.scenario, failure.expected, failure.actual
                ));
            }
        }
        // Se enumeran todos: arreglar de uno en uno cuando fallan cinco es
        // cinco vueltas de compilación en vez de una.
        assert!(
            broken.is_empty(),
            "{}",
            broken.join(
                "
"
            )
        );
    }

    #[test]
    fn no_hay_identificadores_repetidos() {
        let mut seen = Vec::new();
        for report in all() {
            assert!(!seen.contains(&report.id), "identificador repetido: {}", report.id);
            seen.push(report.id);
        }
    }
}
