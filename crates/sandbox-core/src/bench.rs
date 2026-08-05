//! Comparativa entre runtimes.
//!
//! Aislar cuesta. Un namespace de red se monta rápido; una microVM arranca un
//! kernel. Elegir frontera sin saber el precio lleva a dos errores simétricos:
//! pagar de más por una carga inofensiva, o quedarse corto con una que no lo es.
//!
//! Este módulo mide ese precio con la **misma carga y la misma política** en
//! todos los runtimes disponibles, para que la comparación signifique algo.

use serde::{Deserialize, Serialize};

/// Estadísticas de una serie de mediciones, en milisegundos.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub samples: usize,
    pub min_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
    pub mean_ms: f64,
}

impl Stats {
    /// Calcula las estadísticas de una serie. Devuelve `None` si está vacía.
    ///
    /// Se reporta la mediana y el p95, no solo la media: en tiempos de arranque
    /// la cola importa más que el promedio, y una media sola esconde justo el
    /// caso que hará esperar al usuario.
    pub fn from_samples(values: &[f64]) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        Some(Self {
            samples: sorted.len(),
            min_ms: sorted[0],
            p50_ms: percentile(&sorted, 50.0),
            p95_ms: percentile(&sorted, 95.0),
            max_ms: sorted[sorted.len() - 1],
            mean_ms: sorted.iter().sum::<f64>() / sorted.len() as f64,
        })
    }
}

/// Percentil por el método del vecino más cercano sobre una serie ya ordenada.
fn percentile(sorted: &[f64], percentile: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let rank = (percentile / 100.0) * (sorted.len() - 1) as f64;
    let index = rank.round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeBenchmark {
    pub runtime: String,
    pub available: bool,
    pub stats: Option<Stats>,
    /// Cuántas repeticiones fallaron. Un runtime rápido que falla la mitad de
    /// las veces no es rápido, es inestable, y el número debe verse.
    pub failures: usize,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkReport {
    pub schema_version: String,
    pub generated_at: String,
    pub host: String,
    pub workload: String,
    pub policy: String,
    pub repetitions: usize,
    pub runtimes: Vec<RuntimeBenchmark>,
}

impl BenchmarkReport {
    /// Mediana más baja entre los runtimes que midieron algo: la referencia
    /// contra la que se expresa el sobrecoste de los demás.
    pub fn baseline_p50(&self) -> Option<f64> {
        self.runtimes
            .iter()
            .filter_map(|entry| entry.stats.as_ref().map(|stats| stats.p50_ms))
            .filter(|value| *value > 0.0)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Sobrecoste de un runtime respecto a la referencia, en múltiplos.
    pub fn overhead(&self, entry: &RuntimeBenchmark) -> Option<f64> {
        let baseline = self.baseline_p50()?;
        let stats = entry.stats.as_ref()?;
        if baseline <= 0.0 {
            return None;
        }
        Some(stats.p50_ms / baseline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_nothing_for_an_empty_series() {
        assert!(Stats::from_samples(&[]).is_none());
    }

    #[test]
    fn a_single_sample_is_every_statistic() {
        let stats = Stats::from_samples(&[42.0]).expect("una muestra");
        assert_eq!(stats.samples, 1);
        assert_eq!(stats.min_ms, 42.0);
        assert_eq!(stats.p50_ms, 42.0);
        assert_eq!(stats.p95_ms, 42.0);
        assert_eq!(stats.max_ms, 42.0);
    }

    #[test]
    fn computes_median_and_tail() {
        let values: Vec<f64> = (1..=100).map(f64::from).collect();
        let stats = Stats::from_samples(&values).expect("cien muestras");
        assert_eq!(stats.min_ms, 1.0);
        assert_eq!(stats.max_ms, 100.0);
        assert!((stats.mean_ms - 50.5).abs() < 0.001);
        assert!((stats.p50_ms - 50.0).abs() <= 1.0, "p50 fue {}", stats.p50_ms);
        assert!((stats.p95_ms - 95.0).abs() <= 1.0, "p95 fue {}", stats.p95_ms);
    }

    #[test]
    fn the_tail_is_not_hidden_by_the_mean() {
        // 19 medidas rápidas y una lenta: la media apenas se mueve, el p95 sí.
        let mut values = vec![10.0; 19];
        values.push(1000.0);
        let stats = Stats::from_samples(&values).expect("veinte muestras");
        assert!(stats.mean_ms < 60.0, "la media esconde la cola: {}", stats.mean_ms);
        assert_eq!(stats.max_ms, 1000.0, "el máximo sí la muestra");
    }

    fn report(entries: Vec<(&str, Option<f64>)>) -> BenchmarkReport {
        BenchmarkReport {
            schema_version: "1.0".into(),
            generated_at: "1970-01-01T00:00:00Z".into(),
            host: "test".into(),
            workload: "hello".into(),
            policy: "minimal".into(),
            repetitions: 1,
            runtimes: entries
                .into_iter()
                .map(|(name, p50)| RuntimeBenchmark {
                    runtime: name.into(),
                    available: p50.is_some(),
                    stats: p50.map(|value| Stats::from_samples(&[value]).expect("una muestra")),
                    failures: 0,
                    note: String::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn expresses_overhead_against_the_fastest_runtime() {
        let value = report(vec![("native", Some(10.0)), ("unshare", Some(30.0)), ("gvisor", None)]);
        assert_eq!(value.baseline_p50(), Some(10.0));

        let unshare = &value.runtimes[1];
        assert!((value.overhead(unshare).expect("sobrecoste") - 3.0).abs() < 0.001);

        let gvisor = &value.runtimes[2];
        assert!(value.overhead(gvisor).is_none(), "un runtime sin medidas no tiene sobrecoste");
    }

    #[test]
    fn has_no_baseline_when_nothing_could_be_measured() {
        let value = report(vec![("gvisor", None), ("kata", None)]);
        assert!(value.baseline_p50().is_none());
    }
}
