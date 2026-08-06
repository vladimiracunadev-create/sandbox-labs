use crate::{
    process::{run, CommandSpec},
    RuntimeAdapter,
};
use anyhow::Result;
use sandbox_core::{command_exists, ExecutionOutcome, ExecutionPlan, Policy, RuntimeKind, Workload};
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
        // `--mount-proc` no es cosmético: sin remontar /proc dentro del nuevo
        // namespace, el proceso sigue viendo el /proc del host y enumera todos
        // sus PIDs. El namespace de PID existe pero no se nota — que es la
        // clase de falso aislamiento que la suite de contención existe para
        // detectar.
        let mut args =
            vec!["--user", "--map-root-user", "--mount", "--pid", "--fork", "--mount-proc", "--uts", "--ipc"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>();
        if policy.network.isolates_host_network() {
            args.push("--net".into());
        }
        args.push("--".into());
        args.push(workload.command.clone());
        args.extend(workload.command_args(extra_args)?);

        let mut limits = BTreeMap::new();
        limits.insert("namespaces".into(), "user,mount,pid,uts,ipc".into());
        limits.insert("timeout".into(), format!("{}s", policy.resources.timeout_seconds));
        if policy.network.isolates_host_network() {
            limits.insert("network".into(), "isolated network namespace (--net)".into());
        }
        limits.insert("output".into(), format!("{} bytes", policy.resources.output_bytes));

        // Los rlimits los hereda el árbol de procesos, así que envolver
        // `unshare` con `prlimit` acota también a la carga.
        //
        // Deliberadamente NO se usa `--nproc`: RLIMIT_NPROC cuenta los procesos
        // del UID real en TODO el host, no los de esta carga. Fijarlo al
        // presupuesto de la política mata la ejecución nada más empezar (el
        // usuario ya tiene procesos abiertos) y, peor, haría pasar por control
        // de contención algo que no lo es. Un techo real de PIDs necesita el
        // controlador `pids` de cgroups v2 — está en el backlog y hasta
        // entonces el control `processes` no se declara.
        let mut program = "unshare".to_string();
        if command_exists("prlimit") {
            let mut wrapped = vec![
                format!("--as={}", policy.resources.memory_mb * 1024 * 1024),
                format!("--nofile={}", policy.resources.open_files),
                "--".into(),
                "unshare".into(),
            ];
            wrapped.extend(args);
            args = wrapped;
            program = "prlimit".into();
            limits.insert("memory".into(), format!("{}MB RLIMIT_AS", policy.resources.memory_mb));
            limits.insert("openFiles".into(), format!("{} RLIMIT_NOFILE", policy.resources.open_files));
        }

        run(
            CommandSpec {
                program,
                args,
                current_dir: Some(workload.directory.clone()),
                clear_env: true,
                environment: policy.process.environment.clone(),
                effective_limits: limits,
                observe_cgroup: false,
            },
            policy,
        )
    }
}
