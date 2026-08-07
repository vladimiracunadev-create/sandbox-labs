use crate::{
    process::{run, CommandSpec},
    RuntimeAdapter,
};
use anyhow::{bail, Result};
use sandbox_core::{ExecutionOutcome, ExecutionPlan, Policy, RuntimeKind, Workload};
use std::{collections::BTreeMap, env};

pub struct NativeAdapter;
impl RuntimeAdapter for NativeAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Native
    }
    fn execute(
        &self,
        _plan: &ExecutionPlan,
        policy: &Policy,
        workload: &Workload,
        extra_args: &[String],
    ) -> Result<ExecutionOutcome> {
        if env::var("SANDBOX_LABS_ALLOW_NATIVE").ok().as_deref() != Some("1") || !workload.allow_native {
            bail!("Ejecución nativa no autorizada");
        }
        let mut limits = BTreeMap::new();
        limits.insert("timeout".into(), format!("{}s", policy.resources.timeout_seconds));
        limits.insert("output".into(), format!("{} bytes", policy.resources.output_bytes));
        run(
            CommandSpec {
                program: workload.command.clone(),
                args: workload.command_args(extra_args)?,
                current_dir: Some(workload.directory.clone()),
                clear_env: true,
                environment: policy.process.environment.clone(),
                effective_limits: limits,
                observe_cgroup: false,
                seccomp: None,
            },
            policy,
        )
    }
}
