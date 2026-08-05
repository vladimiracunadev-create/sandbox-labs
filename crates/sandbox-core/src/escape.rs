//! Suite de contención.
//!
//! El resto del sistema *planifica* aislamiento: cruza lo que la política pide
//! con lo que el runtime declara. Este módulo hace la pregunta que ningún plan
//! puede responder — **¿el control funciona de verdad en este host?** — y la
//! contesta ejecutando sondas que intentan salirse.
//!
//! Un runtime puede declarar `network` y no cortarla porque falta un binario,
//! el kernel no lo permite o la política se compiló mal. La diferencia entre
//! `Declared` y `Contained` es exactamente el valor de este módulo.

use crate::Policy;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

/// Veredicto de una sonda sobre una dimensión de aislamiento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// La sonda intentó salirse y no pudo: el control funciona.
    Contained,
    /// La sonda se salió: el control no está aplicándose en este host.
    Escaped,
    /// La sonda no llegó a medir (runtime ausente, plan bloqueado, error).
    Inconclusive,
    /// El runtime no ejecuta cargas (`dry-run`, `documented`, `manual`).
    NotApplicable,
}

impl Verdict {
    pub fn symbol(self) -> &'static str {
        match self {
            Self::Contained => "✅",
            Self::Escaped => "❌",
            Self::Inconclusive => "⚠️",
            Self::NotApplicable => "—",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Contained => "contenido",
            Self::Escaped => "ESCAPÓ",
            Self::Inconclusive => "no concluyente",
            Self::NotApplicable => "no aplica",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimension {
    pub id: String,
    pub label: String,
    /// Por qué importa esta dimensión. Se imprime en el informe: un resultado
    /// sin consecuencia explicada no ayuda a decidir nada.
    pub why: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Probe {
    pub id: String,
    pub dimension: String,
    pub workload: String,
    pub control: String,
    /// Campo de `policy.resources` que se pasa como argumento a la sonda, para
    /// que mida contra el presupuesto real y no contra una constante.
    pub argument: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscapeSuite {
    #[serde(default, rename = "$schema")]
    pub schema: Option<String>,
    pub version: String,
    pub description: String,
    pub dimensions: Vec<Dimension>,
    pub probes: Vec<Probe>,
}

impl EscapeSuite {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let content =
            fs::read_to_string(path).with_context(|| format!("No se pudo leer la suite: {}", path.display()))?;
        let suite: EscapeSuite =
            serde_json::from_str(&content).with_context(|| format!("Suite JSON inválida en {}", path.display()))?;
        suite.validate()?;
        Ok(suite)
    }

    pub fn validate(&self) -> Result<()> {
        let known: Vec<&str> = self.dimensions.iter().map(|value| value.id.as_str()).collect();
        for probe in &self.probes {
            if !known.contains(&probe.dimension.as_str()) {
                anyhow::bail!("La sonda {} declara una dimensión desconocida: {}", probe.id, probe.dimension);
            }
        }
        if self.probes.is_empty() {
            anyhow::bail!("La suite no declara ninguna sonda");
        }
        Ok(())
    }

    pub fn dimension(&self, id: &str) -> Option<&Dimension> {
        self.dimensions.iter().find(|value| value.id == id)
    }

    /// Argumento que recibe la sonda, tomado del presupuesto real de la política.
    pub fn argument_value(probe: &Probe, policy: &Policy) -> Option<String> {
        match probe.argument.as_deref() {
            Some("memoryMb") => Some(policy.resources.memory_mb.to_string()),
            Some("processes") => Some(policy.resources.processes.to_string()),
            Some("openFiles") => Some(policy.resources.open_files.to_string()),
            Some("timeoutSeconds") => Some(policy.resources.timeout_seconds.to_string()),
            _ => None,
        }
    }
}

/// Línea de salida de una sonda:
/// `probe=<id> dimension=<dim> result=<contained|escaped|error> detail=<texto>`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeLine {
    pub probe: String,
    pub dimension: String,
    pub result: String,
    pub detail: String,
}

/// Extrae las líneas con el contrato de sonda de una salida arbitraria.
///
/// Se parsea en vez de mirar el código de salida porque una sonda puede
/// reportar varias dimensiones y porque el runtime puede matar el proceso
/// (OOM killer) dejando un código que no dice nada útil.
pub fn parse_probe_lines(output: &str) -> Vec<ProbeLine> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if !line.starts_with("probe=") {
                return None;
            }
            let mut fields: BTreeMap<&str, String> = BTreeMap::new();
            // `detail` es el último campo y puede contener espacios, así que se
            // corta por las claves conocidas en vez de por espacios sueltos.
            let mut rest = line;
            for key in ["probe=", "dimension=", "result=", "detail="] {
                let Some(start) = rest.find(key) else { continue };
                let after = &rest[start + key.len()..];
                let value = if key == "detail=" {
                    after.to_string()
                } else {
                    after.split_whitespace().next().unwrap_or_default().to_string()
                };
                fields.insert(key.trim_end_matches('='), value);
                rest = after;
            }
            Some(ProbeLine {
                probe: fields.get("probe").cloned().unwrap_or_default(),
                dimension: fields.get("dimension").cloned().unwrap_or_default(),
                result: fields.get("result").cloned().unwrap_or_default(),
                detail: fields.get("detail").cloned().unwrap_or_default(),
            })
        })
        .collect()
}

