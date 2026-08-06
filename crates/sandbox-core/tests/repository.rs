//! Contratos del repositorio: verifican que el catálogo, las políticas y las
//! cargas registradas siguen siendo coherentes entre sí.
//!
//! Estas pruebas no ejecutan ninguna carga. Leen los artefactos versionados y
//! comprueban las invariantes que el resto del sistema da por hechas.

use sandbox_core::{Catalog, EnforcementMode, EscapeSuite, ExecutionPlan, Policy, RuntimeKind, Workload};
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

fn all_policies() -> Vec<Policy> {
    policy_files()
        .into_iter()
        .map(|path| Policy::load(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
        .collect()
}

fn load_workload(relative: &str) -> Workload {
    Workload::load(repo_root().join(relative)).expect("carga del repositorio")
}

/// Los casos que existen en disco (los que ya están construidos).
fn built_cases() -> Vec<sandbox_core::Service> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(repo_root().join("cases")) else { return found };
    for entry in entries.filter_map(Result::ok) {
        let manifest = entry.path().join("service.json");
        if manifest.is_file() {
            found
                .push(sandbox_core::Service::load(&manifest).unwrap_or_else(|e| panic!("{}: {e}", manifest.display())));
        }
    }
    found.sort_by(|a, b| a.id.cmp(&b.id));
    found
}

#[test]
fn catalog_loads_and_validates() {
    let catalog = Catalog::load(repo_root().join("sandbox.config.json")).expect("catálogo válido");
    assert!(!catalog.cases.is_empty(), "el catálogo debe declarar casos");
    assert!(!catalog.runtimes.is_empty(), "el catálogo debe declarar runtimes");
    assert_eq!(catalog.project.name, "Sandbox Labs");
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
            ["benign", "controlled", "resource-abuse", "adversarial-simulation"].contains(&workload.risk.as_str()),
            "{} declara un riesgo desconocido: {}",
            path.display(),
            workload.risk
        );
    }
}

