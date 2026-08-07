//! El compilador de políticas a argumentos de bubblewrap. **Uno solo.**
//!
//! # Por qué existe este módulo
//!
//! Había dos. Las cargas que terminan se compilaban en el adaptador de
//! bubblewrap y los servicios de larga duración en el lanzador del CLI, cada uno
//! con su lista de argumentos escrita a mano. Dos caminos hacia el mismo kernel
//! son dos sitios donde un control puede perderse, y solo uno de ellos estaba
//! cubierto por la suite de contención.
//!
//! No era teórico. Cuando se compararon, al camino de los servicios le faltaban:
//!
//! - `--cap-drop ALL`, aunque su política exige el control `capabilities`
//! - `--uid`/`--gid`, así que el servicio corría con la identidad de quien lo
//!   levantó
//! - `--new-session`, que es lo que impide inyectar en el terminal con `TIOCSTI`
//! - `--unshare-cgroup-try`
//! - el filtro seccomp
//!
//! Ninguno se perdió por descuido: se perdieron porque nadie tenía que
//! acordarse de añadirlos en dos sitios a la vez, y eso es exactamente lo que
//! este módulo elimina.
//!
//! # Qué queda fuera
//!
//! Solo lo que de verdad distingue una ejecución de otra: qué se monta, dónde se
//! trabaja, qué variables extra y qué se ejecuta. Todo lo que viene de la
//! política —namespaces, capabilities, identidad, red, syscalls, entorno— se
//! decide aquí y en ningún otro sitio.

use crate::Policy;
use std::{collections::BTreeMap, path::Path};

/// Un montaje del sandbox.
pub struct Mount {
    pub source: String,
    /// Ruta **dentro** del sandbox.
    pub target: String,
    /// De escritura. Por defecto todo entra de solo lectura.
    pub writable: bool,
}

impl Mount {
    pub fn read_only(source: impl Into<String>, target: impl Into<String>) -> Self {
        Self { source: source.into(), target: target.into(), writable: false }
    }

    pub fn writable(source: impl Into<String>, target: impl Into<String>) -> Self {
        Self { source: source.into(), target: target.into(), writable: true }
    }
}

/// Lo que cambia entre una ejecución y otra.
pub struct SandboxRequest {
    pub mounts: Vec<Mount>,
    /// Directorio de trabajo dentro del sandbox.
    pub workdir: String,
    /// Variables **además** de las que declara la política. Aquí van los
    /// secretos que la política autorizó y los datos que el proceso necesita
    /// para orientarse, como el puerto o la ruta de su socket.
    pub environment: BTreeMap<String, String>,
    pub command: String,
    pub args: Vec<String>,
    /// ¿Debe morir el sandbox cuando muera quien lo lanzó?
    ///
    /// Sí para una carga que termina: el supervisor la espera, y si el
    /// supervisor cae no puede quedar una jaula huérfana con algo dentro.
    ///
    /// **No** para un servicio: `sandboxctl service up` termina en cuanto
    /// informa, y con esto puesto el servicio moría con él. El precio es que un
    /// CLI que caiga deja el sandbox vivo, y por eso el registro guarda su PID y
    /// existe `service down`.
    pub die_with_parent: bool,
}

/// Rutas del sistema que se montan de solo lectura para que el intérprete pueda
/// arrancar. Sin ellas no hay Python, ni `sh`, ni nada.
const SYSTEM_PATHS: [&str; 4] = ["/usr", "/bin", "/lib", "/lib64"];

/// Ficheros que hacen que `getpwuid` funcione dentro. Sin ellos, un proceso que
/// pregunte por su propio usuario recibe un error donde esperaba un nombre.
const IDENTITY_FILES: [&str; 2] = ["/etc/passwd", "/etc/group"];