/// Reduce las líneas de una sonda a un único veredicto.
///
/// Una sola dimensión escapada basta para que la sonda entera se considere
/// escapada: contener cuatro de cinco caminos no es contener.
pub fn verdict_from_lines(lines: &[ProbeLine]) -> Verdict {
    if lines.is_empty() {
        return Verdict::Inconclusive;
    }
    if lines.iter().any(|line| line.result == "escaped") {
        return Verdict::Escaped;
    }
    if lines.iter().any(|line| line.result == "error") {
        return Verdict::Inconclusive;
    }
    if lines.iter().all(|line| line.result == "contained") {
        return Verdict::Contained;
    }
    Verdict::Inconclusive
}

/// Resultado de una sonda bajo un runtime concreto.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeResult {
    pub probe: String,
    pub dimension: String,
    pub control: String,
    pub verdict: Verdict,
    /// El runtime declaraba aplicar este control (según el plan compilado).
    pub declared: bool,
    pub detail: String,
    pub duration_ms: u128,
    pub lines: Vec<ProbeLine>,
}

impl ProbeResult {
    /// El caso más peligroso de todos: el sistema **dice** que aplica el
    /// control y la sonda demuestra que no. Peor que no declararlo, porque
    /// invita a confiar.
    pub fn is_false_assurance(&self) -> bool {
        self.declared && self.verdict == Verdict::Escaped
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeReport {
    pub runtime: String,
    pub available: bool,
    pub policy: String,
    pub results: Vec<ProbeResult>,
}

impl RuntimeReport {
    pub fn count(&self, verdict: Verdict) -> usize {
        self.results.iter().filter(|value| value.verdict == verdict).count()
    }

    pub fn false_assurances(&self) -> usize {
        self.results.iter().filter(|value| value.is_false_assurance()).count()
    }

    /// Un runtime «pasa» si no escapó por ninguna dimensión medible.
    pub fn passed(&self) -> bool {
        self.count(Verdict::Escaped) == 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuiteReport {
    pub schema_version: String,
    pub generated_at: String,
    pub host: String,
    pub policy: String,
    pub reports: Vec<RuntimeReport>,
}

impl SuiteReport {
    pub fn escaped_total(&self) -> usize {
        self.reports.iter().map(|report| report.count(Verdict::Escaped)).sum()
    }

    pub fn false_assurances_total(&self) -> usize {
        self.reports.iter().map(RuntimeReport::false_assurances).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_well_formed_probe_line() {
        let lines = parse_probe_lines("probe=network-egress dimension=network result=contained detail=sin salida TCP");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].probe, "network-egress");
        assert_eq!(lines[0].dimension, "network");
        assert_eq!(lines[0].result, "contained");
        assert_eq!(lines[0].detail, "sin salida TCP", "el detalle conserva los espacios");
    }

    #[test]
    fn ignores_output_that_is_not_a_probe_line() {
        let output = "arrancando\nprobe=a dimension=b result=contained detail=ok\nlisto\n";
        assert_eq!(parse_probe_lines(output).len(), 1);
    }

    #[test]
    fn collects_every_dimension_a_probe_reports() {
        let output = concat!(
            "probe=filesystem-read dimension=filesystem result=contained detail=nada legible\n",
            "probe=filesystem-write dimension=filesystem result=escaped detail=escritura en /etc\n",
        );
        let lines = parse_probe_lines(output);
        assert_eq!(lines.len(), 2);
        assert_eq!(verdict_from_lines(&lines), Verdict::Escaped, "una sola fuga basta");
    }

    #[test]
    fn a_single_escape_outweighs_every_containment() {
        let lines = parse_probe_lines(concat!(
            "probe=a dimension=x result=contained detail=ok\n",
            "probe=b dimension=x result=contained detail=ok\n",
            "probe=c dimension=x result=escaped detail=fuga\n",
        ));
        assert_eq!(verdict_from_lines(&lines), Verdict::Escaped);
    }

    #[test]
    fn errors_are_inconclusive_never_contained() {
        let lines = parse_probe_lines("probe=a dimension=x result=error detail=no se pudo leer /proc");
        assert_eq!(verdict_from_lines(&lines), Verdict::Inconclusive);
    }

    #[test]
    fn no_output_is_inconclusive() {
        assert_eq!(verdict_from_lines(&[]), Verdict::Inconclusive);
    }

    #[test]
    fn declared_but_escaped_is_flagged_as_false_assurance() {
        let escaped = ProbeResult {
            probe: "network-egress".into(),
            dimension: "network".into(),
            control: "network".into(),
            verdict: Verdict::Escaped,
            declared: true,
            detail: String::new(),
            duration_ms: 1,
            lines: vec![],
        };
        assert!(escaped.is_false_assurance(), "declarar y no cumplir es peor que no declarar");

        let honest = ProbeResult { declared: false, ..escaped.clone() };
        assert!(!honest.is_false_assurance(), "no declararlo es honesto, no una falsa garantía");

        let good = ProbeResult { verdict: Verdict::Contained, ..escaped };
        assert!(!good.is_false_assurance());
    }
}