#[test]
fn risky_workloads_never_allow_native() {
    // `controlled` existe para las sondas de observación de la suite de
    // contención: solo miden y reportan, así que pueden correr en native para
    // obtener la línea base sin aislamiento. Las que consumen recursos o
    // simulan una fuga siguen fuera de native, sin excepción.
    for path in workload_manifests() {
        let workload = Workload::load(&path).expect("carga válida");
        if matches!(workload.risk.as_str(), "resource-abuse" | "adversarial-simulation") {
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

// ── Suite de contención ──────────────────────────────────────────────────────

fn suite() -> EscapeSuite {
    EscapeSuite::load(repo_root().join("escape-suite").join("suite.json")).expect("suite de contención")
}

#[test]
fn escape_suite_loads_and_validates() {
    let value = suite();
    assert!(!value.probes.is_empty(), "la suite debe declarar sondas");
    assert!(!value.dimensions.is_empty(), "la suite debe declarar dimensiones");
    value.validate().expect("suite coherente");
}

#[test]
fn every_probe_points_to_a_registered_workload() {
    let registered: BTreeSet<String> =
        workload_manifests().iter().map(|path| Workload::load(path).expect("carga válida").id).collect();
    for probe in &suite().probes {
        assert!(
            registered.contains(&probe.workload),
            "la sonda {} apunta a una carga no registrada: {}",
            probe.id,
            probe.workload
        );
    }
}

#[test]
fn every_probe_declares_a_known_control() {
    // El control de la sonda es lo que se compara con `supported_controls` del
    // runtime para detectar falsas garantías: si no es un control real del
    // modelo, la comparación no significaría nada.
    let policy = load_policy("containment-audit");
    let known = RuntimeKind::Gvisor.supported_controls(&policy);
    for probe in &suite().probes {
        assert!(
            known.contains(&probe.control),
            "la sonda {} declara un control desconocido: {}",
            probe.id,
            probe.control
        );
    }
}

#[test]
fn every_dimension_is_exercised_by_a_probe() {
    let value = suite();
    let exercised: BTreeSet<&str> = value.probes.iter().map(|probe| probe.dimension.as_str()).collect();
    for dimension in &value.dimensions {
        assert!(exercised.contains(dimension.id.as_str()), "la dimensión {} no tiene sonda", dimension.id);
    }
}

#[test]
fn the_audit_policy_is_executable_by_design() {
    // Una política estricta falla cerrada antes de ejecutar y no mediría nada.
    // La de auditoría es best-effort justamente para poder observar la realidad.
    let policy = load_policy("containment-audit");
    assert_eq!(policy.enforcement.mode, EnforcementMode::BestEffort, "una política strict no puede auditar contención");
    assert!(policy.enforcement.required_controls.len() >= 8, "la auditoría debe pedir controles suficientes");
    assert_eq!(policy.network.mode, "none", "la auditoría mide con la red cerrada por política");
}

#[test]
fn escape_probes_pass_the_real_resource_budget() {
    // Una sonda que midiera contra una constante inventada no probaría nada
    // sobre la política; debe recibir el presupuesto real.
    let policy = load_policy("containment-audit");
    for probe in &suite().probes {
        match probe.argument.as_deref() {
            Some("memoryMb") => {
                assert_eq!(EscapeSuite::argument_value(probe, &policy), Some(policy.resources.memory_mb.to_string()))
            }
            Some("processes") => {
                assert_eq!(EscapeSuite::argument_value(probe, &policy), Some(policy.resources.processes.to_string()))
            }
            None => assert!(EscapeSuite::argument_value(probe, &policy).is_none()),
            _ => {}
        }
    }
}

#[test]
fn every_service_loads_and_validates() {
    let all = built_cases();
    assert!(!all.is_empty(), "no hay servicios registrados");
    let mut ids = BTreeSet::new();
    for service in &all {
        assert!(ids.insert(service.id.clone()), "servicio duplicado: {}", service.id);
        service.validate().unwrap_or_else(|e| panic!("{}: {e}", service.id));
    }
}

#[test]
fn services_never_share_a_port() {
    // Dos servicios en el mismo puerto se pisan al levantarse y el segundo
    // falla con un error de socket que no dice qué pasó.
    let mut ports = BTreeSet::new();
    for service in built_cases() {
        assert!(ports.insert(service.port), "puerto {} repetido en {}", service.port, service.id);
    }
}

#[test]
fn service_policies_and_runtimes_are_registered() {
    let policies: BTreeSet<String> =
        policy_files().iter().map(|p| p.file_stem().unwrap().to_string_lossy().to_string()).collect();
    for service in built_cases() {
        assert!(policies.contains(&service.policy), "{}: política no registrada ({})", service.id, service.policy);
        for runtime in &service.runtimes {
            RuntimeKind::from_str(runtime).unwrap_or_else(|_| panic!("{}: runtime desconocido {runtime}", service.id));
        }
    }
}

#[test]
fn the_service_policy_names_the_host_network_it_actually_keeps() {
    // Un servicio sin red no puede publicar nada, así que la política de
    // servicios abre esa frontera a propósito. Lo que no puede hacer es
    // llamarla `loopback`: ese modo crea un namespace de red propio, y lo que
    // los servicios reciben es la red del host entera. El nombre tiene que
    // decir lo que pasa.
    let policy = load_policy("service-sandbox");
    assert_eq!(policy.network.mode, "unrestricted", "los servicios conservan la red del host, y así debe llamarse");
    assert!(
        !policy.network.isolates_host_network(),
        "si esto aislara, el puerto publicado no sería alcanzable desde el host"
    );
    assert!(
        !policy.enforcement.required_controls.iter().any(|value| value == "network"),
        "una política que conserva la red del host no puede exigir el control `network`"
    );
    // Y el resto de la contención sigue exigida.
    for control in ["filesystem", "capabilities", "environment"] {
        assert!(
            policy.enforcement.required_controls.iter().any(|value| value == control),
            "la política de servicios debe seguir exigiendo {control}"
        );
    }
}

#[test]
fn a_policy_that_requires_network_either_isolates_or_fails_closed() {
    // Exigir el control `network` con un modo que no crea namespace propio
    // —hoy, `allowlist` y `unrestricted`— es pedir algo que ningún runtime
    // sabe aplicar. Eso no está prohibido: una política puede declarar la
    // frontera que quiere aunque todavía no exista quien la haga cumplir. Lo
    // que no puede pasar es que se ejecute igual y nadie se entere.
    //
    // La forma honesta de que exista es `strict`, que la bloquea. Esta prueba
    // comprueba las dos mitades: la declaración y la consecuencia.
    let workload = load_workload("workloads/benign/hello");
    for policy in all_policies() {
        if !policy.enforcement.required_controls.iter().any(|value| value == "network") {
            continue;
        }
        if policy.network.isolates_host_network() {
            continue;
        }
        assert_eq!(
            policy.enforcement.mode,
            EnforcementMode::Strict,
            "{}: exige `network` con mode «{}», que no lo aplica nadie. En best-effort se ejecutaría \
             sin el control; tiene que ser estricta para fallar cerrado",
            policy.id,
            policy.network.mode
        );
        for runtime in [RuntimeKind::Bwrap, RuntimeKind::Unshare, RuntimeKind::Wasi] {
            let plan = ExecutionPlan::build(runtime, &workload, &policy).expect("plan");
            assert!(
                plan.controls.unsupported.iter().any(|value| value == "network"),
                "{} con {runtime}: `network` tiene que constar como no soportado",
                policy.id
            );
            assert!(!plan.executable, "{} con {runtime}: tiene que fallar cerrado", policy.id);
        }
    }
}

#[test]
fn service_ports_are_unprivileged() {
    for service in built_cases() {
        assert!(service.port >= 1024, "{} usa un puerto privilegiado: {}", service.id, service.port);
    }
}

// ── Casos ────────────────────────────────────────────────────────────────────

#[test]
fn catalog_cases_are_coherent() {
    let catalog = Catalog::load(repo_root().join("sandbox.config.json")).expect("catálogo válido");
    assert!(!catalog.cases.is_empty(), "el catálogo debe declarar casos");

    let mut ports = BTreeSet::new();
    for case in &catalog.cases {
        assert!(ports.insert(case.port), "puerto duplicado: {}", case.port);
        assert!(case.port >= 1024, "{} usa un puerto privilegiado", case.slug);
        // Un caso sin idea propia es un tema, no un caso.
        assert!(case.idea.len() > 20, "{} no explica qué idea enseña", case.slug);
        assert!(
            matches!(case.status.as_str(), "planned" | "building" | "ready"),
            "{} declara un estado desconocido: {}",
            case.slug,
            case.status
        );
    }
}

#[test]
fn built_cases_live_in_the_cases_directory() {
    // Un caso `ready` o `building` tiene que existir en disco; uno `planned`
    // todavía no. Así el catálogo no puede prometer algo que no está.
    let catalog = Catalog::load(repo_root().join("sandbox.config.json")).expect("catálogo válido");
    for case in &catalog.cases {
        let directory = repo_root().join(&catalog.cases_directory).join(format!("{}-{}", case.id, case.slug));
        match case.status.as_str() {
            "planned" => {}
            _ => assert!(
                directory.join("service.json").is_file(),
                "{} está en estado {} pero no existe {}",
                case.slug,
                case.status,
                directory.display()
            ),
        }
    }
}
