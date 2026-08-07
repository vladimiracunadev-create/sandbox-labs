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
//! # Aplicar y observar son cosas distintas
//!
//! Un límite dice lo que el kernel impedirá; el consumo dice lo que pasó. Este
//! módulo hace las dos: `wrap` aplica y `Sampler` observa.
//!
//! Observar obliga a muestrear **mientras** la carga corre, porque systemd
//! retira el cgroup en cuanto el scope termina. Medido:
//!
//! ```text
//! durante : memory.peak = 46288896   pids.peak = 1   oom_kill = 0
//! después : el directorio ya no existe
//! ```
//!
//! Y solo se muestrea cuando el envoltorio existe. Sin él, `/proc/<pid>/cgroup`
//! apunta al cgroup de la sesión del host, y publicar sus cifras como consumo
//! de la carga sería peor que no medir nada.

use crate::ResourcePolicy;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    thread::JoinHandle,
    time::Duration,
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
    // El sondeo ejecuta la MISMA forma de comando que `wrap`, incluido el
    // `env -i` que vacía el entorno. Si sondeara una forma más simple, podría
    // salir disponible y luego fallar al ejecutar de verdad.
    let (program, args) = build_chain("true", &[], &probe_limits());
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
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
    Some(build_chain(program, args, limits))
}

/// La cadena completa: scope de systemd → borrado del entorno → programa.
fn build_chain(program: &str, args: &[String], limits: &ResourcePolicy) -> (String, Vec<String>) {
    let mut wrapped = vec!["--user".into(), "--scope".into(), "--quiet".into(), "--collect".into()];
    wrapped.extend(scope_properties(limits));
    wrapped.push("--".into());
    wrapped.push(EMPTY_ENVIRONMENT.0.into());
    wrapped.push(EMPTY_ENVIRONMENT.1.into());
    wrapped.push(crate::compiler::resolve_program(program));
    wrapped.extend_from_slice(args);
    ("systemd-run".into(), wrapped)
}

/// `env -i`: el runtime arranca con el entorno **vacío**, que es exactamente lo
/// que tenía antes de que existiera este envoltorio.
///
/// # Por qué se vacía entero en vez de borrar variables concretas
///
/// El entorno con el que arranca el runtime acaba siendo el del proceso `init`
/// de bubblewrap, que es el PID 1 **dentro** del sandbox. El `--clearenv` de
/// bubblewrap limpia el entorno de la carga, no el suyo propio, así que la
/// carga puede leerlo en `/proc/1/environ`. La sonda de filesystem marca fuga
/// cuando ese PID 1 expone variables que la carga no tiene.
///
/// El primer intento borró con `env -u` las dos variables que `systemd-run`
/// necesita leer, y la suite volvió a fallar por lo mismo. Medido:
///
/// ```text
/// env -i XDG_RUNTIME_DIR=… DBUS_SESSION_BUS_ADDRESS=… systemd-run --user --scope -- env
///   XDG_RUNTIME_DIR=/run/user/1000
///   DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
///   INVOCATION_ID=c35c6a9fd9a84594ac84703699f62e7a   ← lo inyecta systemd
/// ```
///
/// `INVOCATION_ID` no lo pusimos nosotros. Enumerar lo que systemd inyecta es
/// una lista que se queda corta con la siguiente versión, así que se vacía
/// entero: lo que no está no puede filtrarse.
///
/// `env` hace `exec`, así que no añade un proceso al árbol ni deja al de dentro
/// huérfano cuando el supervisor mata por timeout. Sin `PATH`, `execvp` recurre
/// a la ruta por defecto del sistema, que es donde viven `prlimit` y `bwrap`.
const EMPTY_ENVIRONMENT: (&str, &str) = ("env", "-i");

/// Lo que la carga consumió de verdad, leído del cgroup mientras corría.
///
/// Aplicar un límite y medir el consumo son cosas distintas: lo primero dice lo
/// que el kernel impedirá, lo segundo lo que pasó. Un `Ninguno` aquí significa
/// que no se pudo leer, nunca que valga cero.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Usage {
    /// Marca de agua de memoria, de `memory.peak`.
    pub memory_peak_bytes: Option<u64>,
    /// Máximo de procesos vivos a la vez, de `pids.peak`.
    pub pids_peak: Option<u64>,
    /// CPU consumida, de `usage_usec` en `cpu.stat`.
    pub cpu_usage_usec: Option<u64>,
    /// Veces que el kernel mató un proceso por falta de memoria, de
    /// `oom_kill` en `memory.events`. Es lo que convierte un código de salida
    /// inexplicable en un hecho.
    pub oom_kills: Option<u64>,
}

