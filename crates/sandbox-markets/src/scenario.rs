//! Escenarios de custodia: un caso que se ejecuta, no que se lee.
//!
//! Un escenario es un fichero JSON con el estado inicial, el extracto del
//! custodio y lo que se espera que salga. Ejecutarlo produce un informe de
//! conciliación con los hallazgos.
//!
//! # Por qué el escenario declara lo que espera
//!
//! Sin `expected`, ejecutar un escenario solo dice «pasó algo». Con él, el
//! escenario es una afirmación comprobable: «con estos datos tienen que salir
//! estos hallazgos y no otros». Un escenario adverso que no detecta lo que venía
//! a detectar es un escenario roto, y así se ve.

use crate::custody::{CustodianStatement, CustodyBook, Finding, Owner, PendingMovement, Position, Reconciliation};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Un escenario reproducible.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    pub id: String,
    pub title: String,
    /// Qué enseña este escenario. Uno que no lo diga es un caso de prueba, no
    /// un escenario.
    pub teaches: String,
    /// Semilla. Hoy nada es aleatorio, y va igualmente: en cuanto algo lo sea,
    /// el escenario tiene que seguir siendo reproducible.
    #[serde(default)]
    pub seed: u64,
    pub positions: Vec<ScenarioPosition>,
    #[serde(default)]
    pub pending: Vec<ScenarioPending>,
    /// Lo que el custodio externo dice tener. Es un dato de fuera: si se
    /// derivara del registro, la conciliación no compararía nada.
    pub custodian_statement: BTreeMap<String, i128>,
    pub expected: Expectation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioPosition {
    /// Id del cliente, o `"house"` para el propio custodio.
    pub owner: String,
    pub instrument: String,
    pub units: i128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioPending {
    pub instrument: String,
    pub units: i128,
    #[serde(default)]
    pub reason: String,
}

/// Lo que el escenario afirma que va a pasar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Expectation {
    /// ¿Debe quedar conciliado?
    pub reconciled: bool,
    /// Tipos de hallazgo que tienen que aparecer, por nombre.
    #[serde(default)]
    pub findings: Vec<String>,
}

/// Resultado de ejecutar un escenario.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioOutcome {
    pub id: String,
    pub title: String,
    pub reconciled: bool,
    /// Hallazgos, ya legibles.
    pub findings: Vec<String>,
    /// ¿Salió lo que el escenario decía que saldría?
    pub matches_expectation: bool,
    /// En qué se desvió. Vacío cuando coincide.
    pub deviations: Vec<String>,
}

/// El nombre corto de un hallazgo, que es lo que el escenario declara.
pub fn finding_kind(finding: &Finding) -> &'static str {
    match finding {
        Finding::Shortfall { .. } => "shortfall",
        Finding::Surplus { .. } => "surplus",
        Finding::NegativeClientPosition { .. } => "negative-client-position",
        Finding::CommingledAccount { .. } => "commingled-account",
        Finding::UnexplainedPending { .. } => "unexplained-pending",
    }
}

/// Un hallazgo escrito para que lo lea una persona.
pub fn describe(finding: &Finding) -> String {
    match finding {
        Finding::Shortfall { instrument, registered, custodied, unexplained } => format!(
            "FALTANTE en {instrument}: los clientes tienen {registered} registradas, el custodio dice tener \
             {custodied}, y {unexplained} no las explica nadie"
        ),
        Finding::Surplus { instrument, registered, custodied, unexplained } => format!(
            "SOBRANTE en {instrument}: registradas {registered}, custodiadas {custodied}, {unexplained} de más. \
             No tranquiliza: el registro no describe la realidad"
        ),
        Finding::NegativeClientPosition { owner, instrument, units } => {
            format!("POSICIÓN NEGATIVA: {owner} figura con {units} de {instrument}, y un cliente no puede deber unidades que nunca tuvo")
        }
        Finding::CommingledAccount { instrument } => {
            format!("ACTIVOS MEZCLADOS en {instrument}: cuentas propias y de clientes sin separar")
        }
        Finding::UnexplainedPending { instrument, units } => {
            format!("PENDIENTE SIN MOTIVO: {units} de {instrument} en tránsito sin decir por qué. Eso no es una explicación")
        }
    }
}

