use crate::{
    process::{run, CommandSpec},
    RuntimeAdapter,
};
use anyhow::Result;
use sandbox_core::{command_exists, ExecutionOutcome, ExecutionPlan, Policy, RuntimeKind, Workload};
use std::collections::BTreeMap;
use tempfile::tempdir;

/// Lo que este runtime aplicó **de verdad** en esta ejecución.
///
/// Va tal cual a `limits.effective` de la evidencia, así que solo puede
/// contener controles cuyo argumento se añadió a la línea de comandos. La
/// entrada `network` es el ejemplo que motiva esta función: antes se escribía
/// siempre «isolated network namespace», también cuando la política pedía
/// `loopback` o `allowlist` y por tanto **no** se añadió `--unshare-net`. La
/// evidencia declaraba un aislamiento de red que no existía.
fn effective_limits(
    policy: &Policy,
    network_isolated: bool,
    wrapped_in_prlimit: bool,
    wrapped_in_cgroup: bool,
    filtered: bool,
) -> BTreeMap<String, String> {
    let mut limits = BTreeMap::new();
    limits.insert("filesystem".into(), "bubblewrap mount namespace".into());
    limits.insert("user".into(), format!("uid={} gid={} (--uid/--gid)", policy.process.user, policy.process.group));
    if network_isolated {
        limits.insert("network".into(), "isolated network namespace (--unshare-net)".into());
    }
    limits.insert("timeout".into(), format!("{}s", policy.resources.timeout_seconds));
    limits.insert("output".into(), format!("{} bytes", policy.resources.output_bytes));
    if wrapped_in_prlimit {
        limits.insert("memory".into(), format!("{}MB RLIMIT_AS", policy.resources.memory_mb));
        limits.insert("openFiles".into(), format!("{} RLIMIT_NOFILE", policy.resources.open_files));
    }
    if wrapped_in_cgroup {
        // Pisa deliberadamente la entrada de `prlimit`: cuando los dos están,
        // el que manda es el cgroup, y la evidencia debe nombrar el mecanismo
        // que de verdad acota la memoria residente.
        limits.insert("memory".into(), format!("{}MB cgroup memory.max", policy.resources.memory_mb));
        limits.insert("processes".into(), format!("{} cgroup pids.max", policy.resources.processes));
        limits.insert("cpu".into(), format!("{} núcleos cgroup cpu.max", policy.resources.cpu));
    }
    if filtered {
        limits.insert(
            "syscalls".into(),
            format!("{} llamadas denegadas con EPERM (seccomp BPF)", policy.syscalls.deny.len()),
        );
    }
    limits
}

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
        // `none` y `loopback` crean namespace de red propio; `allowlist` y
        // `unrestricted` conservan la del host. Un servicio que necesite
        // publicar un puerto tiene que pedir uno de los dos últimos y decirlo
        // en su política, no colarse por un modo que suena contenido.
        let network_isolated = policy.network.isolates_host_network();

        // El filtro seccomp se escribe antes de compilar los argumentos, porque
        // el descriptor va dentro de ellos.
        let seccomp = match sandbox_core::seccomp::compile(policy)? {
            None => None,
            Some(filter) => {
                let path = temp.path().join("seccomp.bpf");
                std::fs::write(&path, sandbox_core::seccomp::to_bytes(&filter))?;
                Some(std::fs::File::open(&path)?)
            }
        };
        let filtered = seccomp.is_some();

        // Un único compilador para cargas y servicios: ver `sandbox_core::compiler`.
        let mut args = sandbox_core::bubblewrap(
            policy,
            &sandbox_core::SandboxRequest {
                mounts: vec![
                    sandbox_core::Mount::read_only(workload.directory.display().to_string(), "/workspace/input"),
                    sandbox_core::Mount::writable(output.display().to_string(), "/workspace/output"),
                ],
                workdir: "/workspace/input".into(),
                environment: BTreeMap::new(),
                command: workload.command.clone(),
                args: workload.command_args(extra_args)?,
                // El supervisor espera a esta carga: si él cae, la jaula no
                // puede quedarse viva con algo dentro.
                die_with_parent: true,
            },
            filtered.then_some(sandbox_core::seccomp::FILTER_FD),
        );
        let mut program = "bwrap".to_string();
        // Sin `--nproc`: RLIMIT_NPROC cuenta los procesos del UID real en todo
        // el host, no los de esta carga. Aplicarlo aquí mata la ejecución nada
        // más empezar y haría pasar por control de contención algo que no lo
        // es. El techo real de PIDs lo pone `pids.max` del cgroup, unas líneas
        // más abajo, y solo cuando el host lo admite.
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
        let wrapped_in_prlimit = program == "prlimit";
        // El scope de systemd va por FUERA de todo lo demás: el cgroup tiene
        // que contener al árbol entero, incluidos `prlimit` y el propio
        // `bwrap`, no solo al proceso final.
        let mut environment = BTreeMap::new();
        let wrapped_in_cgroup = match sandbox_core::cgroup::wrap(&program, &args, &policy.resources) {
            None => false,
            Some((outer, outer_args)) => {
                program = outer;
                args = outer_args;
                // `systemd-run` necesita encontrar el bus del gestor de
                // usuario. Solo las lee él: la cadena que devuelve `wrap`
                // intercala un `env -i` antes del runtime, así que bubblewrap
                // arranca con el entorno vacío igual que sin envolver.
                //
                // No basta con confiar en el `--clearenv` de bubblewrap: eso
                // limpia el entorno de la CARGA, no el de su propio `init`, que
                // es el PID 1 dentro del sandbox y cuyo `/proc/1/environ` la
                // carga puede leer.
                for name in sandbox_core::cgroup::REQUIRED_ENVIRONMENT {
                    if let Ok(value) = std::env::var(name) {
                        environment.insert(name.to_string(), value);
                    }
                }
                true
            }
        };
        let limits = effective_limits(policy, network_isolated, wrapped_in_prlimit, wrapped_in_cgroup, filtered);
        run(
            CommandSpec {
                program,
                args,
                current_dir: None,
                clear_env: true,
                environment,
                seccomp,
                effective_limits: limits,
                observe_cgroup: wrapped_in_cgroup,
            },
            policy,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy_with_network(mode: &str) -> Policy {
        let json = serde_json::json!({
            "id": "test",
            "enforcement": { "mode": "best-effort", "requiredControls": [] },
            "filesystem": { "root": "ephemeral", "readOnly": [], "writable": [], "maxWorkspaceMb": 64, "followSymlinks": false },
            "network": { "mode": mode, "hosts": [], "dns": "disabled" },
            "resources": { "cpu": 1.0, "memoryMb": 128, "processes": 8, "timeoutSeconds": 10, "openFiles": 32, "outputBytes": 4096 },
            "process": { "capabilities": [], "environment": {}, "allowedEnvironment": [], "user": 65534, "group": 65534 }
        });
        serde_json::from_value(json).expect("política de prueba válida")
    }

    #[test]
    fn declares_network_isolation_when_the_namespace_exists() {
        let contained = effective_limits(&policy_with_network("none"), true, false, false, false);
        assert!(contained.contains_key("network"), "con --unshare-net la red sí queda aislada");
    }

    /// La regresión que motiva `effective_limits`.
    ///
    /// `service-sandbox` —la política de todos los servicios del catálogo— pide
    /// `loopback`, y con ella bubblewrap **no** añade `--unshare-net`: la carga
    /// se queda en la red del host. La evidencia lo declaraba aislado igual.
    #[test]
    fn never_declares_isolation_while_keeping_the_host_network() {
        for mode in ["loopback", "allowlist", "unrestricted"] {
            let limits = effective_limits(&policy_with_network(mode), false, false, false, false);
            assert!(
                !limits.contains_key("network"),
                "con network={mode} no se creó namespace de red: declararlo sería mentir en la evidencia"
            );
        }
    }

    #[test]
    fn declares_memory_only_when_prlimit_wraps_the_execution() {
        let policy = policy_with_network("none");
        assert!(!effective_limits(&policy, true, false, false, false).contains_key("memory"));
        assert!(effective_limits(&policy, true, true, false, false).contains_key("memory"));
    }

    #[test]
    fn always_declares_the_identity_the_policy_asked_for() {
        // No depende del host: `--uid`/`--gid` los aplica bubblewrap siempre que
        // haya user namespace, y siempre lo hay.
        let limits = effective_limits(&policy_with_network("none"), true, false, false, false);
        assert_eq!(limits["user"], "uid=65534 gid=65534 (--uid/--gid)");
    }

    #[test]
    fn declares_pids_and_cpu_only_under_a_cgroup() {
        let policy = policy_with_network("none");
        let sin = effective_limits(&policy, true, true, false, false);
        assert!(!sin.contains_key("processes"), "RLIMIT_NPROC no es un techo de PIDs de la carga");
        assert!(!sin.contains_key("cpu"), "sin cgroup no hay cuota de CPU que declarar");

        let con = effective_limits(&policy, true, true, true, false);
        assert!(con["processes"].contains("pids.max"));
        assert!(con["cpu"].contains("cpu.max"));
    }

    #[test]
    fn the_cgroup_wins_over_prlimit_when_naming_the_memory_ceiling() {
        // Los dos pueden estar puestos a la vez. La evidencia tiene que nombrar
        // el que de verdad acota la memoria residente, no el del espacio de
        // direcciones virtual.
        let limits = effective_limits(&policy_with_network("none"), true, true, true, false);
        assert!(limits["memory"].contains("memory.max"), "{}", limits["memory"]);
        assert!(!limits["memory"].contains("RLIMIT_AS"), "{}", limits["memory"]);
    }
}
