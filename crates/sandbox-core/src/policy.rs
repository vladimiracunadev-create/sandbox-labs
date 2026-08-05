use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EnforcementMode {
    Strict,
    BestEffort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    #[serde(default, rename = "$schema")]
    pub schema: Option<String>,
    pub id: String,
    #[serde(default)]
    pub description: String,
    pub enforcement: EnforcementPolicy,
    pub filesystem: FilesystemPolicy,
    pub network: NetworkPolicy,
    pub resources: ResourcePolicy,
    pub process: ProcessPolicy,
    #[serde(default)]
    pub syscalls: SyscallPolicy,
    #[serde(default)]
    pub devices: DevicePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnforcementPolicy {
    pub mode: EnforcementMode,
    pub required_controls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesystemPolicy {
    pub root: String,
    pub read_only: Vec<String>,
    pub writable: Vec<String>,
    pub max_workspace_mb: u64,
    pub follow_symlinks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    pub mode: String,
    #[serde(default)]
    pub hosts: Vec<String>,
    pub dns: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcePolicy {
    pub cpu: f64,
    pub memory_mb: u64,
    pub processes: u32,
    pub timeout_seconds: u64,
    pub open_files: u64,
    pub output_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessPolicy {
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub allowed_environment: Vec<String>,
    pub user: u32,
    pub group: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallPolicy {
    #[serde(default = "default_profile")]
    pub profile: String,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DevicePolicy {
    #[serde(default)]
    pub allow: Vec<String>,
}

fn default_profile() -> String {
    "default".to_string()
}

impl Default for SyscallPolicy {
    fn default() -> Self {
        Self { profile: default_profile(), allow: vec![], deny: vec![] }
    }
}

impl Policy {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).with_context(|| format!("No se pudo leer {}", path.display()))?;
        let policy: Policy =
            serde_json::from_str(&content).with_context(|| format!("Política JSON inválida en {}", path.display()))?;
        policy.validate()?;
        Ok(policy)
    }

    pub fn hash(path: impl AsRef<Path>) -> Result<String> {
        let bytes = fs::read(path.as_ref())?;
        Ok(format!("{:x}", Sha256::digest(bytes)))
    }

    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            bail!("La política requiere un id");
        }
        if self.resources.cpu <= 0.0 {
            bail!("resources.cpu debe ser mayor que cero");
        }
        if self.resources.memory_mb < 16 {
            bail!("resources.memoryMb debe ser al menos 16");
        }
        if self.resources.processes == 0 || self.resources.timeout_seconds == 0 {
            bail!("Los límites de procesos y tiempo deben ser mayores que cero");
        }
        if self.resources.open_files < 8 || self.resources.output_bytes < 1024 {
            bail!("openFiles u outputBytes son demasiado pequeños");
        }
        let allowed_network = ["none", "loopback", "allowlist", "unrestricted"];
        if !allowed_network.contains(&self.network.mode.as_str()) {
            bail!("network.mode no reconocido: {}", self.network.mode);
        }
        if self.network.mode != "allowlist" && !self.network.hosts.is_empty() {
            bail!("network.hosts solo se usa con mode=allowlist");
        }
        let known_controls: BTreeSet<&str> = [
            "filesystem",
            "network",
            "processes",
            "memory",
            "cpu",
            "timeout",
            "capabilities",
            "syscalls",
            "devices",
            "environment",
            "output",
        ]
        .into_iter()
        .collect();
        let mut seen = BTreeSet::new();
        for control in &self.enforcement.required_controls {
            if !known_controls.contains(control.as_str()) {
                bail!("Control requerido desconocido: {control}");
            }
            if !seen.insert(control) {
                bail!("Control requerido duplicado: {control}");
            }
        }
        for path in self.filesystem.read_only.iter().chain(&self.filesystem.writable) {
            if !path.starts_with('/') || path.contains("..") {
                bail!("Ruta de política inválida: {path}");
            }
        }
        let read_only: BTreeSet<_> = self.filesystem.read_only.iter().collect();
        if self.filesystem.writable.iter().any(|path| read_only.contains(path)) {
            bail!("Una ruta no puede ser readOnly y writable al mismo tiempo");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> Policy {
        Policy {
            schema: None,
            id: "test".into(),
            description: String::new(),
            enforcement: EnforcementPolicy { mode: EnforcementMode::Strict, required_controls: vec!["timeout".into()] },
            filesystem: FilesystemPolicy {
                root: "ephemeral".into(),
                read_only: vec![],
                writable: vec![],
                max_workspace_mb: 16,
                follow_symlinks: false,
            },
            network: NetworkPolicy { mode: "none".into(), hosts: vec![], dns: "disabled".into() },
            resources: ResourcePolicy {
                cpu: 1.0,
                memory_mb: 128,
                processes: 1,
                timeout_seconds: 1,
                open_files: 16,
                output_bytes: 1024,
            },
            process: ProcessPolicy {
                capabilities: vec![],
                environment: BTreeMap::new(),
                allowed_environment: vec![],
                user: 65534,
                group: 65534,
            },
            syscalls: SyscallPolicy::default(),
            devices: DevicePolicy::default(),
        }
    }

    #[test]
    fn rejects_zero_cpu() {
        let mut value = policy();
        value.resources.cpu = 0.0;
        assert!(value.validate().is_err());
    }

    #[test]
    fn rejects_overlapping_mounts() {
        let mut value = policy();
        value.filesystem.read_only.push("/workspace".into());
        value.filesystem.writable.push("/workspace".into());
        assert!(value.validate().is_err());
    }
}