impl Scenario {
    /// Ejecuta el escenario y compara con lo que declaró esperar.
    pub fn run(&self) -> ScenarioOutcome {
        let mut book = CustodyBook::new();
        for position in &self.positions {
            let owner = if position.owner == "house" { Owner::House } else { Owner::client(position.owner.clone()) };
            book.record(Position { owner, instrument: position.instrument.clone(), units: position.units });
        }
        for movement in &self.pending {
            book.expect_pending(PendingMovement {
                instrument: movement.instrument.clone(),
                units: movement.units,
                reason: movement.reason.clone(),
            });
        }
        let statement: CustodianStatement = self.custodian_statement.clone();
        let reconciliation = book.reconcile(&statement);
        self.compare(reconciliation)
    }

    fn compare(&self, reconciliation: Reconciliation) -> ScenarioOutcome {
        let mut deviations = Vec::new();
        if reconciliation.is_reconciled() != self.expected.reconciled {
            deviations.push(format!(
                "se esperaba conciliado={} y salió {}",
                self.expected.reconciled,
                reconciliation.is_reconciled()
            ));
        }
        let kinds: Vec<&str> = reconciliation.findings.iter().map(finding_kind).collect();
        for expected in &self.expected.findings {
            if !kinds.contains(&expected.as_str()) {
                // Lo importante de verdad: un escenario adverso que NO detecta
                // lo que venía a detectar está roto, y callarlo lo dejaría
                // pasando por bueno para siempre.
                deviations
                    .push(format!("no apareció el hallazgo «{expected}», que este escenario existe para provocar"));
            }
        }
        for kind in &kinds {
            if !self.expected.findings.iter().any(|expected| expected == kind) {
                deviations.push(format!("apareció «{kind}», que el escenario no declaraba"));
            }
        }
        ScenarioOutcome {
            id: self.id.clone(),
            title: self.title.clone(),
            reconciled: reconciliation.is_reconciled(),
            findings: reconciliation.findings.iter().map(describe).collect(),
            matches_expectation: deviations.is_empty(),
            deviations,
        }
    }

    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        serde_json::from_str(&content).map_err(|error| format!("{}: escenario inválido: {error}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scenario(json: serde_json::Value) -> Scenario {
        serde_json::from_value(json).expect("escenario de prueba válido")
    }

    fn base(statement: i128, expected_reconciled: bool, expected: &[&str]) -> Scenario {
        scenario(serde_json::json!({
            "id": "CM-03-TEST",
            "title": "prueba",
            "teaches": "prueba",
            "positions": [
                {"owner": "ana", "instrument": "CL:ACC:LAN", "units": 300},
                {"owner": "house", "instrument": "CL:ACC:LAN", "units": 1000}
            ],
            "custodianStatement": {"CL:ACC:LAN": statement},
            "expected": {"reconciled": expected_reconciled, "findings": expected}
        }))
    }

    #[test]
    fn a_scenario_that_gets_what_it_declared_matches() {
        let outcome = base(300, true, &[]).run();
        assert!(outcome.reconciled);
        assert!(outcome.matches_expectation, "{:?}", outcome.deviations);
    }

    #[test]
    fn an_adverse_scenario_that_detects_nothing_is_broken() {
        // El caso que importa: el escenario dice que va a provocar un faltante
        // y no lo provoca. Callarlo lo dejaría pasando por bueno para siempre.
        let outcome = base(300, false, &["shortfall"]).run();
        assert!(!outcome.matches_expectation);
        assert!(
            outcome.deviations.iter().any(|d| d.contains("no apareció el hallazgo «shortfall»")),
            "{:?}",
            outcome.deviations
        );
    }

    #[test]
    fn an_unexpected_finding_is_also_a_deviation() {
        // Salir «mejor» de lo declarado tampoco vale: significa que el escenario
        // ya no describe lo que ocurre.
        let outcome = base(100, false, &[]).run();
        assert!(!outcome.matches_expectation);
        assert!(outcome.deviations.iter().any(|d| d.contains("apareció «shortfall»")), "{:?}", outcome.deviations);
    }

    #[test]
    fn the_house_position_never_covers_the_shortfall() {
        let outcome = base(100, false, &["shortfall"]).run();
        assert!(!outcome.reconciled);
        assert!(outcome.findings[0].contains("FALTANTE"), "{:?}", outcome.findings);
        assert!(outcome.matches_expectation, "{:?}", outcome.deviations);
    }

    #[test]
    fn findings_are_written_for_a_person_to_read() {
        let outcome = base(100, false, &["shortfall"]).run();
        assert!(outcome.findings[0].contains("no las explica nadie"), "{:?}", outcome.findings);
    }
}
