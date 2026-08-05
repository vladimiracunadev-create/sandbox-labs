use crate::{
    process::{run, CommandSpec},
    RuntimeAdapter,
};
use anyhow::Result;
use sandbox_core::{ExecutionOutcome, ExecutionPlan, Policy, RuntimeKind, Workload};
use std::collections::BTreeMap;

pub struct UnshareAdapter;
impl RuntimeAdapter for UnshareAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Unshare
    }
    fn execute(
        &self,
        _plan: &ExecutionPlan,
        policy: &Policy,
        workload: &Workload,
        extra_args: &[String],
    ) -> Result<ExecutionOutcome> {
        let mut args = vec!["--user", "--map-root-user", "--mount", "--pid", "--fork", "--uts", "--ipc"]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>();
        if policy.network.mode == "none" {
            args.push("--net".into());
        }
        args.push("--".into());
        args.push(workload.command.clone());
        args.extend(workload.command_args(extra_args)?);
        let mut limits = BTreeMap::new();
        limits.insert("namespaces".into(), "user,mount,pid,uts,ipc".into());
        limits.insert("timeout".into(), format!("{}s", policy.resources.timeout_seconds));
        run(
            CommandSpec {
                program: "unshare".into(),
                args,
                current_dir: Some(workload.directory.clone()),
                clear_env: true,
                environment: policy.process.environment.clone(),
                effective_limits: limits,
            },
            policy,
        )
    }
}
