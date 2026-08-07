//! CM-15 · KYC, AML y sanciones.
//!
//! Este caso tiene una particularidad que lo separa del resto: **el error tiene
//! dos direcciones y las dos hacen daño**. El falso negativo deja entrar dinero
//! ilícito; el falso positivo cierra la cuenta de alguien honesto, a veces sin
//! explicación y sin recurso.
//!
//! De ahí la regla dura del módulo: **ninguna medida automática sobre una
//! persona**. Una coincidencia es un motivo para mirar, no una conclusión.
//!
//! Identidades y listas **sintéticas**. Sin datos personales reales en ningún
//! sitio, tampoco como datos de prueba.

use super::{CaseReport, Check, Maturity};
use serde::{Deserialize, Serialize};

/// Un eslabón de la cadena societaria.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ownership {
    pub entity: String,
    /// Participación en puntos porcentuales.
    pub share: u32,
    pub is_person: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Customer {
    pub id: String,
    /// Cadena de propiedad, del cliente hacia arriba.
    pub ownership_chain: Vec<Ownership>,
    pub pep: bool,
    /// Ingreso declarado mensual, para contrastar operaciones.
    pub declared_monthly_income: i128,
}

impl Customer {
    /// Quién manda de verdad. `None` cuando la cadena no llega a una persona,
    /// y **eso es el hallazgo**, no un dato que falte.
    pub fn ultimate_beneficial_owner(&self) -> Option<&str> {
        self.ownership_chain
            .iter()
            .filter(|link| link.is_person && link.share >= 25)
            .max_by_key(|link| link.share)
            .map(|link| link.entity.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RiskLevel {
    Bajo,
    Medio,
    Alto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Alert {
    /// Coincidencia con una lista **simulada**, con su grado de confianza.
    SanctionsNameMatch { customer: String, confidence: u32, match_type: &'static str },
    /// No se llega a una persona física.
    BeneficialOwnerUnknown { customer: String },
    /// Operación que no encaja con el perfil declarado.
    InconsistentWithProfile { customer: String, amount: i128, declared_income: i128 },
}

impl Alert {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::SanctionsNameMatch { .. } => "sanctions-name-match",
            Self::BeneficialOwnerUnknown { .. } => "beneficial-owner-unknown",
            Self::InconsistentWithProfile { .. } => "inconsistent-with-profile",
        }
    }
}

/// El resultado de una evaluación. Nunca una medida.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Assessment {
    pub risk: RiskLevel,
    pub alerts: Vec<Alert>,
    /// Siempre `true` cuando hay alertas.
    pub requires_human_review: bool,
    /// Siempre `false`. Ninguna medida se toma sola sobre una persona.
    pub automatic_measure_taken: bool,
}

/// Cuántas veces el ingreso declarado hace que una operación merezca mirarse.
const INCOME_MULTIPLE: i128 = 12;

/// Compara un nombre contra una lista sintética.
///
/// Devuelve confianza, no un sí o un no: un apellido común basta para parecerse
/// a un nombre de una lista, y tratar ese parecido como certeza es lo que cierra
/// cuentas de gente honesta.
pub fn name_match(name: &str, list: &[&str]) -> Option<(u32, &'static str)> {
    let normalized = name.to_lowercase();
    for entry in list {
        let candidate = entry.to_lowercase();
        if normalized == candidate {
            return Some((100, "exacta"));
        }
        // Parecido pobre pero honesto: comparte el primer componente del
        // nombre. Suficiente para generar la alerta, insuficiente para actuar.
        let (first, other) = (normalized.split_whitespace().next(), candidate.split_whitespace().next());
        if first.is_some() && first == other {
            return Some((62, "fonética"));
        }
    }
    None
}

/// Evalúa un cliente y, si se indica, una operación suya.
pub fn assess(customer: &Customer, name: &str, sanctions_list: &[&str], operation_amount: Option<i128>) -> Assessment {
    let mut alerts = Vec::new();

    if let Some((confidence, match_type)) = name_match(name, sanctions_list) {
        alerts.push(Alert::SanctionsNameMatch { customer: customer.id.clone(), confidence, match_type });
    }
    if customer.ultimate_beneficial_owner().is_none() {
        alerts.push(Alert::BeneficialOwnerUnknown { customer: customer.id.clone() });
    }
    if let Some(amount) = operation_amount {
        if amount > customer.declared_monthly_income * INCOME_MULTIPLE {
            alerts.push(Alert::InconsistentWithProfile {
                customer: customer.id.clone(),
                amount,
                declared_income: customer.declared_monthly_income,
            });
        }
    }

    let risk = if customer.pep || alerts.len() > 1 {
        RiskLevel::Alto
    } else if alerts.is_empty() {
        RiskLevel::Bajo
    } else {
        RiskLevel::Medio
    };

    Assessment { risk, requires_human_review: !alerts.is_empty(), alerts, automatic_measure_taken: false }
}

// ── Escenarios ───────────────────────────────────────────────────────────────

const LISTA_SIMULADA: [&str; 2] = ["Nombre Sancionado Simulado", "Otro Sancionado Simulado"];

fn customer(with_person: bool, pep: bool) -> Customer {
    Customer {
        id: "cli-sintetico-1".into(),
        ownership_chain: if with_person {
            vec![
                Ownership { entity: "empresa-sim-A".into(), share: 60, is_person: false },
                Ownership { entity: "persona-sintetica-1".into(), share: 60, is_person: true },
            ]
        } else {
            vec![
                Ownership { entity: "empresa-sim-A".into(), share: 60, is_person: false },
                Ownership { entity: "fideicomiso-sim".into(), share: 40, is_person: false },
            ]
        },
        pep,
        declared_monthly_income: 1_000_000,
    }
}

fn kinds(alerts: &[Alert]) -> String {
    let mut names: Vec<&str> = alerts.iter().map(Alert::kind).collect();
    names.sort_unstable();
    names.dedup();
    names.join(",")
}

pub fn report() -> CaseReport {
    let mut checks = Vec::new();

    // 1. Cliente limpio.
    let assessment = assess(&customer(true, false), "Persona Sintetica Uno", &LISTA_SIMULADA, None);
    checks.push(Check::new(
        "cliente con beneficiario final identificado y sin coincidencias",
        "el alta normal tiene que pasar sin fricción, o el control se convierte en un peaje",
        "bajo+",
        format!("{:?}+{}", assessment.risk, kinds(&assessment.alerts)).to_lowercase(),
    ));

    // 2. Coincidencia exacta con lista simulada.
    let assessment = assess(&customer(true, false), "Nombre Sancionado Simulado", &LISTA_SIMULADA, None);
    let confidence = assessment
        .alerts
        .iter()
        .find_map(|alert| match alert {
            Alert::SanctionsNameMatch { confidence, .. } => Some(*confidence),
            _ => None,
        })
        .unwrap_or(0);
    checks.push(Check::new(
        "el nombre coincide exactamente con una entrada de la lista simulada",
        "una coincidencia exacta tiene confianza alta y aun así no decide nada por sí sola",
        "100+true",
        format!("{confidence}+{}", assessment.requires_human_review),
    ));

    // 3. Falso positivo: solo coincide el primer nombre.
    let assessment = assess(&customer(true, false), "Nombre Distinto Apellido", &LISTA_SIMULADA, None);
    let confidence = assessment
        .alerts
        .iter()
        .find_map(|alert| match alert {
            Alert::SanctionsNameMatch { confidence, .. } => Some(*confidence),
            _ => None,
        })
        .unwrap_or(0);
    checks.push(Check::new(
        "una persona honesta cuyo primer nombre coincide con el de la lista",
        "un apellido común no es una condena: la confianza baja se ve en la alerta",
        "62",
        confidence.to_string(),
    ));

    // 4. Ninguna medida se toma sola.
    checks.push(Check::new(
        "cualquier evaluación con alerta, mirada de cerca",
        "el coste del falso positivo lo paga una persona, así que decide una persona",
        "false+true",
        format!("{}+{}", assessment.automatic_measure_taken, assessment.requires_human_review),
    ));

    // 5. Beneficiario final desconocido.
    let assessment = assess(&customer(false, false), "Otra Persona", &LISTA_SIMULADA, None);
    checks.push(Check::new(
        "la cadena societaria termina en un fideicomiso, sin persona detrás",
        "no llegar a una persona es el hallazgo, no un dato que falte por rellenar",
        "beneficial-owner-unknown",
        kinds(&assessment.alerts),
    ));

    // 6. Operación muy por encima del perfil.
    let assessment = assess(&customer(true, false), "Otra Persona", &LISTA_SIMULADA, Some(50_000_000));
    checks.push(Check::new(
        "una operación de cincuenta veces el ingreso mensual declarado",
        "no es ilícita por ser grande: es incoherente con lo declarado, y eso se pregunta",
        "inconsistent-with-profile",
        kinds(&assessment.alerts),
    ));

    // 7. Un PEP sube el riesgo sin ser una acusación.
    let assessment = assess(&customer(true, true), "Otra Persona", &LISTA_SIMULADA, None);
    checks.push(Check::new(
        "el cliente es una persona expuesta políticamente",
        "ser PEP no es un delito: es un motivo para vigilar más de cerca",
        "alto",
        format!("{:?}", assessment.risk).to_lowercase(),
    ));

    CaseReport::new("CM-15", "KYC, AML y sanciones", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn nunca_hay_medida_automatica() {
        for pep in [true, false] {
            for chain in [true, false] {
                let assessment = assess(&customer(chain, pep), "Nombre Sancionado Simulado", &LISTA_SIMULADA, Some(1));
                assert!(!assessment.automatic_measure_taken);
            }
        }
    }

    #[test]
    fn una_lista_vacia_no_produce_coincidencias() {
        assert!(name_match("Cualquiera", &[]).is_none());
    }

    #[test]
    fn una_participacion_pequena_no_es_beneficiario_final() {
        let customer = Customer {
            id: "x".into(),
            ownership_chain: vec![Ownership { entity: "persona".into(), share: 10, is_person: true }],
            pep: false,
            declared_monthly_income: 1,
        };
        assert!(customer.ultimate_beneficial_owner().is_none());
    }
}
