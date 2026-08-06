use crate::hash::sha256_hex;
use crate::{ExecutionOutcome, ExecutionPlan, Policy, Workload};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceStatus {
    Planned,
    Running,
    Completed,
    Failed,
    Blocked,
    Cancelled,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub control: String,
    pub operation: String,
    pub target: String,
    pub result: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub schema_version: String,
    pub run_id: String,
    pub timestamp: DateTime<Utc>,
    pub status: EvidenceStatus,
    pub runtime: Value,
    pub host: Value,
    pub integrity: Value,
    pub policy: Value,
    pub workload: Value,
    pub limits: Value,
    pub result: Value,
    pub violations: Vec<Violation>,
    pub unsupported: Vec<String>,
    pub plan: Vec<String>,
}

impl Evidence {
    pub fn planned(
        plan: &ExecutionPlan,
        policy: &Policy,
        policy_hash: &str,
        workload: &Workload,
        workload_hash: &str,
    ) -> Self {
        Self::build(plan, policy, policy_hash, workload, workload_hash, None)
    }

    pub fn executed(
        plan: &ExecutionPlan,
        policy: &Policy,
        policy_hash: &str,
        workload: &Workload,
        workload_hash: &str,
        outcome: &ExecutionOutcome,
    ) -> Self {
        Self::build(plan, policy, policy_hash, workload, workload_hash, Some(outcome))
    }

    fn build(
        plan: &ExecutionPlan,
        policy: &Policy,
        policy_hash: &str,
        workload: &Workload,
        workload_hash: &str,
        outcome: Option<&ExecutionOutcome>,
    ) -> Self {
        let timestamp = Utc::now();
        let seed = format!("{}:{}:{}:{}", timestamp.to_rfc3339(), plan.runtime, workload.id, policy.id);
        let run_id = sha256_hex(seed.as_bytes())[..20].to_string();
        let probe = plan.runtime.probe();
        let runner_sha256 = env::current_exe()
            .ok()
            .and_then(|path| fs::read(path).ok())
            .map(sha256_hex)
            .unwrap_or_else(|| "unavailable".into());
        let kernel = Command::new("uname")
            .arg("-r")
            .output()
            .ok()
            .filter(|value| value.status.success())
            .map(|value| String::from_utf8_lossy(&value.stdout).trim().to_string())
            .unwrap_or_else(|| "unknown".into());
        let (status, result, effective_limits, observed) = match outcome {
            None => (
                if plan.runtime.to_string() == "dry-run" { EvidenceStatus::Planned } else { EvidenceStatus::Blocked },
                json!({"exitCode": null, "reason": plan.block_reason, "durationMs": 0, "stdout": "", "stderr": "", "stdoutTruncated": false, "stderrTruncated": false}),
                BTreeMap::<String, String>::new(),
                BTreeMap::<String, String>::new(),
            ),
            Some(value) => {
                let status = match value.status.as_str() {
                    "completed" => EvidenceStatus::Completed,
                    "timeout" => EvidenceStatus::Timeout,
                    "blocked" => EvidenceStatus::Blocked,
                    _ => EvidenceStatus::Failed,
                };
                (
                    status,
                    json!({"exitCode": value.exit_code, "reason": value.reason, "durationMs": value.duration_ms, "stdout": value.stdout, "stderr": value.stderr, "stdoutTruncated": value.stdout_truncated, "stderrTruncated": value.stderr_truncated}),
                    value.effective_limits.clone(),
                    value.observed.clone(),
                )
            }
        };
        Self {
            schema_version: "1.0".into(),
            run_id,
            timestamp,
            status,
            runtime: json!({"id": plan.runtime.to_string(), "version": probe.version, "available": probe.available}),
            host: json!({"os": env::consts::OS, "architecture": env::consts::ARCH, "family": env::consts::FAMILY, "kernel": kernel}),
            integrity: json!({"policySha256": policy_hash, "workloadSha256": workload_hash, "runnerSha256": runner_sha256, "runnerVersion": env!("CARGO_PKG_VERSION")}),
            policy: json!({"id": policy.id, "enforcement": policy.enforcement.mode, "requestedControls": plan.controls.requested, "effectiveControls": plan.controls.effective, "unsupportedControls": plan.controls.unsupported}),
            workload: json!({"id": workload.id, "path": workload.portable_path(), "risk": workload.risk, "expected": workload.expected.outcome}),
            // `observed` es lo que la carga consumió de verdad, no lo que se le
            // permitía. Va vacío cuando no hubo cgroup propio del que leerlo:
            // publicar ahí las cifras de la sesión del host sería peor que no
            // medir nada.
            limits: json!({"requested": policy.resources, "effective": effective_limits, "observed": observed}),
            result,
            violations: vec![],
            unsupported: plan.controls.unsupported.clone(),
            plan: plan.steps.clone(),
        }
    }

