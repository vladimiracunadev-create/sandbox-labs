//! Contratos del repositorio: verifican que el catálogo, las políticas y las
//! cargas registradas siguen siendo coherentes entre sí.
//!
//! Estas pruebas no ejecutan ninguna carga. Leen los artefactos versionados y
//! comprueban las invariantes que el resto del sistema da por hechas.

use sandbox_core::{Catalog, EnforcementMode, ExecutionPlan, Policy, RuntimeKind, Workload};
use std::{collections::BTreeSet, fs, path::PathBuf, str::FromStr};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").canonicalize().expect("raíz del repositorio")
}

fn policy_files() -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(repo_root().join("policies"))
        .expect("directorio policies")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|value| value == "json"))
        .collect();
    files.sort();
    files
}

fn workload_manifests() -> Vec<PathBuf> {
    fn walk(directory: &PathBuf, found: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(directory).expect("directorio de cargas").filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, found);
            } else if path.file_name().is_some_and(|value| value == "manifest.json") {
                found.push(path);
            }
        }
    }
    let mut found = Vec::new();
    walk(&repo_root().join("workloads"), &mut found);
    found.sort();
    found
}

fn load_policy(id: &str) -> Policy {
    Policy::load(repo_root().join("policies").join(format!("{id}.json"))).expect("política del repositorio")
}

fn load_workload(relative: &str) -> Workload {
    Workload::load(repo_root().join(relative)).expect("carga del repositorio")
}

#[test]
fn catalog_loads_and_validates() {
    let catalog = Catalog::load(repo_root().join("sandbox.config.json")).expect("catálogo válido");
    assert!(!catalog.labs.is_empty(), "el catálogo debe declarar laboratorios");
    assert!(!catalog.runtimes.is_empty(), "el catálogo debe declarar runtimes");
    assert_eq!(catalog.project.name, "Sandbox Labs");
}

#[test]
fn catalog_labs_match_directories() {
    let catalog = Catalog::load(repo_root().join("sandbox.config.json")).expect("catálogo válido");
    // El directorio combina id y slug: 01-baseline-unrestricted.
    let declared: BTreeSet<String> = catalog.labs.iter().map(|lab| format!("{}-{}", lab.id, lab.slug)).collect();
    let on_disk: BTreeSet<String> = fs::read_dir(repo_root().join("labs"))
        .expect("directorio labs")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(declared, on_disk, "el catálogo y el directorio labs/ se desincronizaron");
}

#[test]
fn catalog_runtimes_are_known_kinds() {
    let catalog = Catalog::load(repo_root().join("sandbox.config.json")).expect("catálogo válido");
    for runtime in &catalog.runtimes {
        RuntimeKind::from_str(&runtime.id)
            .unwrap_or_else(|_| panic!("runtime desconocido en el catálogo: {}", runtime.id));
    }
}

