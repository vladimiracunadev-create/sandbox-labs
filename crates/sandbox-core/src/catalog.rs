use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub project: Project,
    pub runtimes: Vec<RuntimeDescriptor>,
    pub labs: Vec<Lab>,
    pub workloads_directory: String,
    pub policies_directory: String,
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
pub struct Lab {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub level: String,
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
        let mut lab_ids = BTreeSet::new();
        let mut slugs = BTreeSet::new();
        for lab in &self.labs {
            if !lab_ids.insert(&lab.id) {
                bail!("ID de laboratorio duplicado: {}", lab.id);
            }
            if !slugs.insert(&lab.slug) {
                bail!("Slug de laboratorio duplicado: {}", lab.slug);
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
