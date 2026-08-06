//! cgroups v2: los únicos límites de memoria, PIDs y CPU que este proyecto
//! puede declarar como controles.
//!
//! # Por qué no basta con `prlimit`
//!
//! `RLIMIT_AS` acota el espacio de direcciones **virtual**, que no es la
//! memoria que el proceso ocupa de verdad; y `RLIMIT_NPROC` cuenta los procesos
//! del UID real en **todo el host**, no los de la carga. Ninguno de los dos es
//! el control que la política pide. Los de verdad son `memory.max`, `pids.max`
//! y `cpu.max` del árbol de cgroups v2.
//!
//! # Por qué a través de systemd y no escribiendo en `/sys/fs/cgroup`
//!
//! Crear un cgroup hijo a mano exige que el cgroup del proceso que lanza esté
//! delegado y sea escribible. Medido en un WSL2 con systemd, el proceso que
//! arranca desde `wsl.exe` vive en `/init.scope`, que no es escribible:
//!
//! ```text
//! own dir: /sys/fs/cgroup/init.scope   writable: no
//! mkdir hijo: FALLO
//! ```
//!
//! Mientras que pedírselo al gestor de usuario sí funciona, sin privilegios:
//!
//! ```text
//! systemd-run --user --scope -p MemoryMax=128M -p TasksMax=32 -- ...
//!   → 0::/user.slice/user-1000.slice/user@1000.service/app.slice/run-….scope
//!     memory.max=134217728
//!     pids.max=32
//!```
//!
//! Así que el camino es `systemd-run --user --scope`. Donde no haya gestor de
//! usuario —contenedores sin systemd, CI, algunas distribuciones— el sondeo
//! devuelve «no disponible» y los controles **no se declaran**. Nunca se
//! sustituyen por un `prlimit` disfrazado.
//!
//! # Qué NO hace todavía
//!
//! Aplicar los límites y observar el consumo son cosas distintas. Esto aplica.
//! Leer `memory.peak`, `pids.peak` y el contador `oom_kill` de `memory.events`
//! exige muestrear el cgroup mientras la carga corre, porque systemd retira el
//! cgroup en cuanto el scope termina. Está en B-02 del backlog técnico.

use crate::ResourcePolicy;
use std::{
    process::{Command, Stdio},
    sync::OnceLock,
};

/// Qué puede respaldar el host en materia de límites de recursos.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CgroupSupport {
    pub available: bool,
    /// Qué se comprobó y qué salió. Va al informe de `doctor` y a la evidencia.
    pub detail: String,
    /// Controles que este mecanismo puede respaldar. Vacío si no está.
    pub controls: Vec<String>,
}

impl CgroupSupport {
    fn unavailable(detail: impl Into<String>) -> Self {
        Self { available: false, detail: detail.into(), controls: vec![] }
    }
}

/// Controles que un scope de systemd respalda cuando está disponible.
///
/// `memory` ← `MemoryMax`, `processes` ← `TasksMax`, `cpu` ← `CPUQuota`.
const CONTROLS: [&str; 3] = ["memory", "processes", "cpu"];

static SUPPORT: OnceLock<CgroupSupport> = OnceLock::new();

/// Sondea una vez por proceso y cachea: la comprobación arranca un scope real,
/// y repetirla por cada ejecución sería un coste sin información nueva.
pub fn support() -> &'static CgroupSupport {
    SUPPORT.get_or_init(probe)
}

