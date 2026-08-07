//! CM-13 · Salida ordenada.
//!
//! Cerrar una empresa que maneja dinero ajeno no es apagar los servidores: es
//! devolver cada peso a su dueño y poder demostrarlo.
//!
//! Los pasos llevan **orden obligatorio**, y no por burocracia: cancelar
//! pendientes antes de dejar de aceptar órdenes nuevas crea obligaciones
//! mientras intentas cumplir las viejas.

use super::{CaseReport, Check, Maturity};
use serde::{Deserialize, Serialize};

/// Los pasos, en el único orden en que funcionan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    StopOnboarding,
    StopNewOrders,
    CancelPending,
    Liquidate,
    RepayClients,
    TransferAssets,
    ExportHistories,
    Notify,
    CloseIntegrations,
    FinalReport,
}

impl Step {
    pub const ORDER: [Self; 10] = [
        Self::StopOnboarding,
        Self::StopNewOrders,
        Self::CancelPending,
        Self::Liquidate,
        Self::RepayClients,
        Self::TransferAssets,
        Self::ExportHistories,
        Self::Notify,
        Self::CloseIntegrations,
        Self::FinalReport,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::StopOnboarding => "stop-onboarding",
            Self::StopNewOrders => "stop-new-orders",
            Self::CancelPending => "cancel-pending",
            Self::Liquidate => "liquidate",
            Self::RepayClients => "repay-clients",
            Self::TransferAssets => "transfer-assets",
            Self::ExportHistories => "export-histories",
            Self::Notify => "notify",
            Self::CloseIntegrations => "close-integrations",
            Self::FinalReport => "final-report",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientBalance {
    pub client: String,
    /// Lo que los libros dicen que se le debe.
    pub owed: i128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Finding {
    /// Se hizo un paso antes que otro que debía ir delante.
    OutOfOrder { step: &'static str, before: &'static str },
    /// Quedan clientes sin cobrar. El cierre **no ha terminado**.
    ClientsPending { count: usize },
    /// No alcanza para todos.
    Shortfall { missing: i128 },
    /// Sin historial, el cliente pierde la prueba de lo que tenía.
    HistoriesNotExported,
}

impl Finding {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::OutOfOrder { .. } => "out-of-order",
            Self::ClientsPending { .. } => "clients-pending",
            Self::Shortfall { .. } => "shortfall",
            Self::HistoriesNotExported => "histories-not-exported",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalReport {
    pub clients_repaid: usize,
    pub clients_pending: usize,
    pub distributed: i128,
    pub findings: Vec<Finding>,
    /// Cierre completo son dos cosas a la vez: nadie pendiente y sin hallazgos.
    pub complete: bool,
}

/// Ejecuta el cierre con los pasos en el orden dado y el efectivo disponible.
///
/// El reparto cuando no alcanza es **a prorrata de lo debido**, con la regla
/// publicada. Repartir por orden de llegada haría que los primeros cobrasen
/// todo y los últimos nada.
pub fn wind_down(
    executed: &[Step],
    balances: &[ClientBalance],
    available: i128,
    export_histories: bool,
) -> FinalReport {
    let mut findings = Vec::new();

    // El orden se comprueba comparando posiciones, no nombres.
    for (index, step) in executed.iter().enumerate() {
        let expected = Step::ORDER.iter().position(|candidate| candidate == step).unwrap_or(usize::MAX);
        for earlier in executed.iter().skip(index + 1) {
            let other = Step::ORDER.iter().position(|candidate| candidate == earlier).unwrap_or(usize::MAX);
            if other < expected {
                findings.push(Finding::OutOfOrder { step: step.label(), before: earlier.label() });
            }
        }
    }

    let owed: i128 = balances.iter().map(|balance| balance.owed).sum();
    let mut distributed = 0;
    let mut pending = 0;

    if available >= owed {
        distributed = owed;
    } else {
        findings.push(Finding::Shortfall { missing: owed - available });
        // A prorrata: cada uno recibe su parte proporcional.
        for balance in balances {
            if owed > 0 {
                distributed += balance.owed * available / owed;
            }
        }
        pending = balances.len();
    }

    if !export_histories {
        findings.push(Finding::HistoriesNotExported);
    }

    if pending > 0 {
        findings.push(Finding::ClientsPending { count: pending });
    }

    FinalReport {
        clients_repaid: balances.len() - pending,
        clients_pending: pending,
        distributed,
        complete: pending == 0 && findings.is_empty(),
        findings,
    }
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn balances() -> Vec<ClientBalance> {
    vec![
        ClientBalance { client: "cli-1".into(), owed: 1_000_000 },
        ClientBalance { client: "cli-2".into(), owed: 3_000_000 },
    ]
}

fn kinds(findings: &[Finding]) -> String {
    let mut names: Vec<&str> = findings.iter().map(Finding::kind).collect();
    names.sort_unstable();
    names.dedup();
    names.join(",")
}

pub fn report() -> CaseReport {
    let mut checks = Vec::new();

    // 1. Cierre completo.
    let outcome = wind_down(&Step::ORDER, &balances(), 4_000_000, true);
    checks.push(Check::new(
        "hay dinero para todos y los pasos se hacen en orden",
        "cierre completo son dos cosas a la vez: nadie pendiente y ningún hallazgo",
        "true+0",
        format!("{}+{}", outcome.complete, outcome.clients_pending),
    ));

    // 2. Pasos fuera de orden.
    let wrong = vec![Step::CancelPending, Step::StopNewOrders, Step::StopOnboarding];
    let outcome = wind_down(&wrong, &balances(), 4_000_000, true);
    checks.push(Check::new(
        "se cancelan pendientes antes de dejar de aceptar órdenes nuevas",
        "el orden no es burocracia: al revés se crean obligaciones mientras intentas cumplir las viejas",
        "out-of-order",
        kinds(&outcome.findings),
    ));

    // 3. Faltante: reparto a prorrata.
    let outcome = wind_down(&Step::ORDER, &balances(), 2_000_000, true);
    checks.push(Check::new(
        "solo hay la mitad de lo que se debe",
        "se reparte a prorrata con la regla publicada, nunca por orden de llegada",
        "2000000+con-faltante",
        format!(
            "{}+{}",
            outcome.distributed,
            if outcome.findings.iter().any(|finding| matches!(finding, Finding::Shortfall { .. })) {
                "con-faltante"
            } else {
                "sin-faltante"
            }
        ),
    ));

    // 4. Faltante: el cierre NO está completo.
    checks.push(Check::new(
        "el mismo cierre con faltante, preguntado si terminó",
        "los servidores apagados no son un cierre terminado",
        "false",
        outcome.complete.to_string(),
    ));

    // 5. Sin exportar historiales.
    let outcome = wind_down(&Step::ORDER, &balances(), 4_000_000, false);
    checks.push(Check::new(
        "se devuelve todo pero no se exportan los historiales",
        "sin historial el cliente pierde la prueba de lo que tenía",
        "histories-not-exported",
        kinds(&outcome.findings),
    ));

    // 6. Nadie recibe de más ni se pierde nada por el camino.
    let outcome = wind_down(&Step::ORDER, &balances(), 3_000_000, true);
    checks.push(Check::new(
        "hay tres cuartas partes de lo que se debe",
        "a prorrata reparte todo lo disponible: lo que no se reparte es dinero de clientes retenido sin motivo",
        "3000000",
        outcome.distributed.to_string(),
    ));

    CaseReport::new("CM-13", "Salida ordenada", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn el_orden_correcto_no_produce_hallazgos_de_orden() {
        let outcome = wind_down(&Step::ORDER, &balances(), 10_000_000, true);
        assert!(!outcome.findings.iter().any(|finding| matches!(finding, Finding::OutOfOrder { .. })));
    }

    #[test]
    fn sin_clientes_el_cierre_es_completo() {
        let outcome = wind_down(&Step::ORDER, &[], 0, true);
        assert!(outcome.complete);
    }
}
