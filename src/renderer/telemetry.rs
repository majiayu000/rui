//! Structured renderer frame telemetry.

use crate::core::geometry::Size;
use crate::renderer::primitives::PrimitiveKind;
use crate::renderer::{RendererDiagnostics, Scene};
use std::collections::VecDeque;
use std::time::Instant;

pub const RUI_PROFILE_ENV: &str = "RUI_PROFILE";
const DEFAULT_JANK_THRESHOLD_NS: u128 = 16_666_667;
const DEFAULT_WINDOW_SIZE: usize = 120;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RendererFramePhaseDurations {
    pub frame_interval_ns: Option<u128>,
    pub event_to_render_latency_ns: Option<u128>,
    pub drawable_wait_ns: u128,
    pub layout_ns: u128,
    pub dispatch_ns: u128,
    pub paint_ns: u128,
    pub render_ns: u128,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RendererBatchDiagnostics {
    pub primitive_count: usize,
    pub draw_count: usize,
    pub batch_count: usize,
    pub clip_change_count: usize,
    pub buffer_allocations: usize,
}

impl RendererBatchDiagnostics {
    pub fn from_scene(scene: &Scene) -> Self {
        Self::from_scene_with_buffer_allocations(scene, 0)
    }

    pub fn for_metal_scene(scene: &Scene) -> Self {
        let draw_count = scene
            .primitives()
            .iter()
            .filter(|primitive| is_draw_kind(primitive.kind()))
            .count();
        Self::from_scene_with_buffer_allocations(scene, draw_count * 2)
    }

    fn from_scene_with_buffer_allocations(scene: &Scene, buffer_allocations: usize) -> Self {
        let mut draw_count = 0;
        let mut batch_count = 0;
        let mut clip_change_count = 0;
        let mut last_draw_kind = None;

        for primitive in scene.primitives() {
            let kind = primitive.kind();
            if matches!(kind, PrimitiveKind::PushClip | PrimitiveKind::PopClip) {
                clip_change_count += 1;
                last_draw_kind = None;
                continue;
            }
            if !is_draw_kind(kind) {
                continue;
            }
            draw_count += 1;
            if last_draw_kind != Some(kind) {
                batch_count += 1;
                last_draw_kind = Some(kind);
            }
        }

        Self {
            primitive_count: scene.len(),
            draw_count,
            batch_count,
            clip_change_count,
            buffer_allocations,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RendererFrameTelemetry {
    pub frame_index: u64,
    pub backend: String,
    pub viewport_size: Size,
    pub phases: RendererFramePhaseDurations,
    pub batch: RendererBatchDiagnostics,
    pub resource_live_bytes: usize,
    pub resource_pressure_events: usize,
    pub resource_cache_hits: usize,
    pub resource_cache_misses: usize,
    pub resource_evictions: usize,
    pub render_p95_ns: u128,
    pub render_p99_ns: u128,
    pub jank_count: usize,
}

impl RendererFrameTelemetry {
    pub fn to_json_line(&self) -> String {
        format!(
            "{{\"schema\":\"rui.renderer.profile.v1\",\"frame_index\":{},\"backend\":{},\"viewport_width\":{},\"viewport_height\":{},\"frame_interval_ns\":{},\"event_to_render_latency_ns\":{},\"drawable_wait_ns\":{},\"layout_ns\":{},\"dispatch_ns\":{},\"paint_ns\":{},\"render_ns\":{},\"render_p95_ns\":{},\"render_p99_ns\":{},\"jank_count\":{},\"primitive_count\":{},\"draw_count\":{},\"batch_count\":{},\"clip_change_count\":{},\"buffer_allocations\":{},\"resource_live_bytes\":{},\"resource_pressure_events\":{},\"resource_cache_hits\":{},\"resource_cache_misses\":{},\"resource_evictions\":{}}}",
            self.frame_index,
            json_string(&self.backend),
            self.viewport_size.width,
            self.viewport_size.height,
            json_optional_u128(self.phases.frame_interval_ns),
            json_optional_u128(self.phases.event_to_render_latency_ns),
            self.phases.drawable_wait_ns,
            self.phases.layout_ns,
            self.phases.dispatch_ns,
            self.phases.paint_ns,
            self.phases.render_ns,
            self.render_p95_ns,
            self.render_p99_ns,
            self.jank_count,
            self.batch.primitive_count,
            self.batch.draw_count,
            self.batch.batch_count,
            self.batch.clip_change_count,
            self.batch.buffer_allocations,
            self.resource_live_bytes,
            self.resource_pressure_events,
            self.resource_cache_hits,
            self.resource_cache_misses,
            self.resource_evictions,
        )
    }
}

#[derive(Debug, Clone)]
pub struct RendererTelemetryRecorder {
    frame_index: u64,
    last_frame_started_at: Option<Instant>,
    render_samples: VecDeque<u128>,
    max_samples: usize,
    jank_threshold_ns: u128,
    jank_count: usize,
}

impl RendererTelemetryRecorder {
    pub fn new() -> Self {
        Self {
            frame_index: 0,
            last_frame_started_at: None,
            render_samples: VecDeque::new(),
            max_samples: DEFAULT_WINDOW_SIZE,
            jank_threshold_ns: DEFAULT_JANK_THRESHOLD_NS,
            jank_count: 0,
        }
    }

    pub fn enabled_from_env() -> Option<Self> {
        let value = std::env::var(RUI_PROFILE_ENV).ok()?;
        if matches!(value.as_str(), "" | "0" | "false" | "off") {
            None
        } else {
            Some(Self::new())
        }
    }

    pub fn capture_telemetry(
        &mut self,
        backend: impl Into<String>,
        viewport_size: Size,
        mut phases: RendererFramePhaseDurations,
        diagnostics: &RendererDiagnostics,
        batch: RendererBatchDiagnostics,
    ) -> RendererFrameTelemetry {
        let frame_started_at = Instant::now();
        phases.frame_interval_ns = self
            .last_frame_started_at
            .map(|last| frame_started_at.saturating_duration_since(last).as_nanos());
        self.last_frame_started_at = Some(frame_started_at);

        self.push_render_sample(phases.render_ns);
        if phases.render_ns > self.jank_threshold_ns {
            self.jank_count += 1;
        }
        let render_p95_ns = telemetry_percentile(&self.render_samples, 95);
        let render_p99_ns = telemetry_percentile(&self.render_samples, 99);

        let telemetry = RendererFrameTelemetry {
            frame_index: self.frame_index,
            backend: backend.into(),
            viewport_size,
            phases,
            batch,
            resource_live_bytes: diagnostics.total_live_bytes(),
            resource_pressure_events: diagnostics.total_pressure_events(),
            resource_cache_hits: diagnostics.total_cache_hits(),
            resource_cache_misses: diagnostics.total_cache_misses(),
            resource_evictions: diagnostics.total_evicted_entries(),
            render_p95_ns,
            render_p99_ns,
            jank_count: self.jank_count,
        };
        self.frame_index += 1;
        telemetry
    }

    fn push_render_sample(&mut self, value: u128) {
        self.render_samples.push_back(value);
        while self.render_samples.len() > self.max_samples {
            self.render_samples.pop_front();
        }
    }
}

impl Default for RendererTelemetryRecorder {
    fn default() -> Self {
        Self::new()
    }
}

fn is_draw_kind(kind: PrimitiveKind) -> bool {
    !matches!(kind, PrimitiveKind::PushClip | PrimitiveKind::PopClip)
}

fn telemetry_percentile(samples: &VecDeque<u128>, percentile: usize) -> u128 {
    if samples.is_empty() {
        return 0;
    }
    let mut sorted = samples.iter().copied().collect::<Vec<_>>();
    sorted.sort_unstable();
    let rank = (sorted.len() * percentile).div_ceil(100);
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

fn json_optional_u128(value: Option<u128>) -> String {
    match value {
        Some(value) => value.to_string(),
        None => String::from("null"),
    }
}

fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            ch if ch.is_control() => output.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => output.push(ch),
        }
    }
    output.push('"');
    output
}
