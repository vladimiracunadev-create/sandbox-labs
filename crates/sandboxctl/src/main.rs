mod bench;
mod escape;
mod forward;
mod service;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sandbox_core::{Catalog, DoctorReport, Evidence, ExecutionPlan, Policy, RuntimeKind, Workload};
use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

#[derive(Debug, Parser)]
#[command(name = "sandboxctl", version, about = "Controlador reproducible de Sandbox Labs")]
struct Cli {
    #[arg(long, default_value = ".")]
    root: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Doctor {
        #[arg(long)]
        json: bool,
    },
    /// Los casos del sistema y su estado.
    Cases,
    Runtimes {
        #[arg(long)]
        json: bool,
    },
    Validate {
        policy: PathBuf,
        #[arg(long)]
        workload: Option<PathBuf>,
    },
    Plan {
        #[arg(long)]
        workload: PathBuf,
        #[arg(long, default_value = "dry-run")]
        runtime: String,
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        json: bool,
    },
    Run {
        #[arg(long)]
        workload: PathBuf,
        #[arg(long, default_value = "dry-run")]
        runtime: String,
        #[arg(long)]
        policy: PathBuf,
        #[arg(long, num_args=0..=16)]
        arg: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    /// Ejecuta la suite de contención: mide qué aísla de verdad cada runtime.
    ///
    /// `plan` dice lo que el runtime declara; `escape` dice lo que hace.
    Escape {
        /// Runtime a medir; repetible. Sin él se miden todos los ejecutables.
        #[arg(long)]
        runtime: Vec<String>,
        #[arg(long, default_value = "policies/containment-audit.json")]
        policy: PathBuf,
        #[arg(long)]
        json: bool,
        /// Devuelve código 1 si alguna sonda escapa, para usarlo como gate de CI.
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        report: Option<PathBuf>,
    },
    /// Levanta, lista y baja sandboxes de larga duración.
    ///
    /// A diferencia de `run`, que ejecuta y termina, esto mantiene un servicio
    /// dentro de la jaula publicando un puerto que se puede abrir y bajar.
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
    /// Comprueba evidencias: su propia huella y los hashes que declaran.
    ///
    /// Sin argumentos revisa todas las de `evidence/runs`.
    Evidence {
        #[command(subcommand)]
        action: EvidenceAction,
    },
    /// Ejecuta escenarios de mercado de capitales.
    ///
    /// Dinero, instrumentos y participantes SIMULADOS. Sin autorización de
    /// ninguna autoridad y sin recomendaciones de inversión.
    Markets {
        #[command(subcommand)]
        action: MarketsAction,
    },
    /// Compara el coste de arranque de cada frontera con la misma carga.
    Bench {
        #[arg(long, default_value = "workloads/benign/hello")]
        workload: PathBuf,
        /// Por defecto la política de auditoría: es best-effort, así que se
        /// ejecuta en todos los runtimes y la comparación es entre iguales.
        /// Una política strict bloquearía a unos sí y a otros no.
        #[arg(long, default_value = "policies/containment-audit.json")]
        policy: PathBuf,
        #[arg(long)]
        runtime: Vec<String>,
        #[arg(long, default_value_t = 10)]
        repeat: usize,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        report: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum MarketsAction {
    /// Ejecuta los escenarios de todos los casos con código y compara cada uno
    /// con lo que declara esperar.
    Check {
        /// Un caso concreto: `CM-04`. Sin él, todos.
        #[arg(long)]
        case: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Concilia la custodia de un escenario, o de todos los de un caso.
    Reconcile {
        /// Escenario concreto; sin él se ejecutan todos los del caso CM-03.
        path: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum EvidenceAction {
    /// Recalcula la huella de cada evidencia y vuelve a hashear la política y
    /// la carga que dice haber ejecutado.
    Verify {
        /// Evidencia concreta; sin ella se revisan todas.
        path: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ServiceAction {
    /// Levanta un sandbox con el servicio dentro.
    Up {
        id: String,
        /// No esperar a que el puerto responda.
        #[arg(long)]
        detach: bool,
    },
    /// Baja el sandbox de un servicio.
    Down {
        /// Id del servicio; con `--all` se bajan todos.
        id: Option<String>,
        #[arg(long)]
        all: bool,
    },
    /// Estado de todos los servicios registrados.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Habla con un servicio, por TCP o por socket Unix.
    ///
    /// Un sandbox con `network: none` no se alcanza con curl: no tiene pila de
    /// red. Este comando entra por el socket.
    Call {
        id: String,
        #[arg(long, default_value = "GET")]
        method: String,
        #[arg(long, default_value = "/api/status")]
        path: String,
        #[arg(long)]
        body: Option<String>,
    },
    /// Últimas líneas del log de un servicio.
    Logs {
        id: String,
        #[arg(long, default_value_t = 40)]
        lines: usize,
    },
    /// Publica el puerto de un servicio empalmándolo con su socket Unix.
    ///
    /// No se invoca a mano: lo lanza `service up` como proceso aparte cuando el
    /// servicio declara `publish: proxy`. Existe como subcomando porque el
    /// reenviador tiene que sobrevivir al CLI que levantó el sandbox, igual que
    /// el sandbox mismo.
    Forward { id: String },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = canonical_or_original(&cli.root);
    // `escape` y `bench` devuelven un código de salida propio: son puertas de
    // CI, y un informe con fugas tiene que poder tumbar el build.
    let code = match cli.command {
        Command::Doctor { json } => doctor(json).map(|_| 0),
        Command::Cases => cases(&root).map(|_| 0),
        Command::Runtimes { json } => runtimes(json).map(|_| 0),
        Command::Validate { policy, workload } => validate(&root, &policy, workload.as_deref()).map(|_| 0),
        Command::Plan { workload, runtime, policy, json } => plan(&root, &workload, &runtime, &policy, json).map(|_| 0),
        Command::Run { workload, runtime, policy, arg, json } => {
            run(&root, &workload, &runtime, &policy, &arg, json).map(|_| 0)
        }
        Command::Escape { runtime, policy, json, strict, report } => escape::run(
            &root,
            &escape::EscapeOptions {
                runtimes: escape::parse_runtimes(&runtime)?,
                policy: resolve_inside(&root, &policy)?,
                json,
                strict,
                report_path: report,
            },
        ),
        Command::Service { action } => {
            let ctx = service::ServiceContext::new(&root)?;
            match action {
                ServiceAction::Up { id, detach } => service::up(&ctx, &id, !detach),
                ServiceAction::Down { id, all } => match (id, all) {
                    (_, true) => service::down_all(&ctx),
                    (Some(id), false) => service::down(&ctx, &id),
                    (None, false) => {
                        anyhow::bail!("Indica el id del servicio o usa --all")
                    }
                },
                ServiceAction::Call { id, method, path, body } => service::call(&ctx, &id, &method, &path, body),
                ServiceAction::List { json } => service::list(&ctx, json),
                ServiceAction::Logs { id, lines } => service::logs(&ctx, &id, lines),
                ServiceAction::Forward { id } => service::forward(&ctx, &id),
            }
        }
        Command::Markets { action } => match action {
            MarketsAction::Check { case, json } => markets_check(case.as_deref(), json),
            MarketsAction::Reconcile { path, json } => markets_reconcile(&root, path.as_deref(), json),
        },
        Command::Evidence { action } => match action {
            EvidenceAction::Verify { path, json } => evidence_verify(&root, path.as_deref(), json),
        },
        Command::Bench { workload, policy, runtime, repeat, json, report } => bench::run(
            &root,
            &bench::BenchOptions {
                workload: resolve_inside(&root, &workload)?,
                policy: resolve_inside(&root, &policy)?,
                runtimes: escape::parse_runtimes(&runtime)?,
                repetitions: repeat,
                json,
                report_path: report,
            },
        ),
    }?;
    if code != 0 {
        std::process::exit(code);
    }
    Ok(())
}

fn doctor(json: bool) -> Result<()> {
    let report = DoctorReport::collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    println!("Sandbox Labs doctor — {}", report.platform);
    for check in report.checks {
        println!("{} {:14} {}", if check.available { "✅" } else { "⚪" }, check.name, check.detail);
    }
    Ok(())
}

fn cases(root: &Path) -> Result<()> {
    let catalog = Catalog::load(root.join("sandbox.config.json"))?;
    println!(
        "{} v{}
",
        catalog.project.name, catalog.project.version
    );
    for case in catalog.cases {
        println!("{} {:22} :{:<6} {:10} {}", case.id, case.slug, case.port, case.status, case.idea);
    }
    Ok(())
}

fn runtimes(json_output: bool) -> Result<()> {
    let kinds = [
        RuntimeKind::DryRun,
        RuntimeKind::Native,
        RuntimeKind::Bwrap,
        RuntimeKind::Unshare,
        RuntimeKind::Gvisor,
        RuntimeKind::Kata,
        RuntimeKind::Wasi,
        RuntimeKind::Firecracker,
    ];
    let probes = kinds.into_iter().map(RuntimeKind::probe).collect::<Vec<_>>();
    if json_output {
        println!("{}", serde_json::to_string_pretty(&probes)?);
    } else {
        for probe in probes {
            println!("{} {:12} {}", if probe.available { "✅" } else { "⚪" }, probe.id, probe.version);
        }
    }
    Ok(())
}

fn validate(root: &Path, policy: &Path, workload: Option<&Path>) -> Result<()> {
    let policy_path = resolve_inside(root, policy)?;
    let policy = Policy::load(&policy_path)?;
    println!("✅ Política válida: {}", policy.id);
    if let Some(path) = workload {
        let workload = Workload::load(resolve_inside(root, path)?)?;
        println!("✅ Carga válida: {}", workload.id);
    }
    Ok(())
}

fn plan(root: &Path, workload: &Path, runtime: &str, policy: &Path, json_output: bool) -> Result<()> {
    let (_, _, _, plan) = prepare(root, workload, runtime, policy)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    } else {
        print_plan(&plan);
    }
    Ok(())
}

fn run(
    root: &Path,
    workload_path: &Path,
    runtime: &str,
    policy_path: &Path,
    args: &[String],
    json_output: bool,
) -> Result<()> {
    let (workload, policy, policy_file, plan) = prepare(root, workload_path, runtime, policy_path)?;
    let policy_hash = Policy::hash(&policy_file)?;
    let workload_hash = workload.hash()?;
    let catalog = Catalog::load(root.join("sandbox.config.json"))?;
    let evidence_dir = root.join(&catalog.project.evidence_directory);
    // La clave de firma vive en el directorio de datos, fuera del repositorio.
    let data_root = root.join(&catalog.project.data_directory);
    let evidence = if plan.runtime == RuntimeKind::DryRun || !plan.executable {
        Evidence::planned(&plan, &policy, &policy_hash, &workload, &workload_hash)
    } else {
        let outcome = sandbox_runtimes::execute(&plan, &policy, &workload, args)?;
        Evidence::executed(&plan, &policy, &policy_hash, &workload, &workload_hash, &outcome)
    };
    let path = evidence.write(evidence_dir, Some(&data_root))?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&evidence)?);
    } else {
        print_plan(&plan);
        println!("\nEstado: {:?}\nEvidencia: {}", evidence.status, path.display());
    }
    Ok(())
}

fn prepare(
    root: &Path,
    workload: &Path,
    runtime: &str,
    policy: &Path,
) -> Result<(Workload, Policy, PathBuf, ExecutionPlan)> {
    let workload = Workload::load(resolve_inside(root, workload)?)?;
    let policy_file = resolve_inside(root, policy)?;
    let policy = Policy::load(&policy_file)?;
    let runtime = RuntimeKind::from_str(runtime)?;
    let plan = ExecutionPlan::build(runtime, &workload, &policy)?;
    Ok((workload, policy, policy_file, plan))
}

fn print_plan(plan: &ExecutionPlan) {
    println!("Plan de ejecución:");
    for step in &plan.steps {
        println!("  - {step}");
    }
    if let Some(reason) = &plan.block_reason {
        println!("  ⚠ {reason}");
    }
}

fn resolve_inside(root: &Path, candidate: &Path) -> Result<PathBuf> {
    let joined = if candidate.is_absolute() { candidate.to_path_buf() } else { root.join(candidate) };
    let canonical = joined.canonicalize().with_context(|| format!("No se pudo resolver {}", joined.display()))?;
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    if !canonical.starts_with(&root) {
        anyhow::bail!("Ruta fuera del repositorio: {}", canonical.display());
    }
    Ok(canonical)
}
fn canonical_or_original(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// `evidence verify`: comprueba una evidencia, o todas las de `evidence/runs`.
///
/// Devuelve código 1 si alguna comprobación falla, para poder usarlo como
/// puerta. Las comprobaciones que **no se pudieron hacer** —la política ya no
/// existe, la evidencia es anterior al sellado— se informan aparte y no cuentan
/// como aprobado: un informe que no se puede verificar no es un informe
/// verificado.
fn evidence_verify(root: &Path, path: Option<&Path>, json: bool) -> Result<i32> {
    let files = match path {
        Some(single) => vec![resolve_inside(root, single)?],
        None => {
            let directory = root.join("evidence").join("runs");
            let mut found: Vec<PathBuf> = std::fs::read_dir(&directory)
                .with_context(|| format!("No se pudo leer {}", directory.display()))?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|value| value == "json"))
                .collect();
            found.sort();
            found
        }
    };

    let mut reports = Vec::new();
    for file in &files {
        reports.push(sandbox_core::evidence::verify(file, root)?);
    }
    // La cadena se comprueba sobre el conjunto ordenado por momento de
    // ejecución: es una propiedad de la serie, no de un informe suelto.
    reports.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    let chain = if path.is_none() { sandbox_core::evidence::verify_chain(&reports) } else { Vec::new() };

    if json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({"evidences": reports, "chain": chain}))?);
    } else if reports.is_empty() {
        println!("No hay evidencias que comprobar en evidence/runs.");
    } else {
        for report in &reports {
            let mark = if report.passed() { "✅" } else { "❌" };
            println!("{mark} {} · {}", report.run_id, report.path);
            for check in &report.checks {
                let symbol = match check.passed {
                    Some(true) => "  ✓",
                    Some(false) => "  ✗",
                    None => "  ⚠",
                };
                println!("{symbol} {}: {}", check.name, check.detail);
            }
        }
        for check in &chain {
            let symbol = match check.passed {
                Some(true) => "✓",
                Some(false) => "✗",
                None => "⚠",
            };
            println!("{symbol} {}: {}", check.name, check.detail);
        }
        let failed = reports.iter().filter(|report| !report.passed()).count()
            + chain.iter().filter(|check| check.passed == Some(false)).count();
        let unverifiable: usize = reports.iter().map(sandbox_core::evidence::VerificationReport::unverifiable).sum();
        println!(
            "\n{} evidencia(s) · {failed} con fallos · {unverifiable} comprobación(es) que no pudieron hacerse",
            reports.len()
        );
    }