    /// Huella del propio contenido de la evidencia.
    ///
    /// Se calcula sobre el JSON con `integrity.evidenceSha256` puesto a cadena
    /// vacía, para que el campo pueda vivir dentro del objeto que resume sin
    /// morderse la cola. `serde_json` ordena las claves de sus mapas, así que la
    /// serialización es estable entre ejecuciones y máquinas.
    ///
    /// No es una firma: quien pueda editar el fichero puede recalcularla. Lo que
    /// detecta es la **alteración accidental o descuidada** —un campo tocado a
    /// mano, una copia truncada, un informe recortado antes de adjuntarlo— que
    /// es el caso que se da en la práctica. La firma Ed25519 sigue en el
    /// backlog.
    pub fn digest(&self) -> Result<String> {
        let mut copy = self.clone();
        copy.integrity["evidenceSha256"] = Value::String(String::new());
        Ok(sha256_hex(serde_json::to_vec(&copy)?))
    }

    /// Sella la evidencia con su propia huella. Idempotente.
    pub fn seal(&mut self) -> Result<()> {
        self.integrity["evidenceSha256"] = Value::String(String::new());
        let digest = self.digest()?;
        self.integrity["evidenceSha256"] = Value::String(digest);
        Ok(())
    }

    pub fn write(&self, directory: impl AsRef<Path>) -> Result<PathBuf> {
        let directory = directory.as_ref();
        fs::create_dir_all(directory).with_context(|| format!("No se pudo crear {}", directory.display()))?;
        let path = directory.join(format!("{}.json", self.run_id));
        let mut sealed = self.clone();
        sealed.seal()?;
        fs::write(&path, serde_json::to_string_pretty(&sealed)?)
            .with_context(|| format!("No se pudo escribir {}", path.display()))?;
        Ok(path)
    }
}

/// Resultado de comprobar una evidencia contra sí misma y contra el repositorio.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationReport {
    pub path: String,
    pub run_id: String,
    /// Cada comprobación con su veredicto. `None` = no se pudo comprobar, que
    /// no es lo mismo que fallar.
    pub checks: Vec<VerificationCheck>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationCheck {
    pub name: String,
    pub passed: Option<bool>,
    pub detail: String,
}

impl VerificationReport {
    /// Falla si alguna comprobación dio negativo. Las que no pudieron hacerse
    /// no cuentan como aprobado ni como fallo: se informan.
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|check| check.passed != Some(false))
    }

    pub fn unverifiable(&self) -> usize {
        self.checks.iter().filter(|check| check.passed.is_none()).count()
    }
}

/// Comprueba una evidencia: su propia huella y los hashes que declara.
///
/// `root` es la raíz del repositorio, necesaria para volver a hashear la carga
/// que la evidencia dice haber ejecutado. Una evidencia cuyo `workloadSha256` ya
/// no coincide no está corrupta: dice que **la carga cambió desde entonces**, y
/// eso es exactamente lo que un informe de hace tres semanas tiene que poder
/// contar.
pub fn verify(path: impl AsRef<Path>, root: impl AsRef<Path>) -> Result<VerificationReport> {
    let path = path.as_ref();
    let raw = fs::read_to_string(path).with_context(|| format!("No se pudo leer {}", path.display()))?;
    let evidence: Evidence =
        serde_json::from_str(&raw).with_context(|| format!("No es una evidencia válida: {}", path.display()))?;
    let mut checks = Vec::new();

    let recorded = evidence.integrity.get("evidenceSha256").and_then(Value::as_str).unwrap_or_default().to_string();
    if recorded.is_empty() {
        checks.push(VerificationCheck {
            name: "huella propia".into(),
            passed: None,
            detail: "la evidencia es anterior al sellado y no trae evidenceSha256".into(),
        });
    } else {
        let computed = evidence.digest()?;
        checks.push(VerificationCheck {
            name: "huella propia".into(),
            passed: Some(computed == recorded),
            detail: if computed == recorded {
                format!("coincide ({})", &recorded[..16.min(recorded.len())])
            } else {
                format!("declara {recorded} y su contenido da {computed}")
            },
        });
    }

    let root = root.as_ref();
    checks.push(rehash_policy(&evidence, root));
    checks.push(rehash_workload(&evidence, root));

    Ok(VerificationReport { path: path.display().to_string(), run_id: evidence.run_id.clone(), checks })
}