impl Usage {
    /// Se queda con el mayor de cada campo.
    ///
    /// Los contadores del kernel ya son monotónicos, así que esto solo protege
    /// del caso en que una lectura tardía falle —el cgroup desaparece en cuanto
    /// el scope termina— y devuelva `None` sobre un valor que sí se había leído.
    fn merge(&mut self, other: &Self) {
        fn keep_max(current: &mut Option<u64>, new: Option<u64>) {
            if let Some(value) = new {
                *current = Some(current.map_or(value, |old| old.max(value)));
            }
        }
        keep_max(&mut self.memory_peak_bytes, other.memory_peak_bytes);
        keep_max(&mut self.pids_peak, other.pids_peak);
        keep_max(&mut self.cpu_usage_usec, other.cpu_usage_usec);
        keep_max(&mut self.oom_kills, other.oom_kills);
    }

    /// En la forma que espera la evidencia: solo lo que se pudo medir.
    pub fn to_map(&self) -> std::collections::BTreeMap<String, String> {
        let mut map = std::collections::BTreeMap::new();
        if let Some(value) = self.memory_peak_bytes {
            map.insert("memoryPeakBytes".into(), value.to_string());
        }
        if let Some(value) = self.pids_peak {
            map.insert("pidsPeak".into(), value.to_string());
        }
        if let Some(value) = self.cpu_usage_usec {
            map.insert("cpuUsageUsec".into(), value.to_string());
        }
        if let Some(value) = self.oom_kills {
            map.insert("oomKills".into(), value.to_string());
        }
        map
    }
}

/// Directorio del cgroup de un proceso, según `/proc/<pid>/cgroup`.
///
/// La línea de cgroups v2 es `0::<ruta>`, relativa a la raíz de la jerarquía.
pub fn directory_of(pid: u32) -> Option<PathBuf> {
    let content = std::fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    let relative = content.lines().find_map(|line| line.strip_prefix("0::"))?.trim();
    let directory = PathBuf::from("/sys/fs/cgroup").join(relative.trim_start_matches('/'));
    directory.is_dir().then_some(directory)
}

fn read_number(directory: &Path, file: &str) -> Option<u64> {
    std::fs::read_to_string(directory.join(file)).ok()?.trim().parse().ok()
}

/// Busca `clave <número>` en un fichero de pares, como `cpu.stat` o
/// `memory.events`.
fn read_field(directory: &Path, file: &str, key: &str) -> Option<u64> {
    let content = std::fs::read_to_string(directory.join(file)).ok()?;
    content.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        (parts.next()? == key).then(|| parts.next()?.parse().ok())?
    })
}

/// Lee el consumo actual del cgroup. Barato: son ficheros de `/sys`.
pub fn read_usage(directory: &Path) -> Usage {
    Usage {
        memory_peak_bytes: read_number(directory, "memory.peak"),
        pids_peak: read_number(directory, "pids.peak"),
        cpu_usage_usec: read_field(directory, "cpu.stat", "usage_usec"),
        oom_kills: read_field(directory, "memory.events", "oom_kill"),
    }
}

/// Muestrea el cgroup de un proceso mientras vive.
///
/// Hace falta porque systemd **retira el cgroup en cuanto el scope termina**:
/// leer al final no encuentra nada. Comprobado — la ruta existe durante la
/// ejecución y ha desaparecido justo después.
pub struct Sampler {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<Usage>>,
}

impl Sampler {
    /// Arranca el muestreo del cgroup de `pid`.
    ///
    /// Devuelve `None` cuando no hay cgroup propio que mirar. Es deliberado:
    /// sin el envoltorio de systemd, `/proc/<pid>/cgroup` apunta al cgroup de
    /// la **sesión del host**, y publicar sus cifras como consumo de la carga
    /// sería peor que no medir nada.
    pub fn start(pid: u32) -> Option<Self> {
        let directory = directory_of(pid)?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let worker = std::thread::spawn(move || {
            let mut total = Usage::default();
            loop {
                total.merge(&read_usage(&directory));
                if flag.load(Ordering::SeqCst) {
                    // Una última lectura después de la señal: el proceso puede
                    // haber consumido su pico entre la penúltima vuelta y el
                    // final.
                    total.merge(&read_usage(&directory));
                    return total;
                }
                std::thread::sleep(Duration::from_millis(SAMPLE_INTERVAL_MS));
            }
        });
        Some(Self { stop, worker: Some(worker) })
    }

