use crate::{
    process::{run, CommandSpec},
    RuntimeAdapter,
};
use anyhow::{bail, Result};
use sandbox_core::{ExecutionOutcome, ExecutionPlan, Policy, RuntimeKind, Workload};
use std::collections::BTreeMap;

pub struct WasiAdapter;
impl RuntimeAdapter for WasiAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Wasi
    }
    fn execute(
        &self,
        _plan: &ExecutionPlan,
        policy: &Policy,
        workload: &Workload,
        extra_args: &[String],
    ) -> Result<ExecutionOutcome> {
        if workload.kind != "wasi" {
            bail!("El runtime WASI exige workload kind=wasi");
        }
        let entrypoint = workload.entrypoint_path()?;
        let mut args =
            vec!["run".into(), "--dir".into(), format!("{}::/workspace/input", workload.directory.display())];
        for (name, value) in &policy.process.environment {
            args.extend(["--env".into(), format!("{name}={value}")]);
        }
        args.push(entrypoint.display().to_string());
        args.extend_from_slice(extra_args);
        let mut limits = BTreeMap::new();
        limits.insert("filesystem".into(), "WASI preopened directory".into());
        limits.insert("network".into(), "not inherited".into());
        limits.insert("timeout".into(), format!("{}s", policy.resources.timeout_seconds));
        run(
            CommandSpec {
                program: "wasmtime".into(),
                args,
                current_dir: None,
                clear_env: true,
                environment: BTreeMap::new(),
                effective_limits: limits,
            },
            policy,
        )
    }
}