fn rehash_policy(evidence: &Evidence, root: &Path) -> VerificationCheck {
    let name = "política sin cambios".to_string();
    let recorded = evidence.integrity.get("policySha256").and_then(Value::as_str).unwrap_or_default();
    let Some(id) = evidence.policy.get("id").and_then(Value::as_str) else {
        return VerificationCheck { name, passed: None, detail: "la evidencia no nombra la política".into() };
    };
    let file = root.join("policies").join(format!("{id}.json"));
    match Policy::hash(&file) {
        Err(_) => VerificationCheck { name, passed: None, detail: format!("{id}.json ya no está en el repositorio") },
        Ok(current) => VerificationCheck {
            name,
            passed: Some(current == recorded),
            detail: if current == recorded {
                format!("{id} coincide")
            } else {
                format!("{id} cambió desde esta ejecución: la evidencia describe otra política")
            },
        },
    }
}

fn rehash_workload(evidence: &Evidence, root: &Path) -> VerificationCheck {
    let name = "carga sin cambios".to_string();
    let recorded = evidence.integrity.get("workloadSha256").and_then(Value::as_str).unwrap_or_default();
    let Some(relative) = evidence.workload.get("path").and_then(Value::as_str) else {
        return VerificationCheck { name, passed: None, detail: "la evidencia no nombra la carga".into() };
    };
    match Workload::load(root.join(relative)).and_then(|value| value.hash()) {
        Err(_) => VerificationCheck { name, passed: None, detail: format!("{relative} ya no está en el repositorio") },
        Ok(current) => VerificationCheck {
            name,
            passed: Some(current == recorded),
            detail: if current == recorded {
                format!("{relative} coincide")
            } else {
                format!("{relative} cambió desde esta ejecución: la evidencia describe otro código")
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence() -> Evidence {
        Evidence {
            schema_version: "1.0".into(),
            run_id: "prueba".into(),
            timestamp: Utc::now(),
            status: EvidenceStatus::Completed,
            runtime: json!({"id": "bwrap"}),
            host: json!({"os": "linux"}),
            integrity: json!({"policySha256": "aa", "workloadSha256": "bb"}),
            policy: json!({"id": "minimal", "effectiveControls": ["filesystem"]}),
            workload: json!({"id": "hello", "path": "workloads/benign/hello"}),
            limits: json!({"requested": {}, "effective": {}, "observed": {}}),
            result: json!({"exitCode": 0}),
            violations: vec![],
            unsupported: vec![],
            plan: vec![],
        }
    }

    #[test]
    fn sealing_is_idempotent() {
        let mut once = evidence();
        once.seal().expect("sellar");
        let first = once.integrity["evidenceSha256"].clone();
        once.seal().expect("volver a sellar");
        assert_eq!(first, once.integrity["evidenceSha256"], "sellar dos veces no puede cambiar la huella");
    }

    #[test]
    fn the_digest_ignores_its_own_field() {
        // Si el campo entrara en su propio cálculo, sellar cambiaría la huella
        // y nada volvería a verificar.
        let mut sealed = evidence();
        sealed.seal().expect("sellar");
        let recorded = sealed.integrity["evidenceSha256"].as_str().expect("huella").to_string();
        assert_eq!(sealed.digest().expect("recalcular"), recorded);
    }

    #[test]
    fn any_edited_field_breaks_the_digest() {
        // El caso peligroso de verdad: añadir a mano un control efectivo que
        // nunca se aplicó, que es justo lo que este proyecto existe para que no
        // pase inadvertido.
        let mut sealed = evidence();
        sealed.seal().expect("sellar");
        let recorded = sealed.integrity["evidenceSha256"].as_str().expect("huella").to_string();

        let mut forged = sealed.clone();
        forged.policy["effectiveControls"] = json!(["filesystem", "network", "memory"]);
        assert_ne!(forged.digest().expect("recalcular"), recorded, "un control inventado tiene que romper la huella");

        let mut retitled = sealed.clone();
        retitled.status = EvidenceStatus::Blocked;
        assert_ne!(retitled.digest().expect("recalcular"), recorded, "cambiar el estado tiene que romper la huella");
    }

    #[test]
    fn the_digest_is_stable_across_serializations() {
        // Sin estabilidad, verificar sería una lotería: dos ejecuciones sobre
        // el mismo contenido tienen que dar la misma huella.
        let value = evidence();
        assert_eq!(value.digest().expect("una"), value.digest().expect("otra"));
    }
}