/// Argumentos de bubblewrap para esta petición bajo esta política.
///
/// `seccomp_fd` es el descriptor por el que bubblewrap leerá el filtro, o
/// `None` si no hay filtro que aplicar. Quien llama es responsable de que ese
/// descriptor exista en el hijo.
pub fn bubblewrap(policy: &Policy, request: &SandboxRequest, seccomp_fd: Option<i32>) -> Vec<String> {
    let mut args: Vec<String> = [
        // Sesión propia. Sin esto, un proceso con el terminal del host puede
        // inyectar caracteres en él con `TIOCSTI` y ejecutar comandos como
        // quien lo lanzó.
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
    .collect();

    if request.die_with_parent {
        args.push("--die-with-parent".into());
    }

    // El flag va suelto, antes de cualquier `--ro-bind`: intercalarlo entre esa
    // opción y sus dos rutas rompería el parseo de argumentos de bubblewrap.
    if policy.network.isolates_host_network() {
        args.push("--unshare-net".into());
    }

    for mount in &request.mounts {
        args.push(if mount.writable { "--bind".into() } else { "--ro-bind".into() });
        args.push(mount.source.clone());
        args.push(mount.target.clone());
    }

    for system in SYSTEM_PATHS.iter().chain(IDENTITY_FILES.iter()) {
        if Path::new(system).exists() {
            args.extend(["--ro-bind".to_string(), (*system).to_string(), (*system).to_string()]);
        }
    }

    // Resolución de nombres solo si hay red que resolver. Montarlo dentro de un
    // namespace sin rutas no da acceso a nada, pero sí filtra los servidores DNS
    // de la red del usuario a una carga que no tiene por qué conocerlos.
    if !policy.network.isolates_host_network() && Path::new("/etc/resolv.conf").exists() {
        args.extend(["--ro-bind".into(), "/etc/resolv.conf".into(), "/etc/resolv.conf".into()]);
    }

    args.extend([
        "--chdir".into(),
        request.workdir.clone(),
        // El entorno se vacía y después se rellena solo con lo declarado. Lo
        // que no aparezca en la política o en la petición, dentro no existe.
        "--clearenv".into(),
        "--cap-drop".into(),
        "ALL".into(),
        // Identidad propia. Sin esto la carga corre con el uid REAL de quien la
        // lanzó y hereda sus grupos suplementarios — la identidad que tiene
        // acceso al repositorio, al llavero y a la sesión.
        //
        // Va después de `--unshare-user`, que es su requisito: el mapeo de
        // bubblewrap es «uid de dentro → uid real», así que los montajes de
        // escritura siguen siendo accesibles aunque el número cambie.
        "--uid".into(),
        policy.process.user.to_string(),
        "--gid".into(),
        policy.process.group.to_string(),
    ]);

    if let Some(descriptor) = seccomp_fd {
        args.extend(["--seccomp".into(), descriptor.to_string()]);
    }

    for (name, value) in policy.process.environment.iter().chain(request.environment.iter()) {
        args.extend(["--setenv".into(), name.clone(), value.clone()]);
    }

    args.push("--".into());
    args.push(request.command.clone());
    args.extend(request.args.clone());
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy_with_network(mode: &str) -> Policy {
        serde_json::from_value(serde_json::json!({
            "id": "test",
            "enforcement": { "mode": "best-effort", "requiredControls": [] },
            "filesystem": { "root": "ephemeral", "readOnly": [], "writable": [], "maxWorkspaceMb": 64, "followSymlinks": false },
            "network": { "mode": mode, "hosts": [], "dns": "disabled" },
            "resources": { "cpu": 1.0, "memoryMb": 128, "processes": 8, "timeoutSeconds": 10, "openFiles": 32, "outputBytes": 4096 },
            "process": { "capabilities": [], "environment": { "LANG": "C.UTF-8" }, "allowedEnvironment": [], "user": 65534, "group": 65534 }
        }))
        .expect("política de prueba válida")
    }

    fn request() -> SandboxRequest {
        SandboxRequest {
            mounts: vec![Mount::read_only("/origen", "/workspace/app")],
            workdir: "/workspace/app".into(),
            environment: BTreeMap::new(),
            command: "python3".into(),
            args: vec!["app.py".into()],
            die_with_parent: true,
        }
    }

    fn pair(args: &[String], flag: &str) -> Option<String> {
        args.iter().position(|value| value == flag).and_then(|index| args.get(index + 1).cloned())
    }

    /// Los controles que la política pide y que antes se perdían en el camino de
    /// los servicios. Que estén no depende de qué se monte ni de qué se ejecute.
    #[test]
    fn every_execution_gets_the_controls_the_policy_asks_for() {
        let args = bubblewrap(&policy_with_network("none"), &request(), None);
        assert!(args.contains(&"--clearenv".to_string()));
        assert_eq!(pair(&args, "--cap-drop").as_deref(), Some("ALL"));
        assert_eq!(pair(&args, "--uid").as_deref(), Some("65534"));
        assert_eq!(pair(&args, "--gid").as_deref(), Some("65534"));
        assert!(args.contains(&"--new-session".to_string()), "TIOCSTI: sin sesión propia se inyecta en el terminal");
        assert!(args.contains(&"--unshare-user".to_string()));
        assert!(args.contains(&"--unshare-pid".to_string()));
    }

    #[test]
    fn the_network_namespace_follows_the_policy() {
        for mode in ["none", "loopback"] {
            let args = bubblewrap(&policy_with_network(mode), &request(), None);
            assert!(args.contains(&"--unshare-net".to_string()), "{mode} tiene que aislar la red");
            assert!(
                !args.contains(&"/etc/resolv.conf".to_string()),
                "{mode}: sin rutas hacia fuera, montar resolv.conf solo filtra los DNS del usuario"
            );
        }
        for mode in ["allowlist", "unrestricted"] {
            let args = bubblewrap(&policy_with_network(mode), &request(), None);
            assert!(!args.contains(&"--unshare-net".to_string()), "{mode} conserva la red del host");
        }
    }

    #[test]
    fn the_filter_descriptor_only_appears_when_there_is_a_filter() {
        assert!(!bubblewrap(&policy_with_network("none"), &request(), None).contains(&"--seccomp".to_string()));
        let args = bubblewrap(&policy_with_network("none"), &request(), Some(63));
        assert_eq!(pair(&args, "--seccomp").as_deref(), Some("63"));
    }

    #[test]
    fn the_request_environment_rides_on_top_of_the_policy() {
        let mut value = request();
        value.environment.insert("SANDBOX_PORT".into(), "8803".into());
        let args = bubblewrap(&policy_with_network("none"), &value, None);
        let names: Vec<_> = args
            .iter()
            .enumerate()
            .filter(|(_, value)| *value == "--setenv")
            .filter_map(|(index, _)| args.get(index + 1).cloned())
            .collect();
        assert!(names.contains(&"LANG".to_string()), "lo que declara la política");
        assert!(names.contains(&"SANDBOX_PORT".to_string()), "lo que añade la petición");
    }

    /// El fallo que encontró la unificación: un servicio con `--die-with-parent`
    /// muere en cuanto `service up` termina de informar. Solo se veía con
    /// bubblewrap, y bubblewrap no estaba instalado donde se probaban servicios.
    #[test]
    fn only_supervised_executions_die_with_their_parent() {
        let supervised = bubblewrap(&policy_with_network("none"), &request(), None);
        assert!(supervised.contains(&"--die-with-parent".to_string()));

        let detached = SandboxRequest { die_with_parent: false, ..request() };
        let args = bubblewrap(&policy_with_network("none"), &detached, None);
        assert!(
            !args.contains(&"--die-with-parent".to_string()),
            "un servicio tiene que sobrevivir al CLI que lo levanta"
        );
    }

    #[test]
    fn the_command_goes_last_after_the_separator() {
        let args = bubblewrap(&policy_with_network("none"), &request(), None);
        let separator = args.iter().position(|value| value == "--").expect("separador");
        assert_eq!(args[separator + 1], "python3");
        assert_eq!(args[separator + 2], "app.py");
    }

    #[test]
    fn writable_mounts_use_bind_and_the_rest_ro_bind() {
        let value = SandboxRequest {
            mounts: vec![
                Mount::read_only("/entrada", "/workspace/input"),
                Mount::writable("/salida", "/workspace/output"),
            ],
            ..request()
        };
        let args = bubblewrap(&policy_with_network("none"), &value, None);
        let at = |target: &str| args.iter().position(|value| value == target).expect("montaje");
        assert_eq!(args[at("/workspace/input") - 2], "--ro-bind");
        assert_eq!(args[at("/workspace/output") - 2], "--bind");
    }
}
