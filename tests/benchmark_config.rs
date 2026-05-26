#[path = "../benches/runtime_baseline_support/config.rs"]
mod runtime_baselines_config;

use runtime_baselines_config::{
    classify_regression, BenchmarkBaseline, RegressionStatus, RuntimeBaseline, BENCHMARK_IDS,
    REQUIRED_CATEGORIES,
};
use std::collections::HashSet;

fn load_baseline() -> RuntimeBaseline {
    match RuntimeBaseline::load() {
        Ok(baseline) => baseline,
        Err(err) => panic!("baseline JSON should parse: {err}"),
    }
}

#[test]
fn benchmark_config_baseline_schema_and_thresholds_are_valid() {
    let baseline = load_baseline();

    assert_eq!(baseline.schema_version, 1);
    assert_eq!(baseline.thresholds.warning_median_percent, 15.0);
    assert_eq!(baseline.thresholds.blocking_median_percent, 25.0);
    assert_eq!(baseline.thresholds.blocking_p95_percent, 30.0);
    assert!(
        baseline.thresholds.warning_median_percent < baseline.thresholds.blocking_median_percent
    );
    assert!(!baseline.thresholds.enforcement_enabled);
}

#[test]
fn benchmark_config_baseline_covers_all_runtime_categories() {
    let baseline = load_baseline();
    let categories = baseline
        .benchmarks
        .iter()
        .map(|benchmark| benchmark.category.as_str())
        .collect::<HashSet<_>>();

    for category in REQUIRED_CATEGORIES {
        assert!(categories.contains(category), "missing {category} category");
    }
}

#[test]
fn benchmark_config_baseline_ids_match_harness_ids() {
    let baseline = load_baseline();
    let baseline_ids = baseline
        .benchmarks
        .iter()
        .map(|benchmark| benchmark.id.as_str())
        .collect::<HashSet<_>>();

    for id in BENCHMARK_IDS {
        assert!(baseline_ids.contains(id), "missing benchmark id {id}");
        assert!(baseline.benchmark(id).is_some(), "lookup failed for {id}");
    }
    assert_eq!(baseline_ids.len(), BENCHMARK_IDS.len());
}

#[test]
fn benchmark_config_baseline_values_are_positive_and_consistent() {
    let baseline = load_baseline();

    for benchmark in baseline.benchmarks {
        assert!(
            benchmark.median.is_finite(),
            "{} median is not finite",
            benchmark.id
        );
        assert!(
            benchmark.p95.is_finite(),
            "{} p95 is not finite",
            benchmark.id
        );
        assert!(
            benchmark.median > 0.0,
            "{} median must be positive",
            benchmark.id
        );
        assert!(
            benchmark.p95 >= benchmark.median,
            "{} p95 must be at least median",
            benchmark.id
        );
        assert!(
            benchmark.samples >= 3,
            "{} should have multiple samples",
            benchmark.id
        );
    }
}

#[test]
fn benchmark_config_classifies_noise_separately_from_regression() {
    let baseline = BenchmarkBaseline {
        id: "sample".to_string(),
        category: "layout".to_string(),
        unit: "ns_per_frame".to_string(),
        median: 100.0,
        p95: 120.0,
        samples: 9,
    };
    let thresholds = load_baseline().thresholds;

    assert_eq!(
        classify_regression(110.0, 125.0, &baseline, thresholds),
        RegressionStatus::Stable
    );
    assert_eq!(
        classify_regression(118.0, 126.0, &baseline, thresholds),
        RegressionStatus::Warning
    );
    assert_eq!(
        classify_regression(140.0, 130.0, &baseline, thresholds),
        RegressionStatus::MeasurementNoise
    );
    assert_eq!(
        classify_regression(140.0, 170.0, &baseline, thresholds),
        RegressionStatus::Blocking
    );
}
