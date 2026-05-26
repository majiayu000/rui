use super::config::{classify_regression, percent_delta, RegressionStatus, RuntimeBaseline};
use super::measure::BenchmarkMeasurement;
use serde::Serialize;
use std::error::Error;

#[derive(Debug, Serialize)]
struct BenchmarkReport<'a> {
    schema_version: u32,
    baseline_revision: &'a str,
    environment: ReportEnvironment,
    thresholds: super::config::RegressionThresholds,
    results: Vec<BenchmarkResult<'a>>,
    enforcement_enabled: bool,
}

#[derive(Debug, Serialize)]
struct ReportEnvironment {
    os: &'static str,
    arch: &'static str,
}

#[derive(Debug, Serialize)]
struct BenchmarkResult<'a> {
    id: &'a str,
    category: &'a str,
    unit: &'a str,
    median: f64,
    p95: f64,
    baseline_median: f64,
    baseline_p95: f64,
    median_delta_percent: f64,
    p95_delta_percent: f64,
    samples: usize,
    status: RegressionStatus,
}

pub fn print_report(
    baseline: &RuntimeBaseline,
    measurements: &[BenchmarkMeasurement],
) -> Result<(), Box<dyn Error>> {
    let results = measurements
        .iter()
        .map(|measurement| result_for_measurement(baseline, measurement))
        .collect::<Result<Vec<_>, _>>()?;
    let has_blocking = results
        .iter()
        .any(|result| result.status == RegressionStatus::Blocking);
    let report = BenchmarkReport {
        schema_version: baseline.schema_version,
        baseline_revision: &baseline.baseline_revision,
        environment: ReportEnvironment {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        },
        thresholds: baseline.thresholds,
        results,
        enforcement_enabled: baseline.thresholds.enforcement_enabled,
    };

    println!("{}", serde_json::to_string_pretty(&report)?);

    if baseline.thresholds.enforcement_enabled && has_blocking {
        return Err("benchmark blocking threshold exceeded".into());
    }

    Ok(())
}

fn result_for_measurement<'a>(
    baseline: &RuntimeBaseline,
    measurement: &'a BenchmarkMeasurement,
) -> Result<BenchmarkResult<'a>, Box<dyn Error>> {
    let baseline_benchmark = baseline
        .benchmark(measurement.id)
        .ok_or_else(|| format!("missing baseline for {}", measurement.id))?;
    if baseline_benchmark.category != measurement.category {
        return Err(format!(
            "baseline category mismatch for {}: expected {}, got {}",
            measurement.id, baseline_benchmark.category, measurement.category
        )
        .into());
    }
    if baseline_benchmark.unit != measurement.unit {
        return Err(format!(
            "baseline unit mismatch for {}: expected {}, got {}",
            measurement.id, baseline_benchmark.unit, measurement.unit
        )
        .into());
    }
    let status = classify_regression(
        measurement.median,
        measurement.p95,
        baseline_benchmark,
        baseline.thresholds,
    );

    Ok(BenchmarkResult {
        id: measurement.id,
        category: measurement.category,
        unit: measurement.unit,
        median: measurement.median,
        p95: measurement.p95,
        baseline_median: baseline_benchmark.median,
        baseline_p95: baseline_benchmark.p95,
        median_delta_percent: percent_delta(measurement.median, baseline_benchmark.median),
        p95_delta_percent: percent_delta(measurement.p95, baseline_benchmark.p95),
        samples: measurement.samples,
        status,
    })
}
