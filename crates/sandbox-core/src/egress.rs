//! Salida de red con lista de permitidos: la única forma de que
//! `network.hosts` signifique algo.
//!
//! # Qué había antes
//!
//! `policy.network.hosts` se validaba y después la ignoraba todo el mundo. Con
//! `allowlist` la carga se quedaba con la red del host **entera**, así que la
//! lista no restringía nada. Una lista de hosts sin nada que la haga cumplir es
//! peor que no tenerla, porque invita a confiar.
//!
//! # Por qué un canal explícito y no NAT transparente
//!
//! Dar salida filtrada a un proceso dentro de un namespace de red propio, sin
//! privilegios, exige una pila de red en espacio de usuario —`slirp4netns`,
//! `pasta`— que hay que instalar en cada host, y **aun así** haría falta un
//! proxy para filtrar: esas herramientas dan conectividad, no política.
//!
//! Así que la salida se entrega como una **capacidad**, no como una propiedad
//! ambiental: la carga no tiene red, y lo único que atraviesa la frontera es un
//! socket Unix montado en su árbol. Por él pide `CONNECT host:puerto` y el
//! proxy decide. Lo que no está en la lista no se abre, y **todo intento queda
//! registrado**, permitido o no.
//!
//! La consecuencia hay que decirla entera: un cliente HTTP corriente no usa
//! esto solo, tiene que hablarle al socket a propósito. Es menos cómodo que un
//! proxy transparente y a cambio no hay forma de salir «sin querer».
//!
//! # Qué queda registrado
//!
//! Cada intento, con destino, veredicto y bytes movidos. Sin registro no hay
//! control, solo intención: un proxy que filtra y no cuenta lo que dejó pasar
//! no permite auditar nada después.

use serde::{Deserialize, Serialize};

/// Un intento de salida, permitido o no.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRecord {
    /// `host:puerto` tal y como lo pidió la carga.
    pub target: String,
    pub allowed: bool,
    /// Por qué se rechazó, o qué pasó después. Vacío cuando no hay nada que
    /// añadir.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reason: String,
    #[serde(default)]
    pub bytes_sent: u64,
    #[serde(default)]
    pub bytes_received: u64,
}

/// La lista de destinos que la política autoriza.
#[derive(Debug, Clone)]
pub struct Allowlist {
    hosts: Vec<String>,
}

impl Allowlist {
    pub fn new(hosts: &[String]) -> Self {
        Self { hosts: hosts.iter().map(|host| host.trim().to_ascii_lowercase()).collect() }
    }

    pub fn is_empty(&self) -> bool {
        self.hosts.is_empty()
    }

    /// ¿Autoriza este `host:puerto`?
    ///
    /// El puerto no se compara: la política nombra hosts, no servicios. Y no
    /// hay comodines. `*.ejemplo.com` parece cómodo y es exactamente cómo una
    /// lista de permitidos deja de serlo: basta un subdominio que el atacante
    /// controle para atravesarla. Si hace falta un subdominio, se escribe.
    pub fn permits(&self, target: &str) -> Result<(), String> {
        let Some((host, port)) = split_target(target) else {
            return Err(format!("destino mal formado: «{target}», se esperaba host:puerto"));
        };
        if port == 0 {
            return Err(format!("puerto inválido en «{target}»"));
        }
        if self.hosts.iter().any(|allowed| allowed == &host) {
            return Ok(());
        }
        Err(format!("«{host}» no está en la lista de la política"))
    }
}

/// Parte `host:puerto`. Devuelve `None` si no tiene esa forma.
fn split_target(target: &str) -> Option<(String, u16)> {
    let (host, port) = target.trim().rsplit_once(':')?;
    if host.is_empty() {
        return None;
    }
    Some((host.to_ascii_lowercase(), port.parse().ok()?))
}

/// Variable por la que la carga descubre su canal de salida.
///
/// Se llama socket y no proxy a propósito: no es un proxy HTTP al que apuntar
/// con `HTTP_PROXY`, es un socket Unix que hay que abrir.
pub const SOCKET_VARIABLE: &str = "SANDBOX_EGRESS_SOCKET";

