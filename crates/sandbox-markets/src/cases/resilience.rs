//! CM-14 · Resiliencia operacional.
//!
//! En un sistema financiero, **seguir funcionando mal es peor que detenerse**:
//! un motor con precios erróneos ejecuta operaciones reales a precios que no
//! existen, y deshacerlas después es caro, lento y a veces imposible.
//!
//! Dos detalles que casi siempre se hacen mal: las **cancelaciones siguen
//! vivas** durante un incidente —impedir cancelar mientras el mercado se mueve
//! atrapa a los clientes en sus posiciones—, y el replay tiene que ser
//! idempotente o duplica lo que intentaba reparar.

use super::{CaseReport, Check, Maturity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Incident {
    DatabaseDown,
    DuplicatedMessages,
    HighLatency,
    OrderEngineDisconnected,
    StalePrices,
    CustodianUnavailable,
    CompromisedCredentials,
    FaultyDeployment,
}

impl Incident {
    /// ¿Puede corromper datos si se sigue operando?
    const fn threatens_integrity(self) -> bool {
        matches!(
            self,
            Self::DatabaseDown
                | Self::DuplicatedMessages
                | Self::OrderEngineDisconnected
                | Self::CompromisedCredentials
        )
    }

    /// ¿Degrada la calidad de la ejecución sin corromper nada?
    const fn threatens_execution(self) -> bool {
        matches!(self, Self::StalePrices | Self::HighLatency | Self::CustodianUnavailable | Self::FaultyDeployment)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Response {
    /// Se para lo afectado de inmediato.
    KillSwitch,
    /// Se apaga por partes, en un orden decidido antes.
    Degrade,
    /// Se sigue, con alerta.
    Alert,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Handling {
    pub response: Response,
    pub stopped: Vec<&'static str>,
    /// Lo que sigue vivo importa tanto como lo que se para.
    pub kept_running: Vec<&'static str>,
    pub detected_at_ms: u32,
    pub acted_at_ms: u32,
    pub data_integrity_preserved: bool,
}

/// Cuánto se tarda en decidir una vez detectado. Fijo y medible.
const DECISION_MS: u32 = 40;

/// Decide qué hacer con un incidente.
///
/// Las condiciones están escritas de antemano: un kill switch que se decide
/// sobre la marcha no se sabe si funciona hasta que hace falta, que es cuando
/// no se puede probar.
pub fn handle(incident: Incident, detected_at_ms: u32) -> Handling {
    let response = if incident.threatens_integrity() {
        Response::KillSwitch
    } else if incident.threatens_execution() {
        Response::Degrade
    } else {
        Response::Alert
    };

    let (stopped, kept_running) = match response {
        Response::KillSwitch => (
            vec!["nuevas órdenes", "ejecuciones", "liquidación"],
            // Cancelar y consultar saldos sobreviven siempre. Es la parte que
            // más se olvida y la que más daña al cliente cuando falta.
            vec!["cancelaciones", "consulta de saldos"],
        ),
        Response::Degrade => (
            vec!["nuevas órdenes en el instrumento afectado"],
            vec!["cancelaciones", "consulta de saldos", "órdenes en el resto de instrumentos"],
        ),
        Response::Alert => (Vec::new(), vec!["todo"]),
    };

    Handling {
        response,
        stopped,
        kept_running,
        detected_at_ms,
        acted_at_ms: detected_at_ms + DECISION_MS,
        data_integrity_preserved: response != Response::Alert || !incident.threatens_integrity(),
    }
}

/// Un evento del registro que solo añade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoggedEvent {
    pub id: String,
    pub payload: i128,
}

/// Reaplica los eventos que quedaron a medias.
///
/// Idempotente por identificador: repetir un evento del registro **no lo aplica
/// dos veces**. Sin esto, el replay duplica justo lo que venía a reparar.
pub fn replay(events: &[LoggedEvent], already_applied: &mut Vec<String>) -> (usize, i128) {
    let mut applied = 0;
    let mut total = 0;
    for event in events {
        if already_applied.contains(&event.id) {
            continue;
        }
        already_applied.push(event.id.clone());
        applied += 1;
        total += event.payload;
    }
    (applied, total)
}

// ── Escenarios ───────────────────────────────────────────────────────────────

pub fn report() -> CaseReport {
    let mut checks = Vec::new();

    // 1. Incidente que amenaza la integridad.
    let handling = handle(Incident::DatabaseDown, 340);
    checks.push(Check::new(
        "la base de datos deja de responder",
        "operar sobre un estado que no se puede persistir es peor que detenerse",
        "killswitch",
        format!("{:?}", handling.response).to_lowercase(),
    ));

    // 2. Las cancelaciones sobreviven al kill switch.
    checks.push(Check::new(
        "el kill switch está activo y un cliente quiere cancelar",
        "impedir cancelar mientras el mercado se mueve atrapa a los clientes en sus posiciones",
        "true",
        handling.kept_running.contains(&"cancelaciones").to_string(),
    ));

    // 3. Incidente de calidad: se degrada, no se para todo.
    let handling = handle(Incident::StalePrices, 120);
    checks.push(Check::new(
        "llegan precios obsoletos de un proveedor",
        "apagar entero lo que se podía apagar por partes es un daño que se elige",
        "degrade+1 detenido",
        format!("{:?}+{} detenido", handling.response, handling.stopped.len()).to_lowercase(),
    ));

    // 4. Se miden los dos tiempos.
    checks.push(Check::new(
        "el mismo incidente, preguntando cuánto se tardó",
        "detectar y detener son las dos métricas del caso: todo lo demás se deriva de ellas",
        "120+160",
        format!("{}+{}", handling.detected_at_ms, handling.acted_at_ms),
    ));

    // 5. Replay que no duplica.
    let events = vec![LoggedEvent { id: "e1".into(), payload: 100 }, LoggedEvent { id: "e2".into(), payload: 200 }];
    let mut applied = Vec::new();
    let (first, total) = replay(&events, &mut applied);
    let (second, _) = replay(&events, &mut applied);
    checks.push(Check::new(
        "se hace replay dos veces del mismo registro",
        "un replay que no es idempotente duplica justo lo que venía a reparar",
        "2+300+0",
        format!("{first}+{total}+{second}"),
    ));

    // 6. Credenciales comprometidas: integridad, kill switch inmediato.
    let handling = handle(Incident::CompromisedCredentials, 10);
    checks.push(Check::new(
        "se detectan credenciales comprometidas",
        "aquí cada segundo cuenta, y la decisión ya estaba escrita antes de que pasara",
        "killswitch+true",
        format!("{:?}+{}", handling.response, handling.data_integrity_preserved).to_lowercase(),
    ));

    CaseReport::new("CM-14", "Resiliencia operacional", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn cancelar_sobrevive_a_cualquier_incidente() {
        for incident in [
            Incident::DatabaseDown,
            Incident::DuplicatedMessages,
            Incident::HighLatency,
            Incident::OrderEngineDisconnected,
            Incident::StalePrices,
            Incident::CustodianUnavailable,
            Incident::CompromisedCredentials,
            Incident::FaultyDeployment,
        ] {
            let handling = handle(incident, 0);
            assert!(
                handling.kept_running.contains(&"cancelaciones") || handling.kept_running.contains(&"todo"),
                "{incident:?} dejó a los clientes sin poder cancelar"
            );
        }
    }

    #[test]
    fn el_mismo_incidente_se_maneja_igual_siempre() {
        assert_eq!(handle(Incident::StalePrices, 5).response, handle(Incident::StalePrices, 5).response);
    }
}
