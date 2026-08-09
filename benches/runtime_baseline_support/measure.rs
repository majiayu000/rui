use std::error::Error;
use std::fmt;

pub const SAMPLE_COUNT: usize = 9;

pub struct BenchCase {
    pub id: &'static str,
    pub category: &'static str,
    pub unit: &'static str,
    pub run: fn() -> Result<f64, BenchError>,
}

#[derive(Debug, Clone)]
pub struct BenchError {
    message: String,
}

impl BenchError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for BenchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for BenchError {}

impl From<rui::renderer::text::TextError> for BenchError {
    fn from(value: rui::renderer::text::TextError) -> Self {
        Self::new(format!("text benchmark failed: {value:?}"))
    }
}

impl From<rui::renderer::RendererError> for BenchError {
    fn from(value: rui::renderer::RendererError) -> Self {
        Self::new(format!("renderer benchmark failed: {value}"))
    }
}

#[derive(Debug, Clone)]
pub struct BenchmarkMeasurement {
    pub id: &'static str,
    pub category: &'static str,
    pub unit: &'static str,
    pub median: f64,
    pub p95: f64,
    pub samples: usize,
}

pub fn run_case(case: &BenchCase) -> Result<BenchmarkMeasurement, BenchError> {
    let _warm_up = (case.run)()?;
    let mut samples = Vec::with_capacity(SAMPLE_COUNT);
    for _ in 0..SAMPLE_COUNT {
        let sample = (case.run)()?;
        samples.push(sample);
    }
    samples.sort_by(f64::total_cmp);

    Ok(BenchmarkMeasurement {
        id: case.id,
        category: case.category,
        unit: case.unit,
        median: percentile(&samples, 0.50),
        p95: percentile(&samples, 0.95),
        samples: samples.len(),
    })
}

fn percentile(samples: &[f64], percentile: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let raw_index = (samples.len() as f64 * percentile).ceil() as usize;
    let index = raw_index.saturating_sub(1).min(samples.len() - 1);
    samples[index]
}
