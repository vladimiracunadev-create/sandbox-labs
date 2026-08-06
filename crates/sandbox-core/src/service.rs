//! Servicios sandboxeados: procesos **largos** que viven dentro de un sandbox.
//!
//! Es la diferencia entre este módulo y el resto del sistema. Una carga
//! (`Workload`) se ejecuta, imprime y termina. Un servicio se **levanta**,
//! publica un puerto en el loopback del host, se puede abrir en el navegador,
//! y se **baja** cuando ya no hace falta — igual que un contenedor.
//!
//! Aislar un proceso efímero es fácil: nadie tiene que hablar con él. Aislar un
//! servicio obliga a decidir qué frontera se deja abierta a propósito, y esa
//! decisión es justo lo que este módulo hace explícita.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Manifiesto de un servicio registrado.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Service {
    #[serde(default, rename = "$schema")]
    pub schema: Option<String>,
    pub id: String,
    pub name: String,
    /// `starter`, `containment` o `platform`: ordena el recorrido en el panel.
    pub category: String,
    pub description: String,
    /// Qué enseña este servicio. Se muestra en la tarjeta: un servicio que
    /// arranca sin explicar qué demuestra es una demo, no un laboratorio.
    pub teaches: String,
    /// Puerto del loopback del host donde publica.
    pub port: u16,
    pub kind: String,
    pub entrypoint: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Id de la política con la que se levanta.
    pub policy: String,
    /// Runtime preferido; si no está disponible se cae al siguiente de la lista.
    pub runtimes: Vec<String>,
    /// Ruta de salud, relativa a la raíz del servicio.
    pub health_path: String,
    #[serde(skip)]
    pub directory: PathBuf,
}

impl Service {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let supplied = path.as_ref();
        let manifest = if supplied.is_dir() { supplied.join("service.json") } else { supplied.to_path_buf() };
        let directory = manifest.parent().context("El manifiesto no tiene directorio")?.canonicalize()?;
        let content =
            fs::read_to_string(&manifest).with_context(|| format!("No se pudo leer {}", manifest.display()))?;
        let mut service: Service = serde_json::from_str(&content)
            .with_context(|| format!("Manifiesto de servicio inválido en {}", manifest.display()))?;
        service.directory = directory;
        service.validate()?;
        Ok(service)
    }

    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            bail!("El servicio requiere id");
        }
        // Puertos por debajo de 1024 exigen privilegios: un servicio que los
        // pide obligaría a levantar el sandbox como root, que es lo contrario
        // de lo que este proyecto enseña.
        if self.port < 1024 {
            bail!("{}: el puerto {} exige privilegios", self.id, self.port);
        }
        if self.command.contains('/') || self.command.contains('\\') {
            bail!("{}: command debe ser un ejecutable, no una ruta", self.id);
        }
        if self.runtimes.is_empty() {
            bail!("{}: debe declarar al menos un runtime", self.id);
        }
        if !self.health_path.starts_with('/') {
            bail!("{}: healthPath debe empezar por /", self.id);
        }
        let entrypoint = self.directory.join(&self.entrypoint);
        if !entrypoint.is_file() {
            bail!("{}: el entrypoint no existe: {}", self.id, entrypoint.display());
        }
        Ok(())
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn health_url(&self) -> String {
        format!("http://127.0.0.1:{}{}", self.port, self.health_path)
    }
}

/// Estado observable de un servicio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceState {
    /// No hay proceso registrado.
    Stopped,
    /// Hay proceso y el puerto responde.
    Running,
    /// Hay proceso registrado pero el puerto todavía no responde.
    Starting,
    /// El proceso registrado ya no existe: murió sin bajarse.
    Crashed,
}

impl ServiceState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stopped => "detenido",
            Self::Running => "corriendo",
            Self::Starting => "arrancando",
            Self::Crashed => "caído",
        }
    }
}

/// Registro persistente de un servicio levantado.
///
/// Se escribe en disco para que `service list` y el panel sepan qué hay en
/// marcha aunque el proceso que lo levantó ya no exista: sin esto, cerrar la
/// terminal dejaría sandboxes huérfanos imposibles de bajar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceRecord {
    pub id: String,
    pub pid: u32,
    pub port: u16,
    pub runtime: String,
    pub policy: String,
    pub started_at: String,
    pub log_path: String,
    /// Controles que el runtime declaró aplicar al levantarlo. Se guardan aquí
    /// para que la tarjeta del panel muestre bajo qué contención corre, no solo
    /// que corre.
    pub effective_controls: Vec<String>,
}

impl ServiceRecord {
    pub fn path(data_root: &Path, id: &str) -> PathBuf {
        data_root.join("services").join(format!("{id}.json"))
    }

    pub fn write(&self, data_root: &Path) -> Result<()> {
        let path = Self::path(data_root, &self.id);
        fs::create_dir_all(path.parent().expect("directorio de servicios"))?;
        fs::write(&path, serde_json::to_string_pretty(self)?)
            .with_context(|| format!("No se pudo registrar el servicio en {}", path.display()))?;
        Ok(())
    }

    pub fn read(data_root: &Path, id: &str) -> Option<Self> {
        let content = fs::read_to_string(Self::path(data_root, id)).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn remove(data_root: &Path, id: &str) {
        let _ = fs::remove_file(Self::path(data_root, id));
    }
}

/// ¿Sigue vivo el proceso registrado?
///
/// En Linux se comprueba `/proc/<pid>`; enviar la señal 0 diría lo mismo pero
/// requiere que el proceso sea del mismo usuario, y un sandbox puede haber
/// cambiado de uid.
pub fn process_alive(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> Service {
        Service {
            schema: None,
            id: "demo".into(),
            name: "Demo".into(),
            category: "starter".into(),
            description: String::new(),
            teaches: String::new(),
            port: 8801,
            kind: "python".into(),
            entrypoint: "app.py".into(),
            command: "python3".into(),
            args: vec![],
            policy: "service-sandbox".into(),
            runtimes: vec!["bwrap".into()],
            health_path: "/health".into(),
            directory: PathBuf::from("."),
        }
    }

    #[test]
    fn rejects_privileged_ports() {
        let mut value = service();
        value.port = 80;
        let error = value.validate().expect_err("un puerto privilegiado debe rechazarse");
        assert!(error.to_string().contains("privilegios"));
    }

    #[test]
    fn rejects_a_command_that_is_a_path() {
        let mut value = service();
        value.command = "/usr/bin/python3".into();
        assert!(value.validate().is_err(), "command debe ser un ejecutable, no una ruta");
    }

    #[test]
    fn rejects_a_service_without_runtimes() {
        let mut value = service();
        value.runtimes.clear();
        assert!(value.validate().is_err());
    }

    #[test]
    fn rejects_a_health_path_that_is_not_absolute() {
        let mut value = service();
        value.health_path = "health".into();
        assert!(value.validate().is_err());
    }

    #[test]
    fn builds_loopback_urls() {
        let value = service();
        assert_eq!(value.url(), "http://127.0.0.1:8801");
        assert_eq!(value.health_url(), "http://127.0.0.1:8801/health");
    }

    #[test]
    fn states_have_readable_labels() {
        assert_eq!(ServiceState::Running.label(), "corriendo");
        assert_eq!(ServiceState::Crashed.label(), "caído");
    }

    #[test]
    fn pid_zero_is_never_alive() {
        // PID 0 no es un proceso de usuario en Linux; tratarlo como vivo dejaría
        // registros huérfanos que nunca se pueden limpiar.
        assert!(!process_alive(0));
    }
}