/// El sondeo, sin caché. No pregunta si el mecanismo existe: lo **usa**.
///
/// Comprobar que `systemd-run` está en el PATH no dice nada —puede estar y no
/// haber gestor de usuario, o haberlo y no tener los controladores delegados—.
/// La única respuesta fiable es levantar un scope de verdad con los tres
/// límites puestos y ver si el kernel los acepta.
fn probe() -> CgroupSupport {
    if !cfg!(target_os = "linux") {
        return CgroupSupport::unavailable("cgroups v2 solo existe en Linux");
    }
    if !std::path::Path::new("/sys/fs/cgroup/cgroup.controllers").exists() {
        return CgroupSupport::unavailable("no hay cgroups v2 montado en /sys/fs/cgroup");
    }
    let mut command = Command::new("systemd-run");
    command
        .args(["--user", "--scope", "--quiet", "--collect"])
        .args(scope_properties(&probe_limits()))
        .args(["--", "true"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    match command.output() {
        Err(error) => CgroupSupport::unavailable(format!("systemd-run no se pudo ejecutar: {error}")),
        Ok(output) if !output.status.success() => {
            let reason = String::from_utf8_lossy(&output.stderr);
            let reason = reason.lines().next().unwrap_or("sin detalle").trim();
            CgroupSupport::unavailable(format!("systemd-run --user --scope falló: {reason}"))
        }
        Ok(_) => CgroupSupport {
            available: true,
            detail: "systemd-run --user --scope aplica MemoryMax, TasksMax y CPUQuota".into(),
            controls: CONTROLS.iter().map(|value| value.to_string()).collect(),
        },
    }
}

/// Límites mínimos con los que se hace el sondeo. Tienen que ser válidos y
/// pequeños: el scope de prueba solo ejecuta `true`.
fn probe_limits() -> ResourcePolicy {
    ResourcePolicy { cpu: 1.0, memory_mb: 64, processes: 16, timeout_seconds: 5, open_files: 32, output_bytes: 4096 }
}

/// Las propiedades `-p` que traducen la política a límites del kernel.
///
/// `CPUQuota` se expresa en porcentaje de **un** núcleo, así que `cpu: 2.0` son
/// 200%. systemd lo traduce a `cpu.max`.
pub fn scope_properties(limits: &ResourcePolicy) -> Vec<String> {
    vec![
        "-p".into(),
        format!("MemoryMax={}M", limits.memory_mb),
        "-p".into(),
        format!("TasksMax={}", limits.processes),
        "-p".into(),
        format!("CPUQuota={}%", (limits.cpu * 100.0).round() as u64),
    ]
}

/// Envuelve un programa en un scope transitorio con los límites de la política.
///
/// Devuelve `None` cuando el host no puede respaldarlos, y entonces quien llama
/// tiene que ejecutar sin envolver **y no declarar** los controles. Nunca
/// devuelve un envoltorio a medias.
///
/// `--collect` hace que systemd retire la unidad en cuanto el proceso termina:
/// sin eso, un scope fallido quedaría en el gestor hasta un `reset-failed`.
///
/// El envoltorio **no** rompe el control `timeout`. `systemd-run --scope`
/// registra la unidad y después hace `exec`, así que el PID que devuelve
/// `spawn` es el del proceso real, no el de un intermediario. Medido:
///
/// ```text
/// pid lanzado: 527   exe: /usr/bin/sleep   cmdline: /usr/bin/sleep 47
/// kill 527 → el sleep muere
/// ```
///
/// Si systemd-run se quedara vivo como padre, matar por timeout dejaría al
/// proceso interno huérfano y el control sería falso.
pub fn wrap(program: &str, args: &[String], limits: &ResourcePolicy) -> Option<(String, Vec<String>)> {
    if !support().available {
        return None;
    }
    let mut wrapped = vec!["--user".into(), "--scope".into(), "--quiet".into(), "--collect".into()];
    wrapped.extend(scope_properties(limits));
    wrapped.push("--".into());
    wrapped.push(program.to_string());
    wrapped.extend_from_slice(args);
    Some(("systemd-run".into(), wrapped))
}

/// Variables que `systemd-run --user` necesita para encontrar el bus del gestor
/// de usuario.
///
/// Importa porque los adaptadores limpian el entorno antes de ejecutar. Estas
/// dos las lee `systemd-run`, no la carga: bubblewrap hace su propio
/// `--clearenv` después, así que no llegan dentro del sandbox.
pub const REQUIRED_ENVIRONMENT: [&str; 2] = ["XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS"];

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> ResourcePolicy {
        ResourcePolicy {
            cpu: 1.5,
            memory_mb: 256,
            processes: 32,
            timeout_seconds: 30,
            open_files: 64,
            output_bytes: 8192,
        }
    }

    #[test]
    fn translates_the_policy_into_kernel_limits() {
        let properties = scope_properties(&limits()).join(" ");
        assert!(properties.contains("MemoryMax=256M"), "{properties}");
        assert!(properties.contains("TasksMax=32"), "{properties}");
        // 1.5 núcleos son 150% de uno.
        assert!(properties.contains("CPUQuota=150%"), "{properties}");
    }

    #[test]
    fn wrapping_preserves_the_inner_command() {
        let inner = vec!["--unshare-net".to_string(), "--".to_string(), "python3".to_string()];
        match wrap("bwrap", &inner, &limits()) {
            // Sin soporte no se envuelve, y quien llama no declara los
            // controles. Es el camino correcto en un host sin systemd.
            None => assert!(!support().available),
            Some((program, args)) => {
                assert_eq!(program, "systemd-run");
                let separator = args.iter().position(|value| value == "--").expect("separador");
                assert_eq!(args[separator + 1], "bwrap", "el programa envuelto va justo detrás de --");
                assert_eq!(&args[separator + 2..], &inner[..], "los argumentos internos no se tocan");
            }
        }
    }

    #[test]
    fn support_is_cached_and_consistent() {
        assert_eq!(support(), support());
        // Un sondeo sin disponibilidad no puede declarar controles.
        if !support().available {
            assert!(support().controls.is_empty());
            assert!(!support().detail.is_empty(), "la indisponibilidad tiene que explicarse");
        }
    }
}
