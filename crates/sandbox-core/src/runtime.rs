use crate::{EnforcementMode, Policy, Workload};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    process::Command,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeKind {
    DryRun,
    Native,
    Bwrap,
    Unshare,
    Gvisor,
    Kata,
    Wasi,
    Firecracker,
}

impl std::str::FromStr for RuntimeKind {
    type Err = anyhow::Error;
    fn from_str(value: &str) -> Result<Self> {
        match value {
            "dry-run" => Ok(Self::DryRun),
            "native" => Ok(Self::Native),
            "bwrap" => Ok(Self::Bwrap),
            "unshare" => Ok(Self::Unshare),
            "gvisor" => Ok(Self::Gvisor),
            "kata" => Ok(Self::Kata),
            "wasi" => Ok(Self::Wasi),
            "firecracker" => Ok(Self::Firecracker),
            other => bail!("Runtime no reconocido: {other}"),
        }
    }
}

impl std::fmt::Display for RuntimeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::DryRun => "dry-run",
                Self::Native => "native",
                Self::Bwrap => "bwrap",
                Self::Unshare => "unshare",
                Self::Gvisor => "gvisor",
                Self::Kata => "kata",
                Self::Wasi => "wasi",
                Self::Firecracker => "firecracker",
            }
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProbe {
    pub id: String,
    pub available: bool,
    pub version: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlAssessment {
    pub requested: Vec<String>,
    pub effective: Vec<String>,
    pub unsupported: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionPlan {
    pub runtime: RuntimeKind,
    pub workload_id: String,
    pub workload_path: String,
    pub policy_id: String,
    pub steps: Vec<String>,
    pub executable: bool,
    pub block_reason: Option<String>,
    pub controls: ControlAssessment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionOutcome {
    pub status: String,
    pub exit_code: Option<i32>,
    pub reason: String,
    pub duration_ms: u128,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub effective_limits: BTreeMap<String, String>,
    /// Lo que la carga consumió de verdad, leído del cgroup mientras corría.
    /// Vacío cuando no hubo cgroup propio que mirar.
    #[serde(default)]
    pub observed: BTreeMap<String, String>,
}

impl RuntimeKind {
    pub fn probe(self) -> RuntimeProbe {
        let (command, args): (&str, &[&str]) = match self {
            Self::DryRun => {
                return RuntimeProbe {
                    id: self.to_string(),
                    available: true,
                    version: env!("CARGO_PKG_VERSION").into(),
                    detail: "Planificación local".into(),
                }
            }
            Self::Native => {
                return RuntimeProbe {
                    id: self.to_string(),
                    available: env::var("SANDBOX_LABS_ALLOW_NATIVE").ok().as_deref() == Some("1"),
                    version: env::consts::OS.into(),
                    detail: "Requiere opt-in y carga allowNative".into(),
                }
            }
            Self::Bwrap => ("bwrap", &["--version"]),
            Self::Unshare => ("unshare", &["--version"]),
            Self::Gvisor => ("runsc", &["--version"]),
            Self::Kata => ("kata-runtime", &["--version"]),
            Self::Wasi => ("wasmtime", &["--version"]),
            Self::Firecracker => ("firecracker", &["--version"]),
        };
        match Command::new(command).args(args).output() {
            Ok(output) => {
                let text = if output.stdout.is_empty() { &output.stderr } else { &output.stdout };
                RuntimeProbe {
                    id: self.to_string(),
                    available: output.status.success(),
                    version: String::from_utf8_lossy(text).lines().next().unwrap_or("desconocida").to_string(),
                    detail: command.into(),
                }
            }
            Err(error) => RuntimeProbe {
                id: self.to_string(),
                available: false,
                version: String::new(),
                detail: error.to_string(),
            },
        }
    }

    pub fn supported_controls(self, policy: &Policy) -> BTreeSet<String> {
        let declared: Vec<&str> = match self {
            Self::DryRun => vec!["planning", "evidence"],
            Self::Native => vec!["timeout", "environment", "output"],
            Self::Bwrap => vec!["filesystem", "network", "timeout", "capabilities", "devices", "environment", "output"],
            Self::Unshare => vec!["network", "timeout", "environment", "output"],
            Self::Wasi => vec!["filesystem", "network", "timeout", "environment", "output"],
            Self::Gvisor => vec![
                "filesystem",
                "network",
                "processes",
                "memory",
                "cpu",
                "timeout",
                "capabilities",
                "syscalls",
                "devices",
                "environment",
                "output",
            ],
            Self::Kata | Self::Firecracker => vec![
                "filesystem",
                "network",
                "processes",
                "memory",
                "cpu",
                "timeout",
                "capabilities",
                "syscalls",
                "devices",
                "environment",
                "output",
            ],
        };
        let mut values: BTreeSet<String> = declared.into_iter().map(String::from).collect();
        // `prlimit` aporta un techo de memoria (RLIMIT_AS) a los adaptadores
        // que envuelven el proceso con él. Se comprueba en el host, no se
        // asume: declarar un control que no existe es exactamente lo que este
        // proyecto trata de evitar.
        //
        // `processes` NO se añade aquí. RLIMIT_NPROC cuenta los procesos del
        // UID real en todo el host, no los de la carga, así que no es el
        // control que la política pide. El techo real de PIDs lo pone el
        // controlador `pids` de cgroups v2, unas líneas más abajo.
        if matches!(self, Self::Bwrap | Self::Unshare) && command_exists("prlimit") {
            values.insert("memory".into());
        }
        // cgroups v2 vía scope de systemd: los únicos `memory`, `processes` y
        // `cpu` que este proyecto puede declarar de verdad. El sondeo levanta un
        // scope real, así que aquí ya se sabe si el kernel los acepta.
        //
        // Solo bubblewrap. `unshare` queda fuera a propósito: `systemd-run`
        // necesita `XDG_RUNTIME_DIR` y `DBUS_SESSION_BUS_ADDRESS` en el entorno,
        // y unshare se los pasaría tal cual a la carga. Bubblewrap no, porque
        // hace su propio `--clearenv` después de que systemd-run haya leído lo
        // que necesitaba.
        if self == Self::Bwrap {
            for control in &crate::cgroup::support().controls {
                values.insert(control.clone());
            }
        }
        // El control `network` sobrevive solo si la política pide un namespace
        // de red propio. Con `allowlist` o `unrestricted` la carga conserva la
        // red del host: da igual lo que declare el runtime, ahí no hay control
        // que declarar.
        if matches!(self, Self::Bwrap | Self::Unshare | Self::Wasi) && !policy.network.isolates_host_network() {
            values.remove("network");
        }
        values
    }
}

impl ExecutionPlan {
    pub fn build(runtime: RuntimeKind, workload: &Workload, policy: &Policy) -> Result<Self> {
        let requested = policy.enforcement.required_controls.clone();
        let supported = runtime.supported_controls(policy);
        let effective: Vec<_> = requested.iter().filter(|v| supported.contains(v.as_str())).cloned().collect();
        let unsupported: Vec<_> = requested.iter().filter(|v| !supported.contains(v.as_str())).cloned().collect();
        let probe = runtime.probe();
        let strict_block = policy.enforcement.mode == EnforcementMode::Strict && !unsupported.is_empty();
        let manual = matches!(runtime, RuntimeKind::Gvisor | RuntimeKind::Kata | RuntimeKind::Firecracker);
        let native_block = runtime == RuntimeKind::Native
            && (!workload.allow_native || env::var("SANDBOX_LABS_ALLOW_NATIVE").ok().as_deref() != Some("1"));
        let executable = runtime != RuntimeKind::DryRun && probe.available && !strict_block && !manual && !native_block;
        let block_reason = if runtime == RuntimeKind::DryRun {
            Some("dry-run no ejecuta la carga".into())
        } else if manual {
            Some("runtime documentado/manual: requiere integración específica del host".into())
        } else if native_block {
            // Antes de "runtime no disponible": para native la indisponibilidad es
            // exactamente la falta de opt-in, y ese mensaje es el accionable.
            Some("native requiere SANDBOX_LABS_ALLOW_NATIVE=1 y allowNative=true".into())
        } else if !probe.available {
            Some(format!("runtime no disponible: {}", probe.detail))
        } else if strict_block {
            Some(format!("la política estricta exige controles no soportados: {}", unsupported.join(", ")))
        } else {
            None
        };
        let steps = vec![
            format!("Carga registrada: {} ({})", workload.id, workload.risk),
            format!("Política: {} ({:?})", policy.id, policy.enforcement.mode),
            format!("Runtime: {} · disponible={}", runtime, probe.available),
            format!(
                "Controles efectivos: {}",
                if effective.is_empty() { "ninguno".into() } else { effective.join(", ") }
            ),
            format!(
                "Controles no soportados: {}",
                if unsupported.is_empty() { "ninguno".into() } else { unsupported.join(", ") }
            ),
            format!(
                "Límites: CPU={} · RAM={}MB · procesos={} · timeout={}s · salida={} bytes",
                policy.resources.cpu,
                policy.resources.memory_mb,
                policy.resources.processes,
                policy.resources.timeout_seconds,
                policy.resources.output_bytes
            ),
            format!("Red: {} · DNS: {}", policy.network.mode, policy.network.dns),
            format!("Filesystem raíz: {} · symlinks={}", policy.filesystem.root, policy.filesystem.follow_symlinks),
        ];
        Ok(Self {
            runtime,
            workload_id: workload.id.clone(),
            workload_path: workload.portable_path(),
            policy_id: policy.id.clone(),
            steps,
            executable,
            block_reason,
            controls: ControlAssessment { requested, effective, unsupported },
        })
    }
}

pub fn command_exists(name: &str) -> bool {
    Command::new(name).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}
