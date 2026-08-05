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
        let (status, result, effective_limits) = match outcome {
            None => (
                if plan.runtime.to_string() == "dry-run" { EvidenceStatus::Planned } else { EvidenceStatus::Blocked },
                json!({"exitCode": null, "reason": plan.block_reason, "durationMs": 0, "stdout": "", "stderr": "", "stdoutTruncated": false, "stderrTruncated": false}),
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
            limits: json!({"requested": policy.resources, "effective": effective_limits}),
            result,
            violations: vec![],
            unsupported: plan.controls.unsupported.clone(),
            plan: plan.steps.clone(),
        }
    }

    pub fn write(&self, directory: impl AsRef<Path>) -> Result<PathBuf> {
        let directory = directory.as_ref();
        fs::create_dir_all(directory).with_context(|| format!("No se pudo crear {}", directory.display()))?;
        let path = directory.join(format!("{}.json", self.run_id));
        fs::write(&path, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("No se pudo escribir {}", path.display()))?;
        Ok(path)
    }
}
