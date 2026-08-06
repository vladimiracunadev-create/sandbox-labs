use crate::{
    process::{run, CommandSpec},
    RuntimeAdapter,
};
use anyhow::Result;
use sandbox_core::{command_exists, ExecutionOutcome, ExecutionPlan, Policy, RuntimeKind, Workload};
use std::{collections::BTreeMap, path::Path};
use tempfile::tempdir;

pub struct BwrapAdapter;
impl RuntimeAdapter for BwrapAdapter {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Bwrap
    }
    fn execute(
        &self,
        _plan: &ExecutionPlan,
        policy: &Policy,
        workload: &Workload,
        extra_args: &[String],
    ) -> Result<ExecutionOutcome> {
        let temp = tempdir()?;
        let output = temp.path().join("output");
        std::fs::create_dir_all(&output)?;
        // `--unshare-all` incluye la red. Un servicio con `network.mode` distinto
        // de `none` necesita conservar el loopback del host para poder publicar
        // un puerto; si no, el sandbox arranca y nadie puede hablar con él.
        // Antes se cortaba siempre, así que la política se ignoraba en silencio.
        let network_isolated = policy.network.mode == "none";
        let mut args = vec![
            "--die-with-parent",
            "--new-session",
            "--unshare-user",
            "--unshare-ipc",
            "--unshare-pid",
            "--unshare-uts",
            "--unshare-cgroup-try",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--tmpfs",
            "/tmp",
            "--dir",
            "/workspace",
        ]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
        // El flag va antes de `--ro-bind`: intercalarlo entre esa opción y sus
        // dos rutas rompería el parseo de argumentos de bwrap.
        if network_isolated {
            args.push("--unshare-net".into());
        }
        args.push("--ro-bind".into());
        args.push(workload.directory.display().to_string());
        args.push("/workspace/input".into());
        args.push("--bind".into());
        args.push(output.display().to_string());
        args.push("/workspace/output".into());
        for system in ["/usr", "/bin", "/lib", "/lib64"] {
            if Path::new(system).exists() {
                args.extend(["--ro-bind".into(), system.into(), system.into()]);
            }
        }
        for system in ["/etc/passwd", "/etc/group"] {
            if Path::new(system).exists() {
                args.extend(["--ro-bind".into(), system.into(), system.into()]);
            }
        }
        args.extend([
            "--chdir".into(),
            "/workspace/input".into(),
            "--clearenv".into(),
            "--cap-drop".into(),
            "ALL".into(),
        ]);
        for (name, value) in &policy.process.environment {
            args.extend(["--setenv".into(), name.clone(), value.clone()]);
        }
        args.push("--".into());
        args.push(workload.command.clone());
        args.extend(workload.command_args(extra_args)?);
        let mut program = "bwrap".to_string();
        // Sin `--nproc`: RLIMIT_NPROC cuenta los procesos del UID real en todo
        // el host, no los de esta carga. Aplicarlo aquí mata la ejecución nada
        // más empezar y haría pasar por control de contención algo que no lo
        // es. Un techo real de PIDs necesita el controlador `pids` de cgroups
        // v2; hasta entonces, el control `processes` no se declara.
        if command_exists("prlimit") {
            let mut wrapped = vec![
                format!("--as={}", policy.resources.memory_mb * 1024 * 1024),
                format!("--nofile={}", policy.resources.open_files),
                "--".into(),
                "bwrap".into(),
            ];
            wrapped.extend(args);
            args = wrapped;
            program = "prlimit".into();
        }
        let mut limits = BTreeMap::new();
        limits.insert("filesystem".into(), "bubblewrap mount namespace".into());
        limits.insert("network".into(), "isolated network namespace".into());
        limits.insert("timeout".into(), format!("{}s", policy.resources.timeout_seconds));
        limits.insert("output".into(), format!("{} bytes", policy.resources.output_bytes));
        if program == "prlimit" {
            limits.insert("memory".into(), format!("{}MB RLIMIT_AS", policy.resources.memory_mb));
            limits.insert("openFiles".into(), format!("{} RLIMIT_NOFILE", policy.resources.open_files));
        }
        run(
            CommandSpec {
                program,
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
