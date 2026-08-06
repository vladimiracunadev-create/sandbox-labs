//! Subcomando `service`: levanta, lista, inspecciona y baja sandboxes.
//!
//! El resto del CLI ejecuta cargas que terminan. Esto mantiene sandboxes
//! **en marcha**: un servicio dentro de una jaula, publicando un puerto en el
//! loopback del host, que se puede abrir en el navegador y bajar después.

use anyhow::{bail, Context, Result};
use sandbox_core::{
    service::{process_start_ticks, same_process, Service, ServiceRecord, ServiceState},
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

/// Crea el directorio si no existe, tolerando que ya exista.
///
/// `create_dir_all` es idempotente en un filesystem normal, pero sobre DrvFs
/// —el montaje de un disco de Windows dentro de WSL— puede devolver EEXIST
/// igualmente. Tratar eso como error impedía levantar cualquier servicio desde
/// un repositorio alojado en `/mnt/c`.
fn ensure_dir(path: &Path) -> Result<()> {
    match fs::create_dir_all(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).with_context(|| format!("No se pudo crear {}", path.display())),
    }
    if path.is_dir() {
        return Ok(());
    }
    // EEXIST sin que el directorio exista: la caché de DrvFs quedó
    // desincronizada, típicamente porque alguien borró la ruta desde Windows
    // mientras WSL la tenía vista. Un reintento la refresca; si aún así no
    // está, se dice qué pasa en vez de fallar más adelante con un ENOENT
    // opaco al abrir el log.
    std::thread::sleep(Duration::from_millis(150));
    fs::create_dir_all(path).ok();
    if path.is_dir() {
        return Ok(());
    }
    bail!(
        "No se pudo crear {}: el sistema de archivos dice que ya existe pero no está.          Si el repositorio vive en /mnt/c y borraste el directorio desde Windows,          cierra WSL (`wsl --shutdown`) y vuelve a intentarlo.",
        path.display()
    )
}

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
        let directory = self.root.join("cases");
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

    /// Directorio de sockets en el host.
    ///
    /// **No** va bajo `.sandbox-data`: si el repositorio vive en `/mnt/c`, ese
    /// directorio está en DrvFs, y sobre DrvFs no se pueden crear sockets Unix
    /// —el servicio moría al enlazar sin decir por qué—. Los sockets viven
    /// donde les corresponde: el directorio de runtime del sistema.
    pub fn socket_dir(&self) -> PathBuf {
        std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("sandbox-labs")
    }

    /// Socket del servicio en el host. El sandbox lo ve en una ruta fija de su
    /// propio árbol; el bind mount los conecta.
    pub fn socket_path(&self, id: &str) -> PathBuf {
        self.socket_dir().join(format!("{id}.sock"))
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
    let ready = endpoint_responds(ctx, &service);
    let state = match &record {
        None => ServiceState::Stopped,
        Some(value) if !same_process(value.pid, value.start_ticks) => ServiceState::Crashed,
        Some(_) if ready => ServiceState::Running,
        Some(_) => ServiceState::Starting,
    };
    Observed { service, record, state }
}

