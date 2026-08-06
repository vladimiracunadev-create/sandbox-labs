//! Subcomando `service`: levanta, lista, inspecciona y baja sandboxes.
//!
//! El resto del CLI ejecuta cargas que terminan. Esto mantiene sandboxes
//! **en marcha**: un servicio dentro de una jaula, publicando un puerto en el
//! loopback del host, que se puede abrir en el navegador y bajar después.

use anyhow::{bail, Context, Result};
use sandbox_core::{
    service::{process_alive, Service, ServiceRecord, ServiceState},
    Policy, RuntimeKind,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    str::FromStr,
    time::{Duration, Instant},
};

/// Tiempo máximo esperando a que el puerto responda tras levantar.
const READY_TIMEOUT: Duration = Duration::from_secs(20);

pub struct ServiceContext {
    pub root: PathBuf,
    pub data_root: PathBuf,
}

impl ServiceContext {
    pub fn new(root: &Path) -> Result<Self> {
        let catalog = sandbox_core::Catalog::load(root.join("sandbox.config.json"))?;
        Ok(Self { root: root.to_path_buf(), data_root: root.join(catalog.project.data_directory) })
    }

    pub fn services(&self) -> Result<Vec<Service>> {
        let directory = self.root.join("services");
        let mut found = Vec::new();
        let Ok(entries) = fs::read_dir(&directory) else {
            return Ok(found);
        };
        for entry in entries.flatten() {
            let manifest = entry.path().join("service.json");
            if manifest.is_file() {
                found.push(Service::load(&manifest)?);
            }
        }
        found.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(found)
    }

    pub fn find(&self, id: &str) -> Result<Service> {
        self.services()?
            .into_iter()
            .find(|value| value.id == id)
            .with_context(|| format!("Servicio no registrado: {id}"))
    }

    pub fn log_path(&self, id: &str) -> PathBuf {
        self.data_root.join("services").join(format!("{id}.log"))
    }
}

/// Estado observado de un servicio: registro + proceso + puerto.
pub struct Observed {
    pub service: Service,
    pub record: Option<ServiceRecord>,
    pub state: ServiceState,
}

pub fn observe(ctx: &ServiceContext, service: Service) -> Observed {
    let record = ServiceRecord::read(&ctx.data_root, &service.id);
    let state = match &record {
        None => ServiceState::Stopped,
        Some(value) if !process_alive(value.pid) => ServiceState::Crashed,
        Some(_) if port_responds(service.port) => ServiceState::Running,
        Some(_) => ServiceState::Starting,
    };
    Observed { service, record, state }
}

/// ¿Hay algo escuchando en el puerto del loopback?
fn port_responds(port: u16) -> bool {
    std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_millis(400),
    )
    .is_ok()
}

/// Primer runtime de la lista que esté disponible en este host.
fn pick_runtime(service: &Service) -> Result<RuntimeKind> {
    for candidate in &service.runtimes {
        let kind = RuntimeKind::from_str(candidate)?;
        if kind.probe().available {
            return Ok(kind);
        }
    }
    bail!(
        "Ningún runtime de {} está disponible en este host (probados: {}). Ejecuta `sandboxctl doctor`.",
        service.id,
        service.runtimes.join(", ")
    )
}

/// Construye la línea de comandos del sandbox para un servicio.
///
/// No se reutiliza el adaptador de cargas porque ahí el proceso es hijo y se
/// espera a que termine. Un servicio tiene que sobrevivir al CLI que lo levanta.
fn sandbox_command(runtime: RuntimeKind, service: &Service, policy: &Policy) -> (String, Vec<String>) {
    let workdir = service.directory.display().to_string();
    let mut args: Vec<String> = Vec::new();

    match runtime {
        RuntimeKind::Bwrap => {
            args.extend(
                [
                    "--die-with-parent",
                    "--unshare-user",
                    "--unshare-ipc",
                    "--unshare-pid",
                    "--unshare-uts",
                    "--proc",
                    "/proc",
                    "--dev",
                    "/dev",
                    "--tmpfs",
                    "/tmp",
                    "--dir",
                    "/workspace",
                ]
                .iter()
                .map(|value| value.to_string()),
            );
            // La red se conserva a propósito: un servicio sin loopback no puede
            // publicar nada. La contención sigue viva en filesystem, PIDs,
            // capabilities y entorno — y la tarjeta del panel lo dice.
            if policy.network.mode == "none" {
                args.push("--unshare-net".into());
            }
            args.extend(["--ro-bind".into(), workdir.clone(), "/workspace/app".into()]);
            for system in ["/usr", "/bin", "/lib", "/lib64", "/etc/passwd", "/etc/group", "/etc/resolv.conf"] {
                if Path::new(system).exists() {
                    args.extend(["--ro-bind".into(), system.into(), system.into()]);
                }
            }
            args.extend(["--chdir".into(), "/workspace/app".into(), "--clearenv".into()]);
            for (name, value) in &policy.process.environment {
                args.extend(["--setenv".into(), name.clone(), value.clone()]);
            }
            args.extend(["--setenv".into(), "SANDBOX_RUNTIME".into(), "bwrap".into()]);
            args.extend(["--setenv".into(), "SANDBOX_PORT".into(), service.port.to_string()]);
            args.push("--".into());
            args.push(service.command.clone());
            args.push(service.entrypoint.clone());
            args.extend(service.args.clone());
            ("bwrap".to_string(), args)
        }
        RuntimeKind::Unshare => {
            args.extend(
                ["--user", "--map-root-user", "--mount", "--pid", "--fork", "--mount-proc", "--uts", "--ipc"]
                    .iter()
                    .map(|value| value.to_string()),
            );
            if policy.network.mode == "none" {
                args.push("--net".into());
            }
            args.push("--".into());
            args.push(service.command.clone());
            args.push(service.entrypoint.clone());
            args.extend(service.args.clone());
            ("unshare".to_string(), args)
        }
        _ => {
            // Sin sandbox: solo se llega aquí con opt-in explícito y sirve de
            // línea base para comparar contra los runtimes que sí contienen.
            let mut plain = vec![service.entrypoint.clone()];
            plain.extend(service.args.clone());
            (service.command.clone(), plain)
        }
    }
}

