mod adapters;
mod process;

use anyhow::{bail, Result};
use sandbox_core::{ExecutionOutcome, ExecutionPlan, Policy, RuntimeKind, Workload};

pub trait RuntimeAdapter {
    fn kind(&self) -> RuntimeKind;
    fn execute(
        &self,
        plan: &ExecutionPlan,
        policy: &Policy,
        workload: &Workload,
        extra_args: &[String],
    ) -> Result<ExecutionOutcome>;
}

pub fn execute(
    plan: &ExecutionPlan,
    policy: &Policy,
    workload: &Workload,
    extra_args: &[String],
) -> Result<ExecutionOutcome> {
    if !plan.executable {
        bail!(plan.block_reason.clone().unwrap_or_else(|| "El plan no es ejecutable".into()));
    }
    match plan.runtime {
        RuntimeKind::Native => adapters::native::NativeAdapter.execute(plan, policy, workload, extra_args),
        RuntimeKind::Bwrap => adapters::bwrap::BwrapAdapter.execute(plan, policy, workload, extra_args),
        RuntimeKind::Unshare => adapters::unshare::UnshareAdapter.execute(plan, policy, workload, extra_args),
        RuntimeKind::Wasi => adapters::wasi::WasiAdapter.execute(plan, policy, workload, extra_args),
        RuntimeKind::DryRun | RuntimeKind::Gvisor | RuntimeKind::Kata | RuntimeKind::Firecracker => {
            bail!("Runtime sin ejecución automática en esta versión")
        }
    }
}
