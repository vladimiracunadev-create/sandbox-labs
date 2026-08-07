use anyhow::{Context, Result};
use sandbox_core::{ExecutionOutcome, Policy};
use std::{
    collections::BTreeMap,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use wait_timeout::ChildExt;

/// Sin `Clone`: lleva el fichero abierto del filtro seccomp, que es un recurso
/// único y no se puede duplicar sin duplicar también el descriptor.
#[derive(Debug)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub current_dir: Option<PathBuf>,
    pub clear_env: bool,
    pub environment: BTreeMap<String, String>,
    pub effective_limits: BTreeMap<String, String>,
    /// ¿Envolvimos la ejecución en un cgroup propio?
    ///
    /// Decide si se muestrea el consumo. Sin envoltorio, `/proc/<pid>/cgroup`
    /// apunta al cgroup de la sesión del host, y publicar sus cifras como
    /// consumo de la carga sería peor que no medir nada.
    pub observe_cgroup: bool,
    /// Filtro seccomp ya compilado y escrito a un fichero temporal.
    ///
    /// Se pasa abierto y no por ruta porque bubblewrap lo lee de un
    /// **descriptor**, no de un camino: `--seccomp <fd>`. El fichero se mantiene
    /// vivo hasta después de `spawn` para que el descriptor siga siendo válido
    /// cuando el hijo lo duplique.
    pub seccomp: Option<std::fs::File>,
}

pub fn run(spec: CommandSpec, policy: &Policy) -> Result<ExecutionOutcome> {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(directory) = &spec.current_dir {
        command.current_dir(directory);
    }
    if spec.clear_env {
        command.env_clear();
    }
    command.envs(&spec.environment);
    // El filtro tiene que llegar al hijo por un descriptor concreto. El cómo
    // vive junto al resto de seccomp, no aquí.
    #[cfg(unix)]
    if let Some(filter) = spec.seccomp.as_ref() {
        sandbox_core::seccomp::inherit(&mut command, filter);
    }
    let started = Instant::now();
    let mut child = command.spawn().with_context(|| format!("No se pudo iniciar {}", spec.program))?;
    // Antes de nada: systemd retira el cgroup en cuanto el scope termina, así
    // que el consumo hay que leerlo MIENTRAS la carga corre.
    let sampler = spec.observe_cgroup.then(|| sandbox_core::cgroup::Sampler::start(child.id())).flatten();
    let stdout = child.stdout.take().context("stdout no disponible")?;
    let stderr = child.stderr.take().context("stderr no disponible")?;
    let cap = policy.resources.output_bytes as usize;
    let out_thread = thread::spawn(move || drain(stdout, cap));
    let err_thread = thread::spawn(move || drain(stderr, cap));
    let timeout = Duration::from_secs(policy.resources.timeout_seconds);
    let (status, reason) = match child.wait_timeout(timeout)? {
        Some(status) => (status, "process_exited".to_string()),
        None => {
            let _ = child.kill();
            (child.wait()?, "timeout".to_string())
        }
    };
    let observed = sampler.map(|value| value.finish().to_map()).unwrap_or_default();
    let (stdout, stdout_truncated) = out_thread.join().unwrap_or_else(|_| (String::new(), false));
    let (stderr, stderr_truncated) = err_thread.join().unwrap_or_else(|_| (String::new(), false));
    let timed_out = reason == "timeout";
    Ok(ExecutionOutcome {
        status: if timed_out {
            "timeout".into()
        } else if status.success() {
            "completed".into()
        } else {
            "failed".into()
        },
        exit_code: status.code(),
        reason,
        duration_ms: started.elapsed().as_millis(),
        stdout,
        stderr,
        stdout_truncated,
        stderr_truncated,
        effective_limits: spec.effective_limits,
        observed,
        network_events: Vec::new(),
    })
}