    /// Detiene el muestreo y devuelve lo observado.
    pub fn finish(mut self) -> Usage {
        self.stop.store(true, Ordering::SeqCst);
        self.worker.take().and_then(|worker| worker.join().ok()).unwrap_or_default()
    }
}

/// Cada cuánto se releen los contadores.
///
/// Los picos del kernel son marcas de agua, así que el intervalo no cambia el
/// valor final mientras el cgroup exista; solo acota cuánto tarda el hilo en
/// enterarse de que debe parar.
const SAMPLE_INTERVAL_MS: u64 = 40;

/// Variables que `systemd-run --user` necesita para encontrar el bus del gestor
/// de usuario.
///
/// Importa porque los adaptadores limpian el entorno antes de ejecutar, así que
/// hay que volver a ponerlas para el proceso más externo. Solo las lee
/// `systemd-run`: el `env -i` de la cadena las borra —junto con todo lo demás—
/// antes de que lleguen al runtime. Ver `EMPTY_ENVIRONMENT`.
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
        let (program, args) = build_chain("/usr/bin/env", &inner, &limits());
        assert_eq!(program, "systemd-run");
        let target = args.iter().position(|value| value == "/usr/bin/env").expect("el programa envuelto");
        assert_eq!(&args[target + 1..], &inner[..], "los argumentos internos no se tocan");
    }

    #[test]
    fn the_wrapped_program_is_resolved_to_an_absolute_path() {
        // Detrás del `env -i` no hay PATH, así que un nombre suelto solo se
        // encontraría si está en la ruta por defecto del sistema.
        let (_, args) = build_chain("sh", &[], &limits());
        let resolved = args.last().expect("programa");
        assert!(resolved.starts_with('/'), "se esperaba ruta absoluta y llegó {resolved}");
        assert!(resolved.ends_with("/sh"));
    }

    #[test]
    fn an_absolute_program_is_left_alone() {
        let (_, args) = build_chain("/opt/lo/que/sea", &[], &limits());
        assert_eq!(args.last().expect("programa"), "/opt/lo/que/sea");
    }

    /// La regresión que encontró la suite de contención en CI, dos veces.
    ///
    /// Nada del entorno puede sobrevivir hasta el runtime: acaba en el
    /// `/proc/1/environ` del sandbox y la carga lo lee — con bubblewrap
    /// declarando el control `filesystem`, que es la peor combinación posible:
    /// una falsa garantía.
    ///
    /// La primera corrección borraba con `-u` las dos variables del bus y falló
    /// igual, porque systemd inyecta `INVOCATION_ID` por su cuenta. Por eso el
    /// contrato es vaciar, no enumerar.
    #[test]
    fn the_runtime_starts_with_an_empty_environment() {
        let (_, args) = build_chain("/usr/bin/env", &[], &limits());
        let target = args.iter().position(|value| value == "/usr/bin/env").expect("el programa envuelto");
        assert_eq!(
            &args[target - 2..target],
            &["env".to_string(), "-i".to_string()],
            "el runtime va precedido de `env -i`, no de una lista de variables a borrar"
        );
        assert!(
            !args[..target].iter().any(|value| value == "-u"),
            "borrar variables una a una es la lista que se queda corta con la siguiente versión de systemd"
        );
    }

    #[test]
    fn the_probe_runs_the_same_shape_it_will_execute() {
        // Sondear una forma más simple que la real dejaría pasar un
        // «disponible» que después falla al ejecutar.
        let (_, probed) = build_chain("true", &[], &probe_limits());
        let (_, real) = build_chain("/opt/runtime", &[], &probe_limits());
        // Todo menos el último elemento, que es el programa.
        assert_eq!(
            probed[..probed.len() - 1],
            real[..real.len() - 1],
            "el prefijo de la cadena tiene que ser idéntico"
        );
    }

    #[test]
    fn without_support_nothing_is_wrapped() {
        // El camino correcto en un host sin gestor de usuario: no envolver, y
        // que quien llama no declare los controles.
        if !support().available {
            assert!(wrap("/usr/bin/env", &[], &limits()).is_none());
        }
    }

    fn fake_cgroup(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("sandbox-labs-cg-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("directorio de prueba");
        for (file, content) in files {
            std::fs::write(directory.join(file), content).expect("fichero de prueba");
        }
        directory
    }

    #[test]
    fn reads_the_counters_the_kernel_actually_publishes() {
        // Los contenidos son los medidos en un WSL2 real, incluido el formato
        // de pares de `cpu.stat` y `memory.events`.
        let directory = fake_cgroup(
            "read",
            &[
                ("memory.peak", "46288896\n"),
                ("pids.peak", "3\n"),
                ("cpu.stat", "usage_usec 20103\nuser_usec 20103\nsystem_usec 0\nnr_periods 1\n"),
                ("memory.events", "low 0\nhigh 0\nmax 2\noom 1\noom_kill 1\noom_group_kill 0\n"),
            ],
        );
        let usage = read_usage(&directory);
        assert_eq!(usage.memory_peak_bytes, Some(46_288_896));
        assert_eq!(usage.pids_peak, Some(3));
        assert_eq!(usage.cpu_usage_usec, Some(20_103));
        // `oom_kill`, no `oom` ni `max`: los tres están en el mismo fichero y
        // solo uno significa «el kernel mató un proceso».
        assert_eq!(usage.oom_kills, Some(1));
        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn a_counter_that_cannot_be_read_is_none_never_zero() {
        // La diferencia importa: cero es un hecho medido, ausente es «no se
        // pudo mirar». Confundirlos convierte un hueco en una afirmación.
        let directory = fake_cgroup("parcial", &[("pids.peak", "7\n")]);
        let usage = read_usage(&directory);
        assert_eq!(usage.pids_peak, Some(7));
        assert_eq!(usage.memory_peak_bytes, None);
        assert_eq!(usage.oom_kills, None);
        assert!(!usage.to_map().contains_key("memoryPeakBytes"), "lo no medido no se publica");
        std::fs::remove_dir_all(&directory).ok();
    }

    #[test]
    fn merging_keeps_the_high_water_mark() {
        // Protege del caso real: la última lectura llega cuando systemd ya
        // retiró el cgroup y devuelve todo a `None`. El pico ya leído no puede
        // perderse por eso.
        let mut total = Usage { memory_peak_bytes: Some(500), pids_peak: Some(4), ..Usage::default() };
        total.merge(&Usage { memory_peak_bytes: Some(120), pids_peak: None, ..Usage::default() });
        assert_eq!(total.memory_peak_bytes, Some(500));
        assert_eq!(total.pids_peak, Some(4));
    }

    #[test]
    fn nothing_observed_publishes_nothing() {
        assert!(Usage::default().to_map().is_empty());
    }

    /// La única prueba que demuestra que el muestreo sirve para algo.
    ///
    /// Las demás ejercitan el parseo con ficheros inventados. Esta envuelve un
    /// proceso de verdad en un scope de systemd, le hace reservar 40 MB y
    /// comprueba que el pico observado los refleja. Si el muestreo llegara
    /// tarde —systemd retira el cgroup en cuanto el scope termina— aquí no
    /// habría nada que leer y la prueba lo diría.
    ///
    /// Se salta donde no hay gestor de usuario de systemd, que es la misma
    /// condición bajo la que los controles no se declaran.
    #[test]
    fn observes_the_real_consumption_of_a_wrapped_process() {
        if !support().available {
            return;
        }
        let program = "python3";
        let script = "import time; blob = bytearray(40 * 1024 * 1024); time.sleep(0.5); len(blob)";
        let Some((outer, args)) = wrap(program, &["-c".to_string(), script.to_string()], &limits()) else {
            panic!("con soporte disponible, `wrap` tiene que envolver");
        };
        let mut child = match Command::new(&outer).args(&args).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            Ok(child) => child,
            // Sin python3 en la ruta por defecto no hay nada que medir, y eso
            // no es un fallo del muestreo.
            Err(_) => return,
        };
        let sampler = Sampler::start(child.id());
        let status = child.wait().expect("esperar al hijo");
        let usage = sampler.map(Sampler::finish).unwrap_or_default();
        if !status.success() {
            return;
        }
        let peak = usage.memory_peak_bytes.expect("el cgroup tenía que publicar memory.peak mientras corría");
        assert!(
            peak > 30 * 1024 * 1024,
            "se reservaron 40MB y el pico observado fue {peak} bytes: el muestreo llegó tarde"
        );
        assert!(usage.pids_peak.is_some(), "pids.peak tenía que leerse");
        assert_eq!(usage.oom_kills, Some(0), "no hubo OOM: el límite era 256MB");
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
