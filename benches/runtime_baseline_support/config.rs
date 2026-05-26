use serde::{Deserialize, Serialize};

pub const BASELINE_JSON: &str = include_str!("../../benchmarks/runtime_baselines.json");

pub const LAYOUT_BENCHMARK_ID: &str = "layout.flex_tree";
pub const TEXT_BENCHMARK_ID: &str = "text.measure_raster";
pub const SCENE_BENCHMARK_ID: &str = "scene.build";
pub const EVENT_BENCHMARK_ID: &str = "event.pointer_dispatch";
pub const RENDERER_BENCHMARK_ID: &str = "renderer.recording_throughput";

#[cfg(test)]
#[allow(dead_code)]
pub const REQUIRED_CATEGORIES: [&str; 5] = ["layout", "text", "scene", "event", "renderer"];

#[cfg(test)]
#[allow(dead_code)]
pub const BENCHMARK_IDS: [&str; 5] = [
    LAYOUT_BENCHMARK_ID,
    TEXT_BENCHMARK_ID,
    SCENE_BENCHMARK_ID,
    EVENT_BENCHMARK_ID,
    RENDERER_BENCHMARK_ID,
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeBaseline {
    pub schema_version: u32,
    pub generated_at: String,
    pub baseline_revision: String,
    pub environment: BenchmarkEnvironment,
    pub thresholds: RegressionThresholds,
    pub benchmarks: Vec<BenchmarkBaseline>,
}

impl RuntimeBaseline {
    pub fn load() -> Result<Self, serde_json::Error> {
        serde_json::from_str(BASELINE_JSON)
    }

    pub fn benchmark(&self, id: &str) -> Option<&BenchmarkBaseline> {
        self.benchmarks.iter().find(|benchmark| benchmark.id == id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkEnvironment {
    pub os: String,
    pub arch: String,
    pub rustc: String,
    pub profile: String,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RegressionThresholds {
    pub warning_median_percent: f64,
    pub blocking_median_percent: f64,
    pub blocking_p95_percent: f64,
    pub enforcement_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkBaseline {
    pub id: String,
    pub category: String,
    pub unit: String,
    pub median: f64,
    pub p95: f64,
    pub samples: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RegressionStatus {
    Stable,
    MeasurementNoise,
    Warning,
    Blocking,
}

pub fn percent_delta(current: f64, baseline: f64) -> f64 {
    if baseline <= 0.0 {
        if current <= 0.0 {
            return 0.0;
        }
        return f64::INFINITY;
    }

    ((current - baseline) / baseline) * 100.0
}

pub fn classify_regression(
    current_median: f64,
    current_p95: f64,
    baseline: &BenchmarkBaseline,
    thresholds: RegressionThresholds,
) -> RegressionStatus {
    let median_delta = percent_delta(current_median, baseline.median);
    let p95_delta = percent_delta(current_p95, baseline.p95);
    let median_warning = median_delta > thresholds.warning_median_percent;
    let median_blocking = median_delta > thresholds.blocking_median_percent;
    let p95_blocking = p95_delta > thresholds.blocking_p95_percent;

    if median_blocking && p95_blocking {
        RegressionStatus::Blocking
    } else if median_blocking || p95_blocking {
        RegressionStatus::MeasurementNoise
    } else if median_warning {
        RegressionStatus::Warning
    } else {
        RegressionStatus::Stable
    }
}
