//! Subcomando `escape`: ejecuta la suite de contención y publica la matriz.
//!
//! La diferencia con `plan` es la que da sentido al proyecto: `plan` dice lo
//! que el runtime **declara**; `escape` dice lo que el runtime **hace**.

use anyhow::{Context, Result};
use sandbox_core::{
    escape::{parse_probe_lines, verdict_from_lines},
    Catalog, EscapeSuite, ExecutionPlan, Policy, Probe, ProbeResult, RuntimeKind, RuntimeReport, SuiteReport, Verdict,
    Workload,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    str::FromStr,
    time::Instant,
};

/// Runtimes que se miden cuando no se pide uno concreto.
const DEFAULT_RUNTIMES: [RuntimeKind; 4] =
    [RuntimeKind::Native, RuntimeKind::Bwrap, RuntimeKind::Unshare, RuntimeKind::Wasi];

pub struct EscapeOptions {
    pub runtimes: Vec<RuntimeKind>,
    pub policy: PathBuf,
    pub json: bool,
    /// Falla con código distinto de cero si alguna sonda escapa. Es lo que
    /// convierte la suite en una puerta de CI en vez de un informe decorativo.
    pub strict: bool,
    pub report_path: Option<PathBuf>,
}

pub fn run(root: &Path, options: &EscapeOptions) -> Result<i32> {
    let suite = EscapeSuite::load(root.join("escape-suite").join("suite.json"))?;
    let policy = Policy::load(&options.policy)?;
    let catalog = Catalog::load(root.join("sandbox.config.json"))?;
    let workloads = index_workloads(root)?;

    let runtimes = if options.runtimes.is_empty() { DEFAULT_RUNTIMES.to_vec() } else { options.runtimes.clone() };

    let mut reports = Vec::new();
    for runtime in runtimes {
        reports.push(measure_runtime(runtime, &suite, &policy, &workloads, &options.policy)?);
    }

    let report = SuiteReport {
        schema_version: "1.0".into(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        host: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        policy: policy.id.clone(),
        reports,
    };

    if options.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_matrix(&suite, &report, &catalog);
    }

    if let Some(path) = &options.report_path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(&report)?)
            .with_context(|| format!("No se pudo escribir {}", path.display()))?;
        if !options.json {
            println!("\nInforme: {}", path.display());
        }
    }

    if options.strict && report.escaped_total() > 0 {
        eprintln!(
            "\n❌ {} sonda(s) escaparon; {} de ellas bajo un control declarado.",
            report.escaped_total(),
            report.false_assurances_total()
        );
        return Ok(1);
    }
    Ok(0)
}

fn measure_runtime(
    runtime: RuntimeKind,
    suite: &EscapeSuite,
    policy: &Policy,
    workloads: &BTreeMap<String, Workload>,
    policy_path: &Path,
) -> Result<RuntimeReport> {
    let probe = runtime.probe();
    let mut results = Vec::new();

    for entry in &suite.probes {
        results.push(measure_probe(runtime, entry, policy, workloads, probe.available));
    }

    Ok(RuntimeReport {
        runtime: runtime.to_string(),
        available: probe.available,
        policy: policy_path.display().to_string(),
        results,
    })
}

fn measure_probe(
    runtime: RuntimeKind,
    entry: &Probe,
    policy: &Policy,
    workloads: &BTreeMap<String, Workload>,
    runtime_available: bool,
) -> ProbeResult {
    let declared = runtime.supported_controls(policy).contains(&entry.control);
    let mut result = ProbeResult {
        probe: entry.id.clone(),
        dimension: entry.dimension.clone(),
        control: entry.control.clone(),
        verdict: Verdict::Inconclusive,
        declared,
        detail: String::new(),
        duration_ms: 0,
        lines: vec![],
    };

    let Some(workload) = workloads.get(&entry.workload) else {
        result.detail = format!("carga no registrada: {}", entry.workload);
        return result;
    };

    if !runtime_available {
        result.verdict = Verdict::NotApplicable;
        result.detail = "runtime no disponible en este host".into();
        return result;
    }

    let plan = match ExecutionPlan::build(runtime, workload, policy) {
        Ok(value) => value,
        Err(error) => {
            result.detail = format!("no se pudo compilar el plan: {error}");
            return result;
        }
    };

    if !plan.executable {
        // Un plan bloqueado no es una fuga ni una contención medida: es que la
        // política impidió la ejecución. Se distingue para no inflar el informe.
        result.verdict = Verdict::NotApplicable;
        result.detail = plan.block_reason.clone().unwrap_or_else(|| "plan no ejecutable".into());
        return result;
    }

    let args: Vec<String> = EscapeSuite::argument_value(entry, policy).into_iter().collect();
    let started = Instant::now();
    let outcome = sandbox_runtimes::execute(&plan, policy, workload, &args);
    result.duration_ms = started.elapsed().as_millis();

    match outcome {
        Ok(value) => {
            let combined = format!("{}\n{}", value.stdout, value.stderr);
            result.lines = parse_probe_lines(&combined);
            result.verdict = verdict_from_lines(&result.lines);
            result.detail = if result.lines.is_empty() {
                // Sin líneas de sonda pero con estado de terminación: el runtime
                // pudo haber matado el proceso, que también es contención — solo
                // que no medida por la sonda.
                format!("sin salida de sonda (estado {}, código {:?})", value.status, value.exit_code)
            } else {
                result
                    .lines
                    .iter()
                    .filter(|line| line.result != "contained")
                    .map(|line| line.detail.clone())
                    .next()
                    .unwrap_or_else(|| result.lines[0].detail.clone())
            };
            // Un timeout o una muerte por OOM sin salida es contención efectiva.
            if result.lines.is_empty() && (value.status == "timeout" || value.exit_code.is_none()) {
                result.verdict = Verdict::Contained;
                result.detail = format!("el runtime terminó el proceso ({})", value.status);
            }
        }
        Err(error) => {
            result.detail = format!("la ejecución falló: {error}");
        }
    }

    result
}

