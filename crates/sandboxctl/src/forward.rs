//! Reenviador TCP → socket Unix: la puerta de un servicio que corre en su
//! propio namespace de red.
//!
//! # Qué problema resuelve
//!
//! Un servicio con `network.mode: loopback` corre en un namespace de red
//! propio. Si escucha en un puerto TCP, ese puerto nace **dentro** del sandbox
//! y nadie fuera lo alcanza. Por eso hasta ahora todos los servicios del
//! catálogo usaban `unrestricted` —la red del host entera— solo para poder
//! publicar un puerto.
//!
//! La salida es la que ya usaba el custodio de claves: el servicio escucha en un
//! **socket Unix**, que entra por el sistema de archivos y no necesita pila de
//! red. Lo que faltaba era el otro extremo. Este módulo escucha en el loopback
//! del **host** y empalma cada conexión con ese socket:
//!
//! ```text
//!   navegador                    host                     sandbox (netns propio)
//!   127.0.0.1:8803  ──TCP──▶  reenviador  ──socket Unix──▶  servicio
//! ```
//!
//! El servicio no cambia de protocolo: sigue hablando HTTP. Solo cambia el
//! transporte por debajo, y a cambio pierde toda la red.
//!
//! # Por qué vive fuera del sandbox
//!
//! El reenviador es parte del supervisor, no de la carga. Está del lado de
//! fuera de la frontera a propósito: si viviera dentro necesitaría red, que es
//! justo lo que se le está quitando.

use anyhow::{Context, Result};
use std::net::TcpListener;

/// Reserva el puerto en el loopback del host.
///
/// Separado de `serve` para que las pruebas puedan pedir el puerto 0 —efímero,
/// asignado por el kernel— y descubrir cuál tocó. Un puerto fijo en una prueba
/// es una prueba que falla cuando alguien más lo está usando.
///
/// Solo `127.0.0.1`: publicar en `0.0.0.0` expondría a la red local un servicio
/// cuyo propósito es ejecutar código que nadie ha revisado.
pub fn bind(port: u16) -> Result<TcpListener> {
    TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .with_context(|| format!("No se pudo reservar 127.0.0.1:{port} para el reenviador"))
}

/// Máximo de conexiones simultáneas empalmadas.
///
/// Cada una cuesta un hilo y un descriptor. Sin techo, quien alcance el puerto
/// puede agotar la tabla de descriptores del supervisor —que corre FUERA del
/// sandbox y por tanto fuera de los límites del cgroup—. Las que sobran se
/// cierran de inmediato en vez de encolarse: encolar traslada el problema.
pub const MAX_CONNECTIONS: usize = 64;

#[cfg(unix)]
pub use unix::serve;

#[cfg(unix)]
mod unix {
    use super::MAX_CONNECTIONS;
    use anyhow::Result;
    use std::{
        io,
        net::{Shutdown, TcpListener, TcpStream},
        os::unix::net::UnixStream,
        path::{Path, PathBuf},
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        thread,
    };

    /// Acepta conexiones hasta que el proceso muera. No vuelve en operación
    /// normal.
    pub fn serve(listener: TcpListener, socket: PathBuf) -> Result<()> {
        let live = Arc::new(AtomicUsize::new(0));
        for incoming in listener.incoming() {
            let Ok(tcp) = incoming else { continue };
            if live.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
                // Cerrar sin leer es la respuesta honesta: el cliente se entera
                // ahora en vez de esperar a un servicio que no le va a atender.
                let _ = tcp.shutdown(Shutdown::Both);
                continue;
            }
            live.fetch_add(1, Ordering::SeqCst);
            let socket = socket.clone();
            let live = Arc::clone(&live);
            thread::spawn(move || {
                let _ = bridge(tcp, &socket);
                live.fetch_sub(1, Ordering::SeqCst);
            });
        }
        Ok(())
    }

    /// Empalma una conexión TCP con el socket del sandbox y copia en los dos
    /// sentidos hasta que alguno cierre.
    ///
    /// El `shutdown` de escritura al terminar cada sentido no es cosmético: sin
    /// él, un cliente HTTP que espera el fin del cuerpo se queda colgado porque
    /// nunca ve el EOF.
    fn bridge(tcp: TcpStream, socket: &Path) -> io::Result<()> {
        let sandbox = UnixStream::connect(socket)?;
        let mut from_client = tcp.try_clone()?;
        let mut to_client = tcp;
        let mut from_sandbox = sandbox.try_clone()?;
        let mut to_sandbox = sandbox;

        let upstream = thread::spawn(move || {
            let _ = io::copy(&mut from_client, &mut to_sandbox);
            let _ = to_sandbox.shutdown(Shutdown::Write);
        });
        let _ = io::copy(&mut from_sandbox, &mut to_client);
        let _ = to_client.shutdown(Shutdown::Write);
        let _ = upstream.join();
        Ok(())
    }
}

