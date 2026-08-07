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
    // El filtro tiene que llegar al hijo por un descriptor concreto, y Rust
    // marca CLOEXEC en todo lo que abre. `dup2` en el hijo —después de fork y
    // antes de exec— crea una copia sin CLOEXEC en el número que bubblewrap
    // espera. No hay API segura en std para esto.
    //
    // `dup2` es async-signal-safe, que es el requisito de lo que corre aquí.
    #[cfg(unix)]
    if let Some(filter) = spec.seccomp.as_ref() {
        use std::os::unix::{io::AsRawFd, process::CommandExt};
        let source = filter.as_raw_fd();
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(source, sandbox_core::seccomp::FILTER_FD) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
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
