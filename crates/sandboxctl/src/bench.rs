//! Subcomando `bench`: mide el precio de cada frontera de aislamiento.

use anyhow::Result;
use sandbox_core::{
    bench::{BenchmarkReport, RuntimeBenchmark, Stats},
    ExecutionPlan, Policy, RuntimeKind, Workload,
};
use std::{
    path::{Path, PathBuf},
    time::Instant,
};

const DEFAULT_RUNTIMES: [RuntimeKind; 4] =
    [RuntimeKind::Native, RuntimeKind::Bwrap, RuntimeKind::Unshare, RuntimeKind::Wasi];

pub struct BenchOptions {
    pub workload: PathBuf,
    pub policy: PathBuf,
    pub runtimes: Vec<RuntimeKind>,
    pub repetitions: usize,
    pub json: bool,
    pub report_path: Option<PathBuf>,
}

pub fn run(_root: &Path, options: &BenchOptions) -> Result<i32> {
    let workload = Workload::load(&options.workload)?;
    let policy = Policy::load(&options.policy)?;
    let runtimes = if options.runtimes.is_empty() { DEFAULT_RUNTIMES.to_vec() } else { options.runtimes.clone() };
    let repetitions = options.repetitions.clamp(1, 100);

    let mut entries = Vec::new();
    for runtime in runtimes {
        entries.push(measure(runtime, &workload, &policy, repetitions));
    }

    let report = BenchmarkReport {
        schema_version: "1.0".into(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        host: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        workload: workload.id.clone(),
        policy: policy.id.clone(),
        repetitions,
        runtimes: entries,
    };

    if options.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_table(&report);
    }

    if let Some(path) = &options.report_path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
        if !options.json {
            println!("\nInforme: {}", path.display());
        }
    }
    Ok(0)
}

fn measure(runtime: RuntimeKind, workload: &Workload, policy: &Policy, repetitions: usize) -> RuntimeBenchmark {
    let probe = runtime.probe();
    if !probe.available {
        return RuntimeBenchmark {
            runtime: runtime.to_string(),
            available: false,
            stats: None,
            failures: 0,
            note: format!("no disponible: {}", probe.detail),
        };
    }

    let plan = match ExecutionPlan::build(runtime, workload, policy) {
        Ok(value) => value,
        Err(error) => {
            return RuntimeBenchmark {
                runtime: runtime.to_string(),
                available: true,
                stats: None,
                failures: 0,
                note: format!("no se pudo compilar el plan: {error}"),
            }
        }
    };

    if !plan.executable {
        return RuntimeBenchmark {
            runtime: runtime.to_string(),
            available: true,
            stats: None,
            failures: 0,
            note: plan.block_reason.clone().unwrap_or_else(|| "plan no ejecutable".into()),
        };
    }

    let mut samples = Vec::with_capacity(repetitions);
    let mut failures = 0;

    // Una repetición de calentamiento que no se mide: la primera ejecución paga
    // cachés de página y del ejecutable, y falsearía la mediana.
    let _ = sandbox_runtimes::execute(&plan, policy, workload, &[]);

    for _ in 0..repetitions {
        let started = Instant::now();
        match sandbox_runtimes::execute(&plan, policy, workload, &[]) {
            Ok(outcome) if outcome.status == "completed" => samples.push(started.elapsed().as_secs_f64() * 1000.0),
            _ => failures += 1,
        }
    }

    RuntimeBenchmark {
        runtime: runtime.to_string(),
        available: true,
        stats: Stats::from_samples(&samples),
        failures,
        note: if failures > 0 { format!("{failures} repetición(es) fallaron") } else { String::new() },
    }
}

fn print_table(report: &BenchmarkReport) {
    println!(
        "Comparativa de runtimes — carga {} · política {} · {} repeticiones · host {}",
        report.workload, report.policy, report.repetitions, report.host
    );
    println!("El sobrecoste se expresa contra el runtime más rápido que pudo medirse.\n");

    println!(
        "{:<12} {:>9} {:>9} {:>9} {:>9} {:>11}  NOTA",
        "RUNTIME", "p50 ms", "p95 ms", "min ms", "max ms", "SOBRECOSTE"
    );
    println!("{}", "─".repeat(96));

    for entry in &report.runtimes {
        match &entry.stats {
            Some(stats) => {
                let overhead = report.overhead(entry).map(|value| format!("{value:.2}×")).unwrap_or_else(|| "—".into());
                println!(
                    "{:<12} {:>9.2} {:>9.2} {:>9.2} {:>9.2} {:>11}  {}",
                    entry.runtime, stats.p50_ms, stats.p95_ms, stats.min_ms, stats.max_ms, overhead, entry.note
                );
            }
            // Sin medidas se imprime un guion en cada columna numérica: una
            // fila vacía se confundiría con un cero.
            None => println!(
                "{:<12} {:>9} {:>9} {:>9} {:>9} {:>11}  {}",
                entry.runtime, "—", "—", "—", "—", "—", entry.note
            ),
        }
    }

    if report.baseline_p50().is_none() {
        println!("\n⚠️  Ningún runtime pudo medirse en este host: revisa `sandboxctl doctor`.");
    }
}
