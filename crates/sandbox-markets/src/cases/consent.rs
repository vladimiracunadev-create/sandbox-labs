//! CM-11 · Finanzas abiertas y consentimiento.
//!
//! El consentimiento es lo único que separa «un servicio que te ayuda» de «un
//! tercero con acceso permanente a tu vida financiera». Y se rompe siempre por
//! el mismo sitio: se comprueba **al conceder** y no **en cada consulta**. Si
//! solo se comprueba al conceder, revocar no significa nada.
//!
//! Sin credenciales reales: los certificados son simulados y locales.

use super::{CaseReport, Check, Maturity};
use serde::{Deserialize, Serialize};

/// Alcance concedido. Lista cerrada: `"balances"` y `"Balances"` serían dos
/// alcances distintos para un mapa, y ese error solo aparece en producción.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    AccountsBalances,
    AccountsTransactions,
    PaymentsInitiate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Vigente,
    Vencido,
    Revocado,
}

/// Un consentimiento con ciclo de vida, no una casilla marcada.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Consent {
    pub id: String,
    /// Usuario **sintético**.
    pub user: String,
    pub participant: String,
    pub scopes: Vec<Scope>,
    /// Día simulado en el que se concedió. Reloj simulado: tres meses de
    /// vigencia se prueban en milisegundos y siempre igual.
    pub granted_day: u32,
    pub expires_day: u32,
    pub revoked_day: Option<u32>,
}

