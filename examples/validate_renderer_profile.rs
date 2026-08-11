use serde::Deserialize;
use std::ffi::OsString;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

const RENDERER_PROFILE_SCHEMA: &str = "rui.renderer.profile.v1";

#[derive(Debug, Deserialize)]
struct RendererProfileFrame {
    schema: String,
    frame_interval_ns: Option<u128>,
    event_to_render_latency_ns: Option<u128>,
    layout_ns: u128,
    dispatch_ns: u128,
    paint_ns: u128,
    render_ns: u128,
    render_p95_ns: u128,
    render_p99_ns: u128,
    jank_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RendererProfileSummary {
    frame_count: usize,
}

impl RendererProfileSummary {
    pub(crate) fn validated_frames(self) -> usize {
        self.frame_count
    }
}

pub(crate) fn validate_renderer_profile(
    reader: impl BufRead,
) -> Result<RendererProfileSummary, String> {
    let mut frame_count = 0;
    let mut has_frame_interval = false;
    let mut has_event_to_render_latency = false;

    for (index, line) in reader.lines().enumerate() {
        let line_number = index + 1;
        let line = line.map_err(|err| format!("failed to read line {line_number}: {err}"))?;
        let frame: RendererProfileFrame = serde_json::from_str(&line)
            .map_err(|err| format!("invalid renderer telemetry at line {line_number}: {err}"))?;
        if frame.schema != RENDERER_PROFILE_SCHEMA {
            return Err(format!(
                "unexpected renderer telemetry schema at line {line_number}: {}",
                frame.schema
            ));
        }

        // Deserialization above guarantees that every frame contains numeric
        // stage, percentile, and jank fields. Read them here so future field
        // removals cannot silently weaken the validator.
        let _required_metrics = (
            frame.layout_ns,
            frame.dispatch_ns,
            frame.paint_ns,
            frame.render_ns,
            frame.render_p95_ns,
            frame.render_p99_ns,
            frame.jank_count,
        );
        has_frame_interval |= frame.frame_interval_ns.is_some();
        has_event_to_render_latency |= frame.event_to_render_latency_ns.is_some();
        frame_count += 1;
    }

    if frame_count == 0 {
        return Err("renderer telemetry profile did not contain any frames".to_string());
    }
    if !has_frame_interval {
        return Err("renderer telemetry did not contain a numeric frame_interval_ns".to_string());
    }
    if !has_event_to_render_latency {
        return Err(
            "renderer telemetry did not contain a numeric event_to_render_latency_ns".to_string(),
        );
    }

    Ok(RendererProfileSummary { frame_count })
}

fn profile_path_from_args() -> Result<PathBuf, String> {
    let mut args = std::env::args_os();
    let program = args
        .next()
        .unwrap_or_else(|| OsString::from("validate_renderer_profile"));
    let path = args.next().ok_or_else(|| {
        format!(
            "usage: {} <renderer-profile.jsonl>",
            PathBuf::from(program).display()
        )
    })?;
    if args.next().is_some() {
        return Err("validate_renderer_profile accepts exactly one profile path".to_string());
    }
    Ok(PathBuf::from(path))
}

fn run() -> Result<RendererProfileSummary, String> {
    let path = profile_path_from_args()?;
    let file = File::open(&path)
        .map_err(|err| format!("failed to open renderer profile {}: {err}", path.display()))?;
    validate_renderer_profile(BufReader::new(file))
}

fn main() {
    match run() {
        Ok(summary) => println!(
            "validated {} renderer telemetry frames",
            summary.validated_frames()
        ),
        Err(err) => {
            eprintln!("renderer telemetry validation failed: {err}");
            std::process::exit(1);
        }
    }
}