fn drain<R: Read>(mut reader: R, cap: usize) -> (String, bool) {
    let mut kept = Vec::with_capacity(cap.min(8192));
    let mut buffer = [0_u8; 8192];
    let mut truncated = false;
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(count) => {
                let remaining = cap.saturating_sub(kept.len());
                let take = remaining.min(count);
                kept.extend_from_slice(&buffer[..take]);
                if take < count {
                    truncated = true;
                }
            }
        }
    }
    (String::from_utf8_lossy(&kept).to_string(), truncated)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn policy() -> Policy {
        serde_json::from_value(serde_json::json!({
            "id": "test",
            "enforcement": { "mode": "best-effort", "requiredControls": [] },
            "filesystem": { "root": "ephemeral", "readOnly": [], "writable": [], "maxWorkspaceMb": 64, "followSymlinks": false },
            "network": { "mode": "none", "hosts": [], "dns": "disabled" },
            "resources": { "cpu": 1.0, "memoryMb": 128, "processes": 8, "timeoutSeconds": 20, "openFiles": 32, "outputBytes": 4096 },
            "process": { "capabilities": [], "environment": {}, "allowedEnvironment": [], "user": 65534, "group": 65534 }
        }))
        .expect("política de prueba válida")
    }

    /// El eslabón que no se puede comprobar mirando la línea de comandos.
    ///
    /// El filtro seccomp viaja a bubblewrap por un descriptor concreto, y ese
    /// descriptor lo crea `dup2` dentro del hijo. Si fallara, bubblewrap leería
    /// basura o nada y **todas** las sondas se caerían a la vez — que es un
    /// fallo ruidoso, pero llegaría en CI y no aquí.
    ///
    /// Se usa `sh` leyendo del descriptor en vez de bubblewrap: lo que se mide
    /// es el paso del descriptor, no lo que el runtime haga con él.
    #[test]
    fn the_filter_descriptor_reaches_the_child() {
        let directory = tempfile::tempdir().expect("directorio temporal");
        let path = directory.path().join("filtro.bpf");
        std::fs::write(&path, b"12345678").expect("escribir filtro");

        let outcome = run(
            CommandSpec {
                program: "/bin/sh".into(),
                args: vec![
                    "-c".into(),
                    // Por `/proc/self/fd` y no con `<&63`: `/bin/sh` es dash en
                    // Debian y Ubuntu, y no admite descriptores de más de un
                    // dígito. El descriptor sí está; lo que no está es la
                    // sintaxis para nombrarlo.
                    format!("/usr/bin/wc -c < /proc/self/fd/{}", sandbox_core::seccomp::FILTER_FD),
                ],
                current_dir: None,
                clear_env: true,
                environment: BTreeMap::new(),
                effective_limits: BTreeMap::new(),
                observe_cgroup: false,
                seccomp: Some(std::fs::File::open(&path).expect("abrir filtro")),
            },
            &policy(),
        )
        .expect("ejecutar");

        assert_eq!(
            outcome.stdout.trim(),
            "8",
            "salida={:?} error={:?} estado={} código={:?}",
            outcome.stdout,
            outcome.stderr,
            outcome.status,
            outcome.exit_code
        );
    }

    /// Y sin filtro, ese descriptor no puede existir: heredarlo por accidente
    /// sería una fuga de descriptores hacia dentro del sandbox.
    #[test]
    fn without_a_filter_the_descriptor_is_not_there() {
        let outcome = run(
            CommandSpec {
                program: "/bin/sh".into(),
                args: vec![
                    "-c".into(),
                    format!(
                        "test -e /proc/self/fd/{} && echo presente || echo ausente",
                        sandbox_core::seccomp::FILTER_FD
                    ),
                ],
                current_dir: None,
                clear_env: true,
                environment: BTreeMap::new(),
                effective_limits: BTreeMap::new(),
                observe_cgroup: false,
                seccomp: None,
            },
            &policy(),
        )
        .expect("ejecutar");

        assert_eq!(
            outcome.stdout.trim(),
            "ausente",
            "salida={:?} error={:?} estado={}",
            outcome.stdout,
            outcome.stderr,
            outcome.status
        );
    }
}