pub fn up(ctx: &ServiceContext, id: &str, wait: bool) -> Result<i32> {
    let service = ctx.find(id)?;
    let observed = observe(ctx, service.clone());

    if observed.state == ServiceState::Running {
        println!("✅ {} ya está corriendo en {}", service.id, service.url());
        return Ok(0);
    }
    if observed.state == ServiceState::Crashed {
        // Un registro huérfano impediría levantarlo de nuevo; se limpia y se
        // deja constancia en vez de fallar de forma opaca.
        println!("⚠️  {} tenía un registro huérfano de un proceso muerto; se limpia.", service.id);
        ServiceRecord::remove(&ctx.data_root, &service.id);
    }
    if port_responds(service.port) {
        bail!("El puerto {} ya está ocupado por otro proceso. Bájalo o cambia el puerto del servicio.", service.port);
    }

    let policy = Policy::load(ctx.root.join("policies").join(format!("{}.json", service.policy)))?;
    let runtime = pick_runtime(&service)?;
    let (program, args) = sandbox_command(runtime, &service, &policy);

    let log_path = ctx.log_path(&service.id);
    fs::create_dir_all(log_path.parent().expect("directorio de servicios"))?;
    let log = fs::File::create(&log_path)?;
    let log_err = log.try_clone()?;

    println!("▶ Levantando {} con {} · política {}", service.id, runtime, policy.id);

    let mut command = Command::new(&program);
    command
        .args(&args)
        .current_dir(&service.directory)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err));

    // El servicio arranca en su **propio grupo de procesos**. Sin esto, el
    // sandbox hereda el grupo del CLI y bajarlo con `kill -TERM -<pid>` mataría
    // también a quien lo levantó — incluido el panel. Además, tener grupo
    // propio es lo que permite terminar el árbol entero: un sandbox lanza hijos
    // y matar solo al padre dejaría el servicio escuchando dentro del namespace.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    if runtime == RuntimeKind::Unshare {
        // bwrap limpia el entorno con `--clearenv`; unshare no tiene equivalente,
        // así que se limpia aquí y se inyecta solo lo que la política declara.
        command.env_clear();
        command.envs(&policy.process.environment);
        command.env("SANDBOX_RUNTIME", "unshare");
        command.env("SANDBOX_PORT", service.port.to_string());
        command.env("PATH", "/usr/local/bin:/usr/bin:/bin");
    }

    let child = command.spawn().with_context(|| {
        format!("No se pudo levantar {} con {program}. ¿Está instalado? `sandboxctl doctor`", service.id)
    })?;

    let record = ServiceRecord {
        id: service.id.clone(),
        pid: child.id(),
        port: service.port,
        runtime: runtime.to_string(),
        policy: policy.id.clone(),
        started_at: chrono::Utc::now().to_rfc3339(),
        log_path: log_path.display().to_string(),
        effective_controls: runtime.supported_controls(&policy).into_iter().collect(),
    };
    record.write(&ctx.data_root)?;

    if !wait {
        println!("   PID {} · logs en {}", record.pid, log_path.display());
        return Ok(0);
    }

    let started = Instant::now();
    while started.elapsed() < READY_TIMEOUT {
        if port_responds(service.port) {
            println!("✅ {} responde en {}", service.id, service.url());
            println!("   contención efectiva: {}", record.effective_controls.join(", "));
            println!("   logs: {}", log_path.display());
            return Ok(0);
        }
        if !process_alive(record.pid) {
            let tail = fs::read_to_string(&log_path).unwrap_or_default();
            eprintln!("❌ {} murió al arrancar. Últimas líneas:", service.id);
            for line in tail.lines().rev().take(10).collect::<Vec<_>>().into_iter().rev() {
                eprintln!("   {line}");
            }
            ServiceRecord::remove(&ctx.data_root, &service.id);
            return Ok(1);
        }
        std::thread::sleep(Duration::from_millis(300));
    }

    eprintln!("❌ {} no respondió en {} s. Revisa {}", service.id, READY_TIMEOUT.as_secs(), log_path.display());
    Ok(1)
}

