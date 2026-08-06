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
    let evidence_dir = root.join(catalog.project.evidence_directory);
    let evidence = if plan.runtime == RuntimeKind::DryRun || !plan.executable {
        Evidence::planned(&plan, &policy, &policy_hash, &workload, &workload_hash)
    } else {
        let outcome = sandbox_runtimes::execute(&plan, &policy, &workload, args)?;
        Evidence::executed(&plan, &policy, &policy_hash, &workload, &workload_hash, &outcome)
    };
    let path = evidence.write(evidence_dir)?;
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