#[test]
fn every_policy_loads_and_validates() {
    let files = policy_files();
    assert!(!files.is_empty(), "no se encontró ninguna política");
    for path in files {
        let policy = Policy::load(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        policy.validate().unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        // El id de la política debe coincidir con el nombre del archivo para que
        // el Control Center pueda resolverla sin un índice adicional.
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        assert_eq!(policy.id, stem, "{} declara id={}", path.display(), policy.id);
    }
}

#[test]
fn every_workload_loads_and_validates() {
    let manifests = workload_manifests();
    assert!(!manifests.is_empty(), "no se encontró ninguna carga");
    let mut ids = BTreeSet::new();
    for path in manifests {
        let workload = Workload::load(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(ids.insert(workload.id.clone()), "id de carga duplicado: {}", workload.id);
        assert!(
            ["benign", "resource-abuse", "adversarial-simulation"].contains(&workload.risk.as_str()),
            "{} declara un riesgo desconocido: {}",
            path.display(),
            workload.risk
        );
    }
}

#[test]
fn risky_workloads_never_allow_native() {
    for path in workload_manifests() {
        let workload = Workload::load(&path).expect("carga válida");
        if workload.risk != "benign" {
            assert!(
                !workload.allow_native,
                "{} es de riesgo {} y no puede declarar allowNative",
                path.display(),
                workload.risk
            );
        }
    }
}

#[test]
fn runtime_kind_round_trips() {
    let ids = ["dry-run", "native", "bwrap", "unshare", "gvisor", "kata", "wasi", "firecracker"];
    for id in ids {
        let kind = RuntimeKind::from_str(id).expect("runtime conocido");
        assert_eq!(kind.to_string(), id);
    }
    assert!(RuntimeKind::from_str("docker").is_err(), "un runtime no registrado debe fallar");
}

#[test]
fn dry_run_never_executes() {
    let policy = load_policy("minimal");
    let workload = load_workload("workloads/benign/hello");
    let plan = ExecutionPlan::build(RuntimeKind::DryRun, &workload, &policy).expect("plan");
    assert!(!plan.executable, "dry-run nunca debe marcarse ejecutable");
    assert!(plan.block_reason.is_some(), "dry-run debe explicar por qué no ejecuta");
    assert!(!plan.steps.is_empty());
}

#[test]
fn documented_runtimes_are_never_executable() {
    let policy = load_policy("minimal");
    let workload = load_workload("workloads/benign/hello");
    for runtime in [RuntimeKind::Gvisor, RuntimeKind::Kata, RuntimeKind::Firecracker] {
        let plan = ExecutionPlan::build(runtime, &workload, &policy).expect("plan");
        assert!(!plan.executable, "{runtime} está documentado y no debe ejecutar");
        assert!(plan.block_reason.is_some(), "{runtime} debe explicar el bloqueo");
    }
}

#[test]
fn native_requires_explicit_opt_in() {
    // La prueba no fija SANDBOX_LABS_ALLOW_NATIVE: sin ese opt-in el plan debe
    // bloquearse aunque la carga lo permita.
    if std::env::var("SANDBOX_LABS_ALLOW_NATIVE").as_deref() == Ok("1") {
        return;
    }
    let policy = load_policy("minimal");
    let workload = load_workload("workloads/benign/hello");
    let plan = ExecutionPlan::build(RuntimeKind::Native, &workload, &policy).expect("plan");
    assert!(!plan.executable, "native sin opt-in no debe ejecutar");
    assert!(plan.block_reason.unwrap().contains("SANDBOX_LABS_ALLOW_NATIVE"));
}

#[test]
fn strict_policy_blocks_when_controls_are_unsupported() {
    let policy = load_policy("high-risk");
    assert_eq!(policy.enforcement.mode, EnforcementMode::Strict);
    let workload = load_workload("workloads/benign/hello");
    // unshare no aplica filesystem ni capabilities: una política estricta que los
    // exige tiene que fallar cerrado.
    let plan = ExecutionPlan::build(RuntimeKind::Unshare, &workload, &policy).expect("plan");
    assert!(!plan.controls.unsupported.is_empty(), "el escenario requiere controles no soportados");
    assert!(!plan.executable, "una política estricta con huecos no puede ejecutar");
}

#[test]
fn control_assessment_partitions_requested_controls() {
    let workload = load_workload("workloads/benign/hello");
    for path in policy_files() {
        let policy = Policy::load(&path).expect("política válida");
        for runtime in
            [RuntimeKind::DryRun, RuntimeKind::Native, RuntimeKind::Bwrap, RuntimeKind::Unshare, RuntimeKind::Wasi]
        {
            let plan = ExecutionPlan::build(runtime, &workload, &policy).expect("plan");
            let mut rebuilt = plan.controls.effective.clone();
            rebuilt.extend(plan.controls.unsupported.clone());
            rebuilt.sort();
            let mut requested = plan.controls.requested.clone();
            requested.sort();
            assert_eq!(rebuilt, requested, "{} con {runtime} pierde controles", policy.id);
        }
    }
}

#[test]
fn workload_rejects_hostile_extra_arguments() {
    let workload = load_workload("workloads/benign/hello");
    assert!(workload.command_args(&[]).is_ok());
    assert!(workload.command_args(&["ok".to_string()]).is_ok());
    let too_many: Vec<String> = (0..17).map(|value| value.to_string()).collect();
    assert!(workload.command_args(&too_many).is_err(), "más de 16 argumentos debe fallar");
    assert!(workload.command_args(&["x".repeat(257)]).is_err(), "un argumento demasiado largo debe fallar");
    assert!(workload.command_args(&["con\0nulo".to_string()]).is_err(), "un byte nulo debe fallar");
}

#[test]
fn workload_hash_is_stable_and_content_addressed() {
    let workload = load_workload("workloads/benign/hello");
    let first = workload.hash().expect("hash");
    let second = workload.hash().expect("hash");
    assert_eq!(first, second, "el hash de una carga debe ser determinista");
    assert_eq!(first.len(), 64, "se espera SHA-256 en hexadecimal");

    let other = load_workload("workloads/benign/filesystem-probe");
    assert_ne!(first, other.hash().expect("hash"), "cargas distintas no pueden compartir hash");
}

#[test]
fn policy_hash_matches_file_contents() {
    let path = repo_root().join("policies").join("minimal.json");
    let hash = Policy::hash(&path).expect("hash");
    assert_eq!(hash.len(), 64);
    assert_eq!(hash, Policy::hash(&path).expect("hash"), "el hash de la política debe ser determinista");
}

#[test]
fn portable_path_never_leaks_the_host() {
    for path in workload_manifests() {
        let workload = Workload::load(&path).expect("carga válida");
        let portable = workload.portable_path();
        assert!(portable.starts_with("workloads/"), "{portable} no es una ruta portable");
        assert!(!portable.contains('\\'), "{portable} arrastra separadores de Windows");
        assert!(!portable.contains(':'), "{portable} arrastra una unidad del host");
    }
}