pub fn down(ctx: &ServiceContext, id: &str) -> Result<i32> {
    let service = ctx.find(id)?;
    let Some(record) = ServiceRecord::read(&ctx.data_root, &service.id) else {
        println!("· {} ya estaba detenido", service.id);
        return Ok(0);
    };

    if process_alive(record.pid) {
        terminate(record.pid);
        // Se le da margen a terminar por las buenas antes de insistir: un
        // servicio que cierra bien deja el puerto libre de inmediato.
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(5) && process_alive(record.pid) {
            std::thread::sleep(Duration::from_millis(200));
        }
        if process_alive(record.pid) {
            kill(record.pid);
        }
    }

    ServiceRecord::remove(&ctx.data_root, &service.id);
    println!("⏹ {} detenido", service.id);
    Ok(0)
}

/// Señaliza al grupo de procesos del servicio.
///
/// El grupo es propio (se fija con `process_group(0)` al levantarlo), así que
/// `-<pid>` alcanza al sandbox y a todo lo que haya lanzado dentro, y a nadie
/// más. Sin grupo propio esto mataría al proceso que lo levantó.
#[cfg(target_os = "linux")]
fn signal_group(pid: u32, signal: &str) {
    let _ = Command::new("kill").arg(signal).arg(format!("-{pid}")).status();
    // Y al proceso suelto, por si el grupo ya no existe pero él sigue vivo.
    let _ = Command::new("kill").arg(signal).arg(pid.to_string()).status();
}

#[cfg(target_os = "linux")]
fn terminate(pid: u32) {
    signal_group(pid, "-TERM");
}

#[cfg(target_os = "linux")]
fn kill(pid: u32) {
    signal_group(pid, "-KILL");
}

#[cfg(not(target_os = "linux"))]
fn terminate(pid: u32) {
    let _ = Command::new("taskkill").args(["/PID", &pid.to_string(), "/T"]).status();
}

#[cfg(not(target_os = "linux"))]
fn kill(pid: u32) {
    let _ = Command::new("taskkill").args(["/F", "/PID", &pid.to_string(), "/T"]).status();
}

pub fn down_all(ctx: &ServiceContext) -> Result<i32> {
    for service in ctx.services()? {
        if ServiceRecord::read(&ctx.data_root, &service.id).is_some() {
            down(ctx, &service.id)?;
        }
    }
    Ok(0)
}

pub fn list(ctx: &ServiceContext, json_output: bool) -> Result<i32> {
    let observed: Vec<Observed> = ctx.services()?.into_iter().map(|value| observe(ctx, value)).collect();

    if json_output {
        let payload: Vec<_> = observed
            .iter()
            .map(|value| {
                serde_json::json!({
                    "id": value.service.id,
                    "name": value.service.name,
                    "category": value.service.category,
                    "description": value.service.description,
                    "teaches": value.service.teaches,
                    "port": value.service.port,
                    "url": value.service.url(),
                    "policy": value.service.policy,
                    "runtimes": value.service.runtimes,
                    "state": value.state,
                    "record": value.record,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(0);
    }

    if observed.is_empty() {
        println!("No hay servicios registrados en services/.");
        return Ok(0);
    }

    println!("{:<20} {:<12} {:>7}  {:<26} RUNTIME", "SERVICIO", "ESTADO", "PUERTO", "URL");
    println!("{}", "─".repeat(94));
    for value in &observed {
        let symbol = match value.state {
            ServiceState::Running => "🟢",
            ServiceState::Starting => "🟡",
            ServiceState::Crashed => "🔴",
            ServiceState::Stopped => "⚪",
        };
        let runtime = value.record.as_ref().map(|r| r.runtime.clone()).unwrap_or_else(|| "—".into());
        println!(
            "{symbol} {:<18} {:<12} {:>7}  {:<26} {}",
            value.service.id,
            value.state.label(),
            value.service.port,
            if value.state == ServiceState::Running { value.service.url() } else { "—".into() },
            runtime
        );
    }
    Ok(0)
}

pub fn logs(ctx: &ServiceContext, id: &str, lines: usize) -> Result<i32> {
    let service = ctx.find(id)?;
    let path = ctx.log_path(&service.id);
    let Ok(content) = fs::read_to_string(&path) else {
        println!("Sin logs para {}: todavía no se ha levantado.", service.id);
        return Ok(0);
    };
    let all: Vec<&str> = content.lines().collect();
    for line in all.iter().skip(all.len().saturating_sub(lines)) {
        println!("{line}");
    }
    Ok(0)
}
