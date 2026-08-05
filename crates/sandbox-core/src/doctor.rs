use crate::RuntimeKind;
use serde::Serialize;
use std::{env, path::Path};

#[derive(Debug, Clone, Serialize)]
pub struct DoctorCheck {
    pub name: String,
    pub available: bool,
    pub detail: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub platform: String,
    pub checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    pub fn collect() -> Self {
        let platform = format!("{}-{}", env::consts::OS, env::consts::ARCH);
        let runtimes = [
            RuntimeKind::DryRun,
            RuntimeKind::Native,
            RuntimeKind::Bwrap,
            RuntimeKind::Unshare,
            RuntimeKind::Gvisor,
            RuntimeKind::Kata,
            RuntimeKind::Wasi,
            RuntimeKind::Firecracker,
        ];
        let mut checks = runtimes
            .into_iter()
            .map(|runtime| {
                let probe = runtime.probe();
                DoctorCheck {
                    name: probe.id,
                    available: probe.available,
                    detail: if probe.version.is_empty() {
                        probe.detail
                    } else {
                        format!("{} · {}", probe.version, probe.detail)
                    },
                }
            })
            .collect::<Vec<_>>();
        #[cfg(target_os = "linux")]
        checks.push(DoctorCheck {
            name: "cgroup-v2".into(),
            available: Path::new("/sys/fs/cgroup/cgroup.controllers").exists(),
            detail: "/sys/fs/cgroup/cgroup.controllers".into(),
        });
        #[cfg(target_os = "linux")]
        checks.push(DoctorCheck {
            name: "KVM".into(),
            available: Path::new("/dev/kvm").exists(),
            detail: "/dev/kvm".into(),
        });
        Self { platform, checks }
    }
}