impl Consent {
    pub fn status_on(&self, day: u32) -> Status {
        if self.revoked_day.is_some_and(|revoked| day >= revoked) {
            return Status::Revocado;
        }
        if day >= self.expires_day {
            return Status::Vencido;
        }
        Status::Vigente
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Query {
    pub consent_id: String,
    pub scope: Scope,
    pub day: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Outcome {
    /// Se sirve el dato, y queda registrado quién lo consultó.
    Served {
        scope: Scope,
    },
    ExpiredConsent {
        consent: String,
        day: u32,
    },
    RevokedConsent {
        consent: String,
        day: u32,
    },
    ScopeViolation {
        consent: String,
        requested: Scope,
    },
    UnknownConsent {
        consent: String,
    },
    /// El proveedor no responde. **No se finge tener el dato.**
    Unavailable {
        consent: String,
    },
}

impl Outcome {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Served { .. } => "served",
            Self::ExpiredConsent { .. } => "expired-consent",
            Self::RevokedConsent { .. } => "revoked-consent",
            Self::ScopeViolation { .. } => "scope-violation",
            Self::UnknownConsent { .. } => "unknown-consent",
            Self::Unavailable { .. } => "unavailable",
        }
    }
}

/// Atiende una consulta comprobando el consentimiento **en ese momento**.
pub fn serve(consents: &[Consent], query: &Query, provider_up: bool) -> Outcome {
    let Some(consent) = consents.iter().find(|candidate| candidate.id == query.consent_id) else {
        return Outcome::UnknownConsent { consent: query.consent_id.clone() };
    };

    match consent.status_on(query.day) {
        Status::Revocado => return Outcome::RevokedConsent { consent: consent.id.clone(), day: query.day },
        Status::Vencido => return Outcome::ExpiredConsent { consent: consent.id.clone(), day: query.day },
        Status::Vigente => {}
    }

    if !consent.scopes.contains(&query.scope) {
        return Outcome::ScopeViolation { consent: consent.id.clone(), requested: query.scope };
    }
    if !provider_up {
        return Outcome::Unavailable { consent: consent.id.clone() };
    }
    Outcome::Served { scope: query.scope }
}

/// Consultas por día a partir de las cuales conviene mirar.
const EXCESSIVE_PER_DAY: usize = 100;

/// Detecta consulta excesiva: puede ser un fallo del participante o una
/// recolección encubierta. En los dos casos hay que preguntar.
pub fn excessive(queries: &[Query]) -> bool {
    let days = queries.iter().map(|query| query.day).collect::<std::collections::BTreeSet<_>>().len().max(1);
    queries.len() / days > EXCESSIVE_PER_DAY
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn consent(id: &str, scopes: &[Scope], revoked: Option<u32>) -> Consent {
    Consent {
        id: id.into(),
        user: "usuario-sintetico-1".into(),
        participant: "app-sim-1".into(),
        scopes: scopes.to_vec(),
        granted_day: 0,
        expires_day: 90,
        revoked_day: revoked,
    }
}

fn query(day: u32, scope: Scope) -> Query {
    Query { consent_id: "cons-001".into(), scope, day }
}

pub fn report() -> CaseReport {
    let vigente = vec![consent("cons-001", &[Scope::AccountsBalances], None)];
    let revocado = vec![consent("cons-001", &[Scope::AccountsBalances], Some(30))];

    let mut checks = vec![
        // 1. Dentro de alcance y de plazo.
        Check::new(
            "consulta de saldos el día 10, con consentimiento vigente",
            "el caso normal tiene que funcionar, o el control sobra",
            "served",
            serve(&vigente, &query(10, Scope::AccountsBalances), true).kind().to_string(),
        ),
    ];

    // 2. Fuera de alcance.
    checks.push(Check::new(
        "se concedieron saldos y se piden movimientos",
        "el alcance no se amplía sin volver a preguntarle al usuario",
        "scope-violation",
        serve(&vigente, &query(10, Scope::AccountsTransactions), true).kind().to_string(),
    ));

    // 3. Vencido.
    checks.push(Check::new(
        "la misma consulta el día 120, con vigencia de 90",
        "un consentimiento sin fecha de fin es acceso permanente",
        "expired-consent",
        serve(&vigente, &query(120, Scope::AccountsBalances), true).kind().to_string(),
    ));

    // 4. Revocado: el fallo más grave si no se detecta.
    checks.push(Check::new(
        "el usuario revocó el día 30 y se consulta el día 31",
        "si el consentimiento solo se comprueba al conceder, revocar no significa nada",
        "revoked-consent",
        serve(&revocado, &query(31, Scope::AccountsBalances), true).kind().to_string(),
    ));

    // 5. Antes de revocar sí valía.
    checks.push(Check::new(
        "una consulta del día 29, antes de la revocación",
        "revocar corta hacia adelante: lo servido antes fue legítimo y queda registrado",
        "served",
        serve(&revocado, &query(29, Scope::AccountsBalances), true).kind().to_string(),
    ));

    // 6. Proveedor caído: no se finge.
    checks.push(Check::new(
        "el proveedor de datos no responde",
        "servir un dato viejo como si fuera actual es peor que no servir nada",
        "unavailable",
        serve(&vigente, &query(10, Scope::AccountsBalances), false).kind().to_string(),
    ));

    // 7. Consulta excesiva.
    let flood: Vec<Query> = (0..500).map(|_| query(5, Scope::AccountsBalances)).collect();
    checks.push(Check::new(
        "quinientas consultas en un solo día para un servicio que necesita unas pocas",
        "el volumen distingue un servicio de una recolección encubierta",
        "true",
        excessive(&flood).to_string(),
    ));

    CaseReport::new("CM-11", "Finanzas abiertas y consentimiento", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn un_consentimiento_inventado_no_sirve_datos() {
        let outcome = serve(&[], &query(1, Scope::AccountsBalances), true);
        assert_eq!(outcome.kind(), "unknown-consent");
    }

    #[test]
    fn la_revocacion_gana_al_vencimiento() {
        // Revocado el día 10 y vencido el 90: el día 50 es revocado, no vencido.
        let consents = vec![consent("cons-001", &[Scope::AccountsBalances], Some(10))];
        assert_eq!(serve(&consents, &query(50, Scope::AccountsBalances), true).kind(), "revoked-consent");
    }

    #[test]
    fn un_uso_normal_no_es_excesivo() {
        let normal: Vec<Query> = (0..20).map(|day| query(day, Scope::AccountsBalances)).collect();
        assert!(!excessive(&normal));
    }
}