    let broken = reports.iter().any(|report| !report.passed()) || chain.iter().any(|check| check.passed == Some(false));
    Ok(i32::from(broken))
}

/// `markets check`: ejecuta los escenarios de todos los casos de mercado de
/// capitales que tienen código.
///
/// Devuelve 1 si alguno se desvía de lo que declara. Un escenario que deja de
/// detectar lo que venía a detectar es un escenario roto, y dejarlo pasar lo
/// convertiría en decoración.
fn markets_check(case: Option<&str>, json: bool) -> Result<i32> {
    let reports: Vec<sandbox_markets::CaseReport> = sandbox_markets::cases::all()
        .into_iter()
        .filter(|report| case.is_none_or(|wanted| report.id.eq_ignore_ascii_case(wanted)))
        .collect();

    if reports.is_empty() {
        anyhow::bail!(
            "No hay ningún caso con ese identificador. Los que tienen código: {}",
            sandbox_markets::cases::all().iter().map(|report| report.id).collect::<Vec<_>>().join(", ")
        );
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&reports)?);
    } else {
        println!(
            "Mercado de capitales · {} caso(s) con código
",
            reports.len()
        );
        println!(
            "⚠ Dinero, instrumentos, participantes y datos SIMULADOS. Sin autorización de ninguna autoridad
  y sin recomendaciones de inversión.
"
        );
        for report in &reports {
            let mark = if report.passed() { "✅" } else { "❌" };
            println!("{mark} {} · {} [{}]", report.id, report.title, report.maturity.label());
            for check in &report.checks {
                if check.passed() {
                    println!("   · {}", check.scenario);
                } else {
                    println!("   ❌ {}", check.scenario);
                    println!("      esperaba «{}» y obtuve «{}»", check.expected, check.actual);
                }
            }
        }
        let checks: usize = reports.iter().map(|report| report.checks.len()).sum();
        let broken = reports.iter().filter(|report| !report.passed()).count();
        println!(
            "
{} caso(s) · {checks} escenario(s) · {broken} que no hacen lo que declaran",
            reports.len()
        );
    }

    Ok(i32::from(reports.iter().any(|report| !report.passed())))
}