/// ¿Responde el servicio por su transporte?
///
/// Un servicio con `network: none` no tiene puerto que sondear: se comprueba
/// que el socket exista y acepte una conexión.
fn endpoint_responds(ctx: &ServiceContext, service: &Service) -> bool {
    if service.is_socket() {
        #[cfg(unix)]
        {
            return std::os::unix::net::UnixStream::connect(ctx.socket_path(&service.id)).is_ok();
        }
        #[cfg(not(unix))]
        {
            return false;
        }
    }
    port_responds(service.port)
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
/// Secretos que de verdad entran al sandbox.
///
/// Intersección de tres conjuntos: lo que el servicio pide, lo que la política
/// permite y lo que existe en el host. Los tres tienen que coincidir. Un
/// secreto que el servicio pide pero la política no declara **no entra**, y eso
/// no es un fallo: es la política haciendo su trabajo.
fn resolved_secrets(service: &Service, policy: &Policy) -> (Vec<(String, String)>, Vec<String>) {
    let mut injected = Vec::new();
    let mut refused = Vec::new();
    for name in &service.secrets {
        if !policy.process.allowed_environment.contains(name) {
            refused.push(format!("{name} (la política {} no lo declara)", policy.id));
            continue;
        }
        match std::env::var(name) {
            Ok(value) if !value.is_empty() => injected.push((name.clone(), value)),
            _ => refused.push(format!("{name} (ausente en el host)")),
        }
    }
    (injected, refused)
}

/// Ruta del socket **dentro** del sandbox. Fija a propósito: el servicio no
/// necesita saber dónde vive en el host.
const SANDBOX_SOCKET_DIR: &str = "/workspace/socket";

fn sandbox_command(
    runtime: RuntimeKind,
    service: &Service,
    policy: &Policy,
    secrets: &[(String, String)],
    socket_dir: &Path,
) -> (String, Vec<String>) {
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
            // La red se conserva a propósito cuando la política lo dice con
            // todas sus letras (`unrestricted`): un servicio que publica un
            // puerto TCP no puede estar en un namespace de red propio. La
            // contención sigue viva en filesystem, PIDs, capabilities y
            // entorno — y la tarjeta del panel lo dice.
            if policy.network.isolates_host_network() {
                args.push("--unshare-net".into());
            }
            args.extend(["--ro-bind".into(), workdir.clone(), "/workspace/app".into()]);
            if service.is_socket() {
                // El socket entra por el filesystem, que es la única puerta que
                // le queda a un sandbox sin red. Montaje de escritura: el
                // servicio tiene que poder crear el fichero del socket.
                args.extend(["--bind".into(), socket_dir.display().to_string(), SANDBOX_SOCKET_DIR.into()]);
            }
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
            for (name, value) in secrets {
                args.extend(["--setenv".into(), name.clone(), value.clone()]);
            }
            if service.is_socket() {
                args.extend([
                    "--setenv".into(),
                    "SANDBOX_SOCKET".into(),
                    format!("{SANDBOX_SOCKET_DIR}/{}.sock", service.id),
                ]);
            }
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
            if policy.network.isolates_host_network() {
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

/// Falla en cerrado cuando el transporte del servicio y su política de red no
/// pueden ser ciertos a la vez.
///
/// Un servicio con `transport: tcp` publica un puerto en el loopback del host.
/// Con `network.mode` en `none` o `loopback` ese puerto nace **dentro** del
/// namespace de red del sandbox y nadie fuera lo alcanza: el servicio arranca,
/// el proceso vive, y el sondeo de salud espera veinte segundos a algo que
/// nunca va a responder. Antes eso se veía como «el servicio no levanta»; ahora
/// se dice qué está mal y cuáles son las dos salidas.
fn check_transport_matches_network(service: &Service, policy: &Policy) -> Result<()> {
    if service.is_socket() || policy.network.allows_published_port() {
        return Ok(());
    }
    bail!(
        "{}: la política {} pide network.mode «{}», que crea un namespace de red propio, \
         pero el servicio declara transport «tcp» y publica el puerto {}. Ese puerto no sería \
         alcanzable desde el host.\n   → o el servicio pasa a transport «unix-socket», \
         que entra por el filesystem y no necesita red;\n   → o la política declara \
         network.mode «unrestricted», y entonces el control `network` no se cuenta como efectivo.",
        service.id,
        policy.id,
        policy.network.mode,
        service.port
    )
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
    if !service.is_socket() && port_responds(service.port) {
        bail!("El puerto {} ya está ocupado por otro proceso. Bájalo o cambia el puerto del servicio.", service.port);
    }
    if service.is_socket() {
        // Un socket huérfano de una ejecución anterior impediría enlazar.
        ensure_dir(&ctx.socket_dir())?;
        let _ = fs::remove_file(ctx.socket_path(&service.id));
    }

    let policy = Policy::load(ctx.root.join("policies").join(format!("{}.json", service.policy)))?;
    check_transport_matches_network(&service, &policy)?;
    let runtime = pick_runtime(&service)?;
    let (secrets, refused) = resolved_secrets(&service, &policy);
    let socket_dir = ctx.socket_dir();
    let (program, args) = sandbox_command(runtime, &service, &policy, &secrets, &socket_dir);

    let log_path = ctx.log_path(&service.id);
    ensure_dir(log_path.parent().expect("directorio de servicios"))?;
    let log = fs::File::create(&log_path)?;
    let log_err = log.try_clone()?;

    println!("▶ Levantando {} con {} · política {}", service.id, runtime, policy.id);
    if !secrets.is_empty() {
        // Se nombran, nunca se imprime el valor.
        println!("   secretos inyectados: {}", secrets.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", "));
    }
    for reason in &refused {
        println!("   ⚠ sin inyectar: {reason}");
    }
    if !refused.is_empty() && secrets.len() < service.secrets.len() {
        println!("   → el servicio arrancará en modo plan: mostrará qué haría, sin hacerlo.");
    }

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
        for (name, value) in &secrets {
            command.env(name, value);
        }
        if service.is_socket() {
            // unshare no monta una raíz nueva: el sandbox ve el árbol del host,
            // así que la ruta real del socket vale tal cual.
            command.env("SANDBOX_SOCKET", ctx.socket_path(&service.id));
        }
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
        start_ticks: process_start_ticks(child.id()),
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
        if endpoint_responds(ctx, &service) {
            let endpoint = if service.is_socket() {
                format!("unix:{}", ctx.socket_path(&service.id).display())
            } else {
                service.url()
            };
            println!("✅ {} responde en {}", service.id, endpoint);
            println!("   contención efectiva: {}", record.effective_controls.join(", "));
            println!("   logs: {}", log_path.display());
            return Ok(0);
        }
        if !same_process(record.pid, record.start_ticks) {
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

    if same_process(record.pid, record.start_ticks) {
        terminate(record.pid);
        // Se le da margen a terminar por las buenas antes de insistir: un
        // servicio que cierra bien deja el puerto libre de inmediato.
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(5) && same_process(record.pid, record.start_ticks) {
            std::thread::sleep(Duration::from_millis(200));
        }
        if same_process(record.pid, record.start_ticks) {
            kill(record.pid);
        }
    } else {
        // El PID ya no es de este servicio. Se limpia el registro y no se
        // señaliza a nadie: matar a quien heredó el número sería mucho peor
        // que dejar un registro obsoleto.
        println!("· el registro de {} estaba obsoleto (PID reutilizado o proceso muerto)", service.id);
    }

    ServiceRecord::remove(&ctx.data_root, &service.id);
    if service.is_socket() {
        let _ = fs::remove_file(ctx.socket_path(&service.id));
    }
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
    // El `--` no es opcional: sin él, `/bin/kill -TERM -1234` puede leer el
    // objetivo negativo como una opción, y en el peor caso acabar señalizando
    // a procesos que no son el servicio. Aquí ya se llevó por delante la shell
    // que ejecutaba las pruebas.
    // stderr se descarta: matar el grupo suele llevarse también al proceso, y
    // el segundo intento imprimiría entonces un «No such process» que no es un
    // problema sino la señal de que ya funcionó.
    for target in [format!("-{pid}"), pid.to_string()] {
        let _ =
            Command::new("kill").arg(signal).arg("--").arg(target).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
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
                    "transport": value.service.transport,
                    // Un servicio por socket no tiene URL que abrir en el
                    // navegador: se dice explícitamente en vez de dar una que
                    // no responde.
                    "url": if value.service.is_socket() { serde_json::Value::Null } else { value.service.url().into() },
                    "socket": if value.service.is_socket() { ctx.socket_path(&value.service.id).display().to_string().into() } else { serde_json::Value::Null },
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
        println!("No hay casos registrados en cases/.");
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
            match (value.state, value.service.is_socket()) {
                (ServiceState::Running, true) => format!("unix:{}.sock", value.service.id),
                (ServiceState::Running, false) => value.service.url(),
                _ => "—".into(),
            },
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

/// Habla con un servicio, por TCP o por socket Unix indistintamente.
///
/// Existe porque un sandbox con `network: none` no se puede alcanzar con
/// `curl http://127.0.0.1:...`: no hay pila de red. Sin este comando, un
/// custodio de claves correctamente aislado sería inoperable desde la terminal.
pub fn call(ctx: &ServiceContext, id: &str, method: &str, path: &str, body: Option<String>) -> Result<i32> {
    use std::io::{Read, Write};

    let service = ctx.find(id)?;
    let payload = body.unwrap_or_default();
    // CRLF explícito: HTTP lo exige y un salto de línea suelto haría que
    // el servidor esperase indefinidamente la cabecera que falta.
    const CRLF: &str = "\r\n";
    let head = [
        format!("{method} {path} HTTP/1.1"),
        "Host: sandbox".into(),
        "Connection: close".into(),
        "content-type: application/json".into(),
        format!("content-length: {}", payload.len()),
    ]
    .join(CRLF);
    let request = format!("{head}{CRLF}{CRLF}{payload}");

    let mut stream: Box<dyn ReadWrite> = if service.is_socket() {
        #[cfg(unix)]
        {
            let path = ctx.socket_path(&service.id);
            Box::new(std::os::unix::net::UnixStream::connect(&path).with_context(|| {
                format!("No se pudo hablar con {} en {}. ¿Está levantado?", service.id, path.display())
            })?)
        }
        #[cfg(not(unix))]
        {
            bail!("los sockets Unix solo están disponibles en Linux")
        }
    } else {
        Box::new(
            std::net::TcpStream::connect(("127.0.0.1", service.port))
                .with_context(|| format!("No se pudo hablar con {} en el puerto {}", service.id, service.port))?,
        )
    };

    stream.write_all(request.as_bytes())?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    // Se imprime solo el cuerpo: la cabecera HTTP no aporta nada aquí.
    let separator = format!("{CRLF}{CRLF}");
    match response.split_once(separator.as_str()) {
        Some((_, body)) => println!("{}", body.trim()),
        None => println!("{response}"),
    }
    Ok(0)
}

/// Los dos transportes se usan igual; este alias evita duplicar `call`.
trait ReadWrite: std::io::Read + std::io::Write {}
impl<T: std::io::Read + std::io::Write> ReadWrite for T {}

#[cfg(test)]
mod transport_tests {
    use super::*;

    fn service(transport: &str) -> Service {
        serde_json::from_value(serde_json::json!({
            "id": "demo", "name": "Demo", "category": "platform",
            "description": "d", "teaches": "t", "port": 8899, "kind": "python",
            "entrypoint": "app.py", "command": "python3", "policy": "p",
            "runtimes": ["bwrap"], "healthPath": "/health", "transport": transport
        }))
        .expect("servicio de prueba válido")
    }

    fn policy(mode: &str) -> Policy {
        serde_json::from_value(serde_json::json!({
            "id": "p",
            "enforcement": { "mode": "best-effort", "requiredControls": [] },
            "filesystem": { "root": "ephemeral", "readOnly": [], "writable": [], "maxWorkspaceMb": 64, "followSymlinks": false },
            "network": { "mode": mode, "hosts": [], "dns": "disabled" },
            "resources": { "cpu": 1.0, "memoryMb": 128, "processes": 8, "timeoutSeconds": 10, "openFiles": 32, "outputBytes": 4096 },
            "process": { "capabilities": [], "environment": {}, "allowedEnvironment": [], "user": 65534, "group": 65534 }
        }))
        .expect("política de prueba válida")
    }

    #[test]
    fn a_tcp_service_cannot_live_in_its_own_network_namespace() {
        for mode in ["none", "loopback"] {
            let error = check_transport_matches_network(&service("tcp"), &policy(mode))
                .expect_err("un puerto dentro del namespace no es alcanzable desde el host");
            let text = error.to_string();
            assert!(text.contains("unix-socket"), "el error debe decir la salida: {text}");
            assert!(text.contains(mode), "el error debe nombrar el modo que lo provoca: {text}");
        }
    }

    #[test]
    fn a_socket_service_can_keep_the_network_closed() {
        // Es justo el patrón del custodio de claves: sin pila de red hacia
        // fuera, y alcanzable por el filesystem.
        for mode in ["none", "loopback"] {
            check_transport_matches_network(&service("unix-socket"), &policy(mode))
                .expect("un socket Unix no necesita red");
        }
    }

    #[test]
    fn a_tcp_service_runs_when_the_policy_admits_it_keeps_the_host_network() {
        check_transport_matches_network(&service("tcp"), &policy("unrestricted"))
            .expect("con la red del host el puerto sí se publica");
    }
}
