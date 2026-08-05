use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workload {
    #[serde(default, rename = "$schema")]
    pub schema: Option<String>,
    pub id: String,
    pub kind: String,
    pub entrypoint: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub risk: String,
    #[serde(default)]
    pub description: String,
    pub allow_native: bool,
    pub expected: ExpectedOutcome,
    #[serde(skip)]
    pub directory: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedOutcome {
    pub outcome: String,
    #[serde(default)]
    pub contains: Vec<String>,
}

impl Workload {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let supplied = path.as_ref();
        let manifest = if supplied.is_dir() { supplied.join("manifest.json") } else { supplied.to_path_buf() };
        let directory = manifest.parent().context("El manifiesto no tiene directorio")?.canonicalize()?;
        let content =
            fs::read_to_string(&manifest).with_context(|| format!("No se pudo leer {}", manifest.display()))?;
        let mut workload: Workload =
            serde_json::from_str(&content).with_context(|| format!("Manifest inválido en {}", manifest.display()))?;
        workload.directory = directory;
        workload.validate()?;
        Ok(workload)
    }

    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            bail!("La carga requiere id");
        }
        if self.args.len() > 32 {
            bail!("La carga supera 32 argumentos");
        }
        if self.command.contains('/') || self.command.contains('\\') {
            bail!("command debe ser un ejecutable, no una ruta");
        }
        let candidate = self.directory.join(&self.entrypoint);
        if self.kind == "wasi" && !candidate.exists() {
            // El módulo puede generarse en una etapa posterior del build. El runtime
            // fallará de forma explícita si se intenta ejecutar antes de compilarlo.
            return Ok(());
        }
        let entrypoint = self.entrypoint_path()?;
        if !entrypoint.is_file() {
            bail!("El entrypoint no es un archivo: {}", entrypoint.display());
        }
        Ok(())
    }

    pub fn entrypoint_path(&self) -> Result<PathBuf> {
        if self.entrypoint.contains("..") || Path::new(&self.entrypoint).is_absolute() {
            bail!("Entrypoint fuera de la carga");
        }
        let candidate = self.directory.join(&self.entrypoint).canonicalize()?;
        if !candidate.starts_with(&self.directory) {
            bail!("Entrypoint fuera de la carga");
        }
        Ok(candidate)
    }

    pub fn portable_path(&self) -> String {
        let components = self
            .directory
            .components()
            .map(|value| value.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        if let Some(index) = components.iter().position(|value| value == "workloads") {
            return components[index..].join("/");
        }
        self.directory.file_name().map(|value| value.to_string_lossy().to_string()).unwrap_or_else(|| self.id.clone())
    }

    pub fn command_args(&self, extra: &[String]) -> Result<Vec<String>> {
        if extra.len() > 16 || extra.iter().any(|value| value.len() > 256 || value.contains('\0')) {
            bail!("Argumentos adicionales inválidos");
        }
        let mut args = self.args.clone();
        if !self.entrypoint.is_empty() && !args.iter().any(|arg| arg == &self.entrypoint) {
            args.insert(0, self.entrypoint.clone());
        }
        args.extend_from_slice(extra);
        Ok(args)
    }

    pub fn hash(&self) -> Result<String> {
        let mut entries: Vec<_> = WalkDir::new(&self.directory)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .collect();
        entries.sort_by_key(|entry| entry.path().to_path_buf());
        let mut digest = Sha256::new();
        for entry in entries {
            let relative = entry.path().strip_prefix(&self.directory)?;
            digest.update(relative.to_string_lossy().as_bytes());
            digest.update([0]);
            digest.update(fs::read(entry.path())?);
            digest.update([0]);
        }
        Ok(format!("{:x}", digest.finalize()))
    }
}
