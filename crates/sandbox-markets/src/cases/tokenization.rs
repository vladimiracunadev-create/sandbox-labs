//! CM-08 · Tokenización de instrumentos.
//!
//! El invariante es hermano del de custodia: **las unidades emitidas no pueden
//! superar el respaldo**. Y hay una segunda regla que la gente olvida: el token
//! no es el activo, es una anotación. Si el registro legal dice que el dueño es
//! otro, **gana el registro legal**.
//!
//! Sin cadena de bloques: el registro es un libro que solo añade, local.

use super::{CaseReport, Check, Maturity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issuance {
    pub instrument: String,
    /// Valor del activo que respalda, en unidades mínimas.
    pub backing: i128,
    /// Valor de cada unidad emitida.
    pub unit_value: i128,
    /// Solo pueden tener unidades quienes cumplan esto.
    #[serde(default)]
    pub restricted_to_qualified: bool,
}

impl Issuance {
    /// Cuántas unidades admite el respaldo. Nada más.
    pub fn max_units(&self) -> i128 {
        if self.unit_value <= 0 {
            return 0;
        }
        self.backing / self.unit_value
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Holder {
    pub id: String,
    pub qualified: bool,
    pub frozen: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Finding {
    /// Más unidades que respaldo.
    OverIssuance { instrument: String, issued: i128, max_by_backing: i128 },
    /// El registro digital y el legal no coinciden.
    LegalDesync { instrument: String, digital_owner: String, legal_owner: String },
    /// Transferencia a quien no puede tener el instrumento.
    TransferRestricted { holder: String, reason: &'static str },
}

impl Finding {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::OverIssuance { .. } => "over-issuance",
            Self::LegalDesync { .. } => "legal-desync",
            Self::TransferRestricted { .. } => "transfer-restricted",
        }
    }
}

/// El registro de propiedad. Solo añade.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    balances: BTreeMap<String, i128>,
    issued: i128,
}

impl Registry {
    pub fn issued(&self) -> i128 {
        self.issued
    }

    pub fn balance(&self, holder: &str) -> i128 {
        self.balances.get(holder).copied().unwrap_or(0)
    }

    /// Emite. Falla si el respaldo no da: comprobar al cierre del día sería
    /// comprobar tarde.
    pub fn issue(&mut self, issuance: &Issuance, to: &str, units: i128) -> Result<(), Finding> {
        let after = self.issued + units;
        let max = issuance.max_units();
        if after > max {
            return Err(Finding::OverIssuance {
                instrument: issuance.instrument.clone(),
                issued: after,
                max_by_backing: max,
            });
        }
        self.issued = after;
        *self.balances.entry(to.to_string()).or_insert(0) += units;
        Ok(())
    }

    /// Transfiere comprobando restricciones y congelamiento.
    pub fn transfer(&mut self, issuance: &Issuance, from: &Holder, to: &Holder, units: i128) -> Result<(), Finding> {
        if from.frozen {
            return Err(Finding::TransferRestricted {
                holder: from.id.clone(),
                reason: "las unidades están congeladas",
            });
        }
        if issuance.restricted_to_qualified && !to.qualified {
            return Err(Finding::TransferRestricted {
                holder: to.id.clone(),
                reason: "el instrumento solo admite inversionistas calificados",
            });
        }
        if self.balance(&from.id) < units {
            return Err(Finding::TransferRestricted { holder: from.id.clone(), reason: "no tiene tantas unidades" });
        }
        *self.balances.entry(from.id.clone()).or_insert(0) -= units;
        *self.balances.entry(to.id.clone()).or_insert(0) += units;
        Ok(())
    }
}

/// Compara el registro digital con el legal simulado.
pub fn reconcile_with_legal(
    instrument: &str,
    digital: &BTreeMap<String, i128>,
    legal: &BTreeMap<String, i128>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (owner, units) in digital {
        if legal.get(owner).copied().unwrap_or(0) != *units {
            findings.push(Finding::LegalDesync {
                instrument: instrument.to_string(),
                digital_owner: owner.clone(),
                legal_owner: legal
                    .iter()
                    .find(|(_, value)| *value == units)
                    .map(|(name, _)| name.clone())
                    .unwrap_or_else(|| "desconocido".to_string()),
            });
        }
    }
    findings
}

// ── Escenarios ───────────────────────────────────────────────────────────────

fn issuance(restricted: bool) -> Issuance {
    Issuance {
        instrument: "INMUEBLE-SIM-1".into(),
        backing: 500_000_000,
        unit_value: 100_000,
        restricted_to_qualified: restricted,
    }
}

fn holder(id: &str, qualified: bool, frozen: bool) -> Holder {
    Holder { id: id.into(), qualified, frozen }
}

pub fn report() -> CaseReport {
    let mut checks = Vec::new();

    // 1. Emitir hasta el respaldo, exacto.
    let mut registry = Registry::default();
    let ok = registry.issue(&issuance(false), "cli-1", 5_000).is_ok();
    checks.push(Check::new(
        "se emiten 5 000 unidades con respaldo para exactamente 5 000",
        "el respaldo es un techo, y llegar al techo es legítimo",
        "true+5000",
        format!("{ok}+{}", registry.issued()),
    ));

    // 2. Una unidad más.
    let error = registry.issue(&issuance(false), "cli-1", 1).unwrap_err();
    checks.push(Check::new(
        "se intenta emitir una unidad por encima del respaldo",
        "el invariante se comprueba en cada emisión, no al cierre del día",
        "over-issuance",
        error.kind().to_string(),
    ));

    // 3. Transferencia a alguien no calificado.
    let mut registry = Registry::default();
    registry.issue(&issuance(true), "cli-1", 1_000).expect("emisión válida");
    let error = registry
        .transfer(&issuance(true), &holder("cli-1", true, false), &holder("cli-2", false, false), 100)
        .unwrap_err();
    checks.push(Check::new(
        "se transfiere a alguien que no cumple la restricción de tenencia",
        "la restricción viaja con el instrumento: el token no la olvida al cambiar de manos",
        "transfer-restricted",
        error.kind().to_string(),
    ));

    // 4. Unidades congeladas.
    let error = registry
        .transfer(&issuance(false), &holder("cli-1", true, true), &holder("cli-2", true, false), 10)
        .unwrap_err();
    checks.push(Check::new(
        "se transfieren unidades congeladas",
        "congelar tiene que impedir la transferencia, no solo marcarla",
        "transfer-restricted",
        error.kind().to_string(),
    ));

    // 5. Desincronización con el registro legal.
    let digital = BTreeMap::from([("cli-3".to_string(), 500_i128)]);
    let legal = BTreeMap::from([("cli-7".to_string(), 500_i128)]);
    let findings = reconcile_with_legal("INMUEBLE-SIM-1", &digital, &legal);
    checks.push(Check::new(
        "el registro digital dice cli-3 y el legal dice cli-7",
        "el token es una anotación: si el registro legal discrepa, gana el legal",
        "legal-desync",
        findings.first().map(|finding| finding.kind().to_string()).unwrap_or_default(),
    ));

    // 6. Las transferencias conservan el total emitido.
    let mut registry = Registry::default();
    registry.issue(&issuance(false), "cli-1", 1_000).expect("emisión válida");
    registry
        .transfer(&issuance(false), &holder("cli-1", true, false), &holder("cli-2", true, false), 400)
        .expect("transferencia válida");
    checks.push(Check::new(
        "tras varias transferencias, sumadas todas las tenencias",
        "transferir mueve unidades, no las crea: el total emitido no cambia",
        "1000+1000",
        format!("{}+{}", registry.issued(), registry.balance("cli-1") + registry.balance("cli-2")),
    ));

    CaseReport::new("CM-08", "Tokenización de instrumentos", Maturity::Prototype, checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_escenarios_hacen_lo_que_declaran() {
        assert!(report().passed());
    }

    #[test]
    fn un_valor_unitario_cero_no_divide_por_cero() {
        let broken = Issuance { instrument: "X".into(), backing: 100, unit_value: 0, restricted_to_qualified: false };
        assert_eq!(broken.max_units(), 0);
    }

    #[test]
    fn no_se_transfiere_mas_de_lo_que_se_tiene() {
        let mut registry = Registry::default();
        registry.issue(&issuance(false), "cli-1", 10).expect("emisión válida");
        assert!(registry
            .transfer(&issuance(false), &holder("cli-1", true, false), &holder("cli-2", true, false), 11)
            .is_err());
    }
}
