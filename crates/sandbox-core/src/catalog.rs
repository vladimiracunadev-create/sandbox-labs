use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub project: Project,
    pub runtimes: Vec<RuntimeDescriptor>,
    /// Los casos del sistema. Cada uno es un producto que se levanta en su
    /// propio puerto y donde se hacen tareas, no un tema que se explica.
    pub cases: Vec<Case>,
    pub workloads_directory: String,
    pub policies_directory: String,
    pub cases_directory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub name: String,
    pub version: String,
    pub control_center_port: u16,
    pub default_runtime: String,
    pub evidence_directory: String,
    pub data_directory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeDescriptor {
    pub id: String,
    pub label: String,
    pub status: String,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub controls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub id: String,
    pub slug: String,
    pub title: String,
    /// La idea que este caso enseña y ningún otro enseña. Si dos casos
    /// comparten idea, uno de los dos sobra.
    pub idea: String,
    pub port: u16,
    pub status: String,
}

impl Catalog {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).with_context(|| format!("No se pudo leer {}", path.display()))?;
        let catalog: Catalog =
            serde_json::from_str(&content).with_context(|| format!("JSON inválido en {}", path.display()))?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<()> {
        if self.project.control_center_port == 0 {
            bail!("controlCenterPort debe ser mayor que cero");
        }
        let mut case_ids = BTreeSet::new();
        let mut slugs = BTreeSet::new();
        let mut ports = BTreeSet::new();
        for case in &self.cases {
            if !case_ids.insert(&case.id) {
                bail!("ID de caso duplicado: {}", case.id);
            }
            if !slugs.insert(&case.slug) {
                bail!("Slug de caso duplicado: {}", case.slug);
            }
            // Dos casos en el mismo puerto se pisan al levantarse y el segundo
            // falla con un error de socket que no explica nada.
            if !ports.insert(case.port) {
                bail!("Puerto duplicado entre casos: {}", case.port);
            }
        }
        let mut runtime_ids = BTreeSet::new();
        for runtime in &self.runtimes {
            if !runtime_ids.insert(&runtime.id) {
                bail!("Runtime duplicado: {}", runtime.id);
            }
        }
        if !runtime_ids.contains(&self.project.default_runtime) {
            bail!("defaultRuntime no existe en runtimes");
        }
        Ok(())
    }
}