/// Ruta del socket **dentro** del sandbox, fija como la del socket de servicio.
pub const SANDBOX_SOCKET_PATH: &str = "/workspace/egress/egress.sock";

/// Directorio que se monta para llevarlo.
pub const SANDBOX_SOCKET_DIR: &str = "/workspace/egress";

/// Respuestas del proxy. Se usa la sintaxis de `CONNECT` de HTTP porque es la
/// que ya conocen las herramientas, aunque el transporte sea un socket Unix.
pub const ESTABLISHED: &str = "HTTP/1.1 200 Connection Established\r\n\r\n";
pub const FORBIDDEN: &str = "HTTP/1.1 403 Forbidden\r\n\r\n";
pub const BAD_REQUEST: &str = "HTTP/1.1 400 Bad Request\r\n\r\n";

/// Extrae el destino de una línea `CONNECT host:puerto HTTP/1.1`.
pub fn parse_connect(line: &str) -> Option<String> {
    let mut parts = line.split_whitespace();
    if !parts.next()?.eq_ignore_ascii_case("CONNECT") {
        return None;
    }
    split_target(parts.next()?).map(|(host, port)| format!("{host}:{port}"))
}

#[cfg(unix)]
pub use proxy::Proxy;

#[cfg(unix)]
mod proxy {
    use super::{parse_connect, Allowlist, ConnectionRecord, BAD_REQUEST, ESTABLISHED, FORBIDDEN};
    use anyhow::{Context, Result};
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::{Shutdown, TcpStream},
        os::unix::net::{UnixListener, UnixStream},
        path::{Path, PathBuf},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        thread::{self, JoinHandle},
        time::Duration,
    };

    /// Techo de conexiones simultáneas. El proxy corre fuera del sandbox y por
    /// tanto fuera de sus límites: sin techo, la carga agota los descriptores
    /// del supervisor pidiendo conexiones que ni se le van a permitir.
    const MAX_CONNECTIONS: usize = 32;

    /// Cuánto se espera a que la carga mande su línea `CONNECT`. Una conexión
    /// que se abre y calla ocuparía un hilo para siempre.
    const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

    /// El proxy de salida. Vive **fuera** del sandbox, que es lo que le permite
    /// tener red mientras la carga no la tiene.
    pub struct Proxy {
        socket: PathBuf,
        stop: Arc<AtomicBool>,
        log: Arc<Mutex<Vec<ConnectionRecord>>>,
        worker: Option<JoinHandle<()>>,
    }

    impl Proxy {
        /// Levanta el proxy sobre un socket Unix del host.
        pub fn start(socket: &Path, allowlist: Allowlist) -> Result<Self> {
            if let Some(parent) = socket.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let _ = std::fs::remove_file(socket);
            let listener = UnixListener::bind(socket)
                .with_context(|| format!("No se pudo abrir el socket de salida en {}", socket.display()))?;
            // Sin bloqueo eterno en `accept`: así el hilo mira la señal de
            // parada aunque nadie llegue a conectarse nunca.
            listener.set_nonblocking(true)?;

            let stop = Arc::new(AtomicBool::new(false));
            let log: Arc<Mutex<Vec<ConnectionRecord>>> = Arc::new(Mutex::new(Vec::new()));
            let flag = Arc::clone(&stop);
            let records = Arc::clone(&log);

            let worker = thread::spawn(move || {
                let mut handles: Vec<JoinHandle<()>> = Vec::new();
                while !flag.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            handles.retain(|handle| !handle.is_finished());
                            if handles.len() >= MAX_CONNECTIONS {
                                let _ = stream.shutdown(Shutdown::Both);
                                continue;
                            }
                            let allowlist = allowlist.clone();
                            let records = Arc::clone(&records);
                            handles.push(thread::spawn(move || {
                                let record = serve(stream, &allowlist);
                                if let Ok(mut log) = records.lock() {
                                    log.push(record);
                                }
                            }));
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(20));
                        }
                        Err(_) => break,
                    }
                }
                for handle in handles {
                    let _ = handle.join();
                }
            });

            Ok(Self { socket: socket.to_path_buf(), stop, log, worker: Some(worker) })
        }

        /// Detiene el proxy y devuelve **todo** lo que se intentó, permitido o
        /// no. Es el registro que hace del filtro un control auditable.
        pub fn finish(mut self) -> Vec<ConnectionRecord> {
            self.stop.store(true, Ordering::SeqCst);
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
            let _ = std::fs::remove_file(&self.socket);
            self.log.lock().map(|log| log.clone()).unwrap_or_default()
        }
    }

    impl Drop for Proxy {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            let _ = std::fs::remove_file(&self.socket);
        }
    }

    fn refused(target: String, reason: String) -> ConnectionRecord {
        ConnectionRecord { target, allowed: false, reason, bytes_sent: 0, bytes_received: 0 }
    }

    /// Atiende una conexión: lee el `CONNECT`, decide y, si procede, empalma.
    fn serve(stream: UnixStream, allowlist: &Allowlist) -> ConnectionRecord {
        let _ = stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT));
        let Ok(clone) = stream.try_clone() else {
            return refused(String::new(), "no se pudo duplicar el socket".into());
        };
        let mut reader = BufReader::new(clone);
        let mut writer = stream;

        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
            let _ = writer.write_all(BAD_REQUEST.as_bytes());
            return refused(String::new(), "petición vacía o ilegible".into());
        }

        let Some(target) = parse_connect(&line) else {
            let _ = writer.write_all(BAD_REQUEST.as_bytes());
            return refused(line.trim().to_string(), "no es una línea CONNECT host:puerto".into());
        };

        // Las cabeceras hasta la línea en blanco se consumen y se descartan,
        // como hace cualquier proxy. Si se dejaran en el búfer, el salto final
        // del CONNECT viajaría al destino como si fuera el primer byte de la
        // carga — y eso no es reenviar, es corromper.
        loop {
            let mut header = String::new();
            match reader.read_line(&mut header) {
                Ok(0) => break,
                Ok(_) if header.trim().is_empty() => break,
                Ok(_) => continue,
                Err(_) => break,
            }
        }

        if let Err(reason) = allowlist.permits(&target) {
            // Se responde y se cierra. La carga se entera ahora, y el intento
            // queda registrado igual que si hubiera salido.
            let _ = writer.write_all(FORBIDDEN.as_bytes());
            let _ = writer.shutdown(Shutdown::Both);
            return refused(target, reason);
        }

        let upstream = match TcpStream::connect(&target) {
            Ok(upstream) => upstream,
            Err(error) => {
                let _ = writer.write_all(BAD_REQUEST.as_bytes());
                // Permitido por la política: que el destino no responda es otra
                // cosa, y la evidencia tiene que distinguirlas.
                return ConnectionRecord {
                    target,
                    allowed: true,
                    reason: format!("la política lo permite pero no se pudo conectar: {error}"),
                    bytes_sent: 0,
                    bytes_received: 0,
                };
            }
        };

        if writer.write_all(ESTABLISHED.as_bytes()).is_err() {
            return ConnectionRecord {
                target,
                allowed: true,
                reason: "la carga cerró antes de recibir la confirmación".into(),
                bytes_sent: 0,
                bytes_received: 0,
            };
        }
        let _ = writer.set_read_timeout(None);
        let (sent, received) = splice(reader, writer, upstream);
        ConnectionRecord { target, allowed: true, reason: String::new(), bytes_sent: sent, bytes_received: received }
    }

    /// Copia en los dos sentidos y cuenta los bytes. Contar no es adorno: un
    /// registro que dice «se permitió» sin decir cuánto salió no sirve para
    /// investigar nada después.
    fn splice(mut from_load: BufReader<UnixStream>, mut to_load: UnixStream, upstream: TcpStream) -> (u64, u64) {
        let Ok(mut to_target) = upstream.try_clone() else { return (0, 0) };
        let mut from_target = upstream;

        let outbound = thread::spawn(move || {
            let mut total = 0_u64;
            let mut buffer = [0_u8; 8192];
            loop {
                match from_load.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(count) => {
                        if to_target.write_all(&buffer[..count]).is_err() {
                            break;
                        }
                        total += count as u64;
                    }
                }
            }
            let _ = to_target.shutdown(Shutdown::Write);
            total
        });

        let mut received = 0_u64;
        let mut buffer = [0_u8; 8192];
        loop {
            match from_target.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    if to_load.write_all(&buffer[..count]).is_err() {
                        break;
                    }
                    received += count as u64;
                }
            }
        }
        let _ = to_load.shutdown(Shutdown::Write);
        (outbound.join().unwrap_or(0), received)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list() -> Allowlist {
        Allowlist::new(&["api.anthropic.com".to_string(), "127.0.0.1".to_string()])
    }

    #[test]
    fn allows_exactly_what_the_policy_names() {
        assert!(list().permits("api.anthropic.com:443").is_ok());
        assert!(list().permits("127.0.0.1:8080").is_ok());
    }

    #[test]
    fn the_comparison_ignores_case_because_dns_does() {
        assert!(list().permits("API.Anthropic.COM:443").is_ok());
    }

    #[test]
    fn rejects_anything_not_on_the_list() {
        let error = list().permits("evil.example:443").expect_err("no está en la lista");
        assert!(error.contains("evil.example"), "{error}");
    }

    /// El error clásico de una lista de permitidos: el comodín. Un subdominio
    /// que el atacante controle atraviesa `*.anthropic.com` sin despeinarse.
    #[test]
    fn there_are_no_wildcards() {
        assert!(list().permits("malo.api.anthropic.com:443").is_err());
        assert!(list().permits("api.anthropic.com.evil.example:443").is_err());
        assert!(list().permits("notapi.anthropic.com:443").is_err());
    }

    #[test]
    fn a_malformed_target_is_rejected_not_guessed() {
        for target in ["api.anthropic.com", "", ":443", "api.anthropic.com:0", "api.anthropic.com:abc"] {
            assert!(list().permits(target).is_err(), "«{target}» no debería aceptarse");
        }
    }

    #[test]
    fn an_empty_list_permits_nothing() {
        let empty = Allowlist::new(&[]);
        assert!(empty.is_empty());
        assert!(empty.permits("127.0.0.1:80").is_err());
    }

    #[test]
    fn reads_the_target_from_a_connect_line() {
        assert_eq!(parse_connect("CONNECT api.anthropic.com:443 HTTP/1.1"), Some("api.anthropic.com:443".into()));
        assert_eq!(parse_connect("connect 127.0.0.1:80 HTTP/1.1"), Some("127.0.0.1:80".into()));
        assert_eq!(parse_connect("GET / HTTP/1.1"), None);
        assert_eq!(parse_connect("CONNECT sinpuerto HTTP/1.1"), None);
    }

    // ── El proxy de verdad ────────────────────────────────────────────────
    //
    // Las pruebas de arriba comprueban la decisión. Estas comprueban que la
    // decisión se hace cumplir sobre sockets reales, que es otra cosa.

    #[cfg(unix)]
    mod real {
        use super::super::*;
        use std::{
            io::{BufRead, BufReader, Read, Write},
            net::{TcpListener, TcpStream},
            os::unix::net::UnixStream,
            thread,
        };

        /// Un servidor TCP local que devuelve una respuesta fija. Hace de
        /// «internet» para la prueba: sin salir a ninguna red de verdad, que
        /// convertiría una prueba en una lotería.
        fn destination() -> (String, TcpListener) {
            let listener = TcpListener::bind("127.0.0.1:0").expect("destino de prueba");
            let address = listener.local_addr().expect("dirección").to_string();
            let accepting = listener.try_clone().expect("clon");
            thread::spawn(move || {
                for stream in accepting.incoming() {
                    let Ok(mut stream) = stream else { continue };
                    thread::spawn(move || {
                        let mut received = Vec::new();
                        let mut buffer = [0_u8; 512];
                        while let Ok(count) = stream.read(&mut buffer) {
                            if count == 0 {
                                break;
                            }
                            received.extend_from_slice(&buffer[..count]);
                        }
                        let _ = stream.write_all(b"destino:");
                        let _ = stream.write_all(&received);
                    });
                }
            });
            (address, listener)
        }

        fn socket_path(name: &str) -> std::path::PathBuf {
            let path = std::env::temp_dir()
                .join(format!("sandbox-labs-egress-{name}-{}", std::process::id()))
                .join("egress.sock");
            let _ = std::fs::remove_file(&path);
            path
        }

        /// Manda un CONNECT por el socket y devuelve (respuesta, conexión).
        fn connect(socket: &std::path::Path, target: &str) -> (String, UnixStream) {
            let stream = UnixStream::connect(socket).expect("conexión al proxy");
            let mut writer = stream.try_clone().expect("clon");
            writer
                .write_all(
                    format!(
                        "CONNECT {target} HTTP/1.1

"
                    )
                    .as_bytes(),
                )
                .expect("CONNECT");
            let mut reader = BufReader::new(stream.try_clone().expect("clon"));
            let mut line = String::new();
            reader.read_line(&mut line).expect("respuesta");
            (line, stream)
        }

        #[test]
        fn an_allowed_destination_goes_through_and_is_counted() {
            let (address, _keep) = destination();
            let host = address.rsplit_once(':').expect("puerto").0.to_string();
            let socket = socket_path("permitido");
            let proxy = Proxy::start(&socket, Allowlist::new(&[host])).expect("proxy");

            let (status, stream) = connect(&socket, &address);
            assert!(status.contains("200"), "se esperaba 200 y llegó: {status}");

            let mut writer = stream.try_clone().expect("clon");
            writer.write_all(b"hola").expect("envío");
            writer.shutdown(std::net::Shutdown::Write).expect("cierre");
            let mut answer = Vec::new();
            BufReader::new(stream).read_to_end(&mut answer).expect("lectura");
            assert_eq!(answer, b"destino:hola", "la respuesta viene del destino, no del proxy");

            let log = proxy.finish();
            assert_eq!(log.len(), 1);
            assert!(log[0].allowed);
            assert_eq!(log[0].bytes_sent, 4, "cuatro bytes salieron y el registro tiene que decirlo");
            assert_eq!(log[0].bytes_received, 12);
        }

        #[test]
        fn a_destination_outside_the_list_is_refused_and_still_logged() {
            let (address, _keep) = destination();
            let socket = socket_path("denegado");
            // La lista nombra otra cosa: el destino real no está.
            let proxy = Proxy::start(&socket, Allowlist::new(&["solo.esto".to_string()])).expect("proxy");

            let (status, _stream) = connect(&socket, &address);
            assert!(status.contains("403"), "se esperaba 403 y llegó: {status}");

            // Y el destino no vio a nadie: el proxy ni siquiera abrió la conexión.
            assert!(
                TcpStream::connect(&address).is_ok(),
                "el destino sigue vivo; lo que no hubo es conexión desde el proxy"
            );

            let log = proxy.finish();
            assert_eq!(log.len(), 1, "un intento denegado se registra igual que uno permitido");
            assert!(!log[0].allowed);
            assert!(log[0].reason.contains("no está en la lista"), "{}", log[0].reason);
            assert_eq!(log[0].bytes_sent, 0);
        }

        #[test]
        fn a_request_that_is_not_a_connect_gets_nothing() {
            let socket = socket_path("basura");
            let proxy = Proxy::start(&socket, Allowlist::new(&["127.0.0.1".to_string()])).expect("proxy");

            let stream = UnixStream::connect(&socket).expect("conexión");
            let mut writer = stream.try_clone().expect("clon");
            writer
                .write_all(
                    b"GET /etc/passwd HTTP/1.1

",
                )
                .expect("envío");
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).expect("respuesta");
            assert!(line.contains("400"), "{line}");

            let log = proxy.finish();
            assert_eq!(log.len(), 1);
            assert!(!log[0].allowed);
        }

        #[test]
        fn the_socket_disappears_when_the_proxy_stops() {
            // Un socket huérfano dejaría una puerta abierta con nadie detrás, y
            // la siguiente ejecución no podría enlazar.
            let socket = socket_path("limpieza");
            let proxy = Proxy::start(&socket, Allowlist::new(&[])).expect("proxy");
            assert!(socket.exists());
            proxy.finish();
            assert!(!socket.exists(), "el teardown tiene que retirar el socket");
        }
    }
}