/// `markets reconcile`: ejecuta escenarios de custodia y compara con lo que
/// cada uno declara esperar.
///
/// Devuelve 1 si algún escenario se desvía de su expectativa. Un escenario
/// adverso que deja de detectar lo que venía a detectar es un escenario roto, y
/// dejarlo pasar lo convertiría en decoración.
fn markets_reconcile(root: &Path, path: Option<&Path>, json: bool) -> Result<i32> {
    let files = match path {
        Some(single) => vec![resolve_inside(root, single)?],
        None => {
            let directory = root.join("domains/capital-markets/cases/03-asset-custody/scenarios");
            let mut found: Vec<PathBuf> = std::fs::read_dir(&directory)
                .with_context(|| format!("No se pudo leer {}", directory.display()))?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|value| value == "json"))
                .collect();
            found.sort();
            found
        }
    };

    let mut outcomes = Vec::new();
    for file in &files {
        let scenario = sandbox_markets::Scenario::load(file).map_err(anyhow::Error::msg)?;
        outcomes.push(scenario.run());
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&outcomes)?);
    } else {
        println!(
            "Custodia y segregación · {} escenario(s)
",
            outcomes.len()
        );
        println!(
            "⚠ Activos, clientes y saldos SIMULADOS. Sin autorización de ninguna autoridad.
"
        );
        for outcome in &outcomes {
            let mark = if outcome.matches_expectation { "✅" } else { "❌" };
            let state = if outcome.reconciled { "conciliado" } else { "CON HALLAZGOS" };
            println!("{mark} {} · {} · {state}", outcome.id, outcome.title);
            for finding in &outcome.findings {
                println!("   · {finding}");
            }
            for deviation in &outcome.deviations {
                println!("   ⚠ {deviation}");
            }
        }
        let broken = outcomes.iter().filter(|outcome| !outcome.matches_expectation).count();
        println!(
            "
{} escenario(s) · {broken} que no hacen lo que declaran",
            outcomes.len()
        );
    }

    Ok(i32::from(outcomes.iter().any(|outcome| !outcome.matches_expectation)))
}