#[cfg(not(unix))]
pub fn serve(_listener: TcpListener, _socket: std::path::PathBuf) -> Result<()> {
    anyhow::bail!("El reenviador necesita sockets Unix: solo Linux o WSL2")
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::Shutdown,
        net::TcpStream,
        os::unix::net::UnixListener,
        path::PathBuf,
        thread,
        time::Duration,
    };

    /// Levanta un servidor Unix que devuelve lo que recibe, más un prefijo que
    /// demuestra que la respuesta viene del otro lado y no del reenviador.
    fn echo_server(path: PathBuf) -> UnixListener {
        let listener = UnixListener::bind(&path).expect("socket de prueba");
        let accepting = listener.try_clone().expect("clon del listener");
        thread::spawn(move || {
            for stream in accepting.incoming() {
                let Ok(mut stream) = stream else { continue };
                thread::spawn(move || {
                    let mut buffer = [0_u8; 1024];
                    while let Ok(count) = stream.read(&mut buffer) {
                        if count == 0 {
                            break;
                        }
                        let mut answer = b"sandbox:".to_vec();
                        answer.extend_from_slice(&buffer[..count]);
                        if stream.write_all(&answer).is_err() {
                            break;
                        }
                    }
                });
            }
        });
        listener
    }

    fn temporary_socket(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("sandbox-labs-test-{name}-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn carries_bytes_between_the_host_port_and_the_sandbox_socket() {
        let socket = temporary_socket("bridge");
        let _server = echo_server(socket.clone());
        // Puerto 0: lo elige el kernel, así que la prueba no choca con nada.
        let listener = bind(0).expect("puerto efímero");
        let port = listener.local_addr().expect("dirección").port();
        thread::spawn(move || {
            let _ = serve(listener, socket);
        });

        let mut client = TcpStream::connect(("127.0.0.1", port)).expect("conexión al reenviador");
        client.set_read_timeout(Some(Duration::from_secs(5))).expect("timeout");
        client.write_all(b"hola").expect("envío");
        let mut answer = [0_u8; 64];
        let count = client.read(&mut answer).expect("respuesta");
        assert_eq!(&answer[..count], b"sandbox:hola", "la respuesta tiene que venir del socket del sandbox");
    }

    #[test]
    fn the_client_sees_the_end_of_the_answer() {
        // Sin `shutdown` de escritura al cerrar cada sentido, un cliente que lee
        // hasta EOF se cuelga. Es el fallo que convierte «funciona» en «el
        // navegador se queda cargando para siempre».
        let socket = temporary_socket("eof");
        let _server = echo_server(socket.clone());
        let listener = bind(0).expect("puerto efímero");
        let port = listener.local_addr().expect("dirección").port();
        thread::spawn(move || {
            let _ = serve(listener, socket);
        });

        let mut client = TcpStream::connect(("127.0.0.1", port)).expect("conexión");
        client.set_read_timeout(Some(Duration::from_secs(5))).expect("timeout");
        client.write_all(b"adios").expect("envío");
        client.shutdown(Shutdown::Write).expect("cierre de escritura");
        let mut answer = Vec::new();
        client.read_to_end(&mut answer).expect("lectura hasta EOF");
        assert_eq!(answer, b"sandbox:adios", "el EOF tiene que propagarse en los dos sentidos");
    }

    #[test]
    fn a_connection_without_anyone_listening_fails_without_taking_down_the_forwarder() {
        // El sandbox puede no haber enlazado su socket todavía. Eso cierra ESA
        // conexión, no el reenviador: la siguiente tiene que poder atenderse.
        let socket = temporary_socket("ausente");
        let listener = bind(0).expect("puerto efímero");
        let port = listener.local_addr().expect("dirección").port();
        thread::spawn(move || {
            let _ = serve(listener, socket.clone());
        });

        let mut early = TcpStream::connect(("127.0.0.1", port)).expect("conexión temprana");
        early.set_read_timeout(Some(Duration::from_secs(5))).expect("timeout");
        let mut discarded = Vec::new();
        // El socket del sandbox no existe: la conexión muere sin datos.
        let _ = early.read_to_end(&mut discarded);
        assert!(discarded.is_empty());

        // Y el reenviador sigue aceptando.
        assert!(TcpStream::connect(("127.0.0.1", port)).is_ok(), "una conexión fallida no puede tumbar el reenviador");
    }
}