fn print_matrix(suite: &EscapeSuite, report: &SuiteReport, catalog: &Catalog) {
    println!("Suite de contención — política {} · host {}", report.policy, report.host);
    println!("{}\n", suite.description);

    let width = suite.probes.iter().map(|probe| probe.id.len()).max().unwrap_or(20).max(20);

    print!("{:<width$}", "DIMENSIÓN / SONDA", width = width + 2);
    for entry in &report.reports {
        print!("{:>14}", entry.runtime);
    }
    println!();
    println!("{}", "─".repeat(width + 2 + 14 * report.reports.len()));

    for probe in &suite.probes {
        print!("{:<width$}", probe.id, width = width + 2);
        for entry in &report.reports {
            let cell = entry
                .results
                .iter()
                .find(|value| value.probe == probe.id)
                .map(|value| {
                    // Una falsa garantía se marca distinto: es el hallazgo que
                    // más importa y no puede perderse entre los demás.
                    if value.is_false_assurance() {
                        "❌ DECLARADO".to_string()
                    } else {
                        value.verdict.symbol().to_string()
                    }
                })
                .unwrap_or_else(|| "?".into());
            print!("{cell:>14}");
        }
        println!();
    }

    println!("\n✅ contenido   ❌ escapó   ⚠️ no concluyente   — no aplica");
    println!("«❌ DECLARADO» = el runtime declara el control y la sonda demostró que no lo aplica.\n");

    for entry in &report.reports {
        let status = if !entry.available {
            "no disponible en este host".to_string()
        } else if entry.passed() {
            format!("sin fugas ({} contenidas)", entry.count(Verdict::Contained))
        } else {
            format!("{} FUGA(S), {} con control declarado", entry.count(Verdict::Escaped), entry.false_assurances())
        };
        let declared = catalog
            .runtimes
            .iter()
            .find(|value| value.id == entry.runtime)
            .map(|value| value.status.as_str())
            .unwrap_or("desconocido");
        println!("  {:<12} [{}] {}", entry.runtime, declared, status);
    }

    let escaped: Vec<&ProbeResult> =
        report.reports.iter().flat_map(|r| r.results.iter()).filter(|v| v.verdict == Verdict::Escaped).collect();
    if !escaped.is_empty() {
        println!("\nDetalle de las fugas:");
        for value in escaped {
            let dimension = suite.dimension(&value.dimension).map(|d| d.why.as_str()).unwrap_or("");
            println!("  · {} — {}", value.probe, value.detail);
            if !dimension.is_empty() {
                println!("    por qué importa: {dimension}");
            }
        }
    }
}

/// Indexa por id todas las cargas registradas del repositorio.
pub fn index_workloads(root: &Path) -> Result<BTreeMap<String, Workload>> {
    let mut found = BTreeMap::new();
    let mut stack = vec![root.join("workloads")];
    while let Some(directory) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|name| name == "manifest.json") {
                if let Ok(workload) = Workload::load(&path) {
                    found.insert(workload.id.clone(), workload);
                }
            }
        }
    }
    Ok(found)
}

/// Convierte la lista de runtimes de la línea de comandos en tipos conocidos.
pub fn parse_runtimes(values: &[String]) -> Result<Vec<RuntimeKind>> {
    values.iter().map(|value| RuntimeKind::from_str(value)).collect()
}
