//! The profiling overlay: per-frame stage times and renderer counters, drawn
//! into the scene after paint, outside layout.

use super::{allocations, profiler_started, profiler_stopped};
use crate::core::color::Rgba;
use crate::core::frame_pipeline::FrameStageDurations;
use crate::core::geometry::{Bounds, Edges, Size};
use crate::core::style::Corners;
use crate::elements::text::TextAlign;
use crate::renderer::{Primitive, Reach, Scene};
use std::fmt::Write as _;
use std::time::{Duration, Instant};

const PANEL_WIDTH: f32 = 340.0;
const MARGIN: f32 = 8.0;
const PADDING: f32 = 8.0;
const FONT_SIZE: f32 = 11.0;
const LINE_HEIGHT: f32 = 15.0;
const LINES: usize = 8;
const FONT_FAMILY: &str = "SF Mono";
/// A frame later than this many expected intervals dropped the ones between.
const LATE_FACTOR: f64 = 1.5;

/// What the backend reports about presenting a frame. Runners without a GPU
/// backend report nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderSample {
    /// Waiting for a drawable to draw into.
    pub drawable_wait_ns: u128,
    /// Encoding and committing the frame's GPU work.
    pub render_ns: u128,
    pub draw_calls: u32,
    pub instances: u32,
    /// Device pixels drawn again.
    pub redrawn_pixels: u64,
    /// GPU time of the latest frame the GPU finished, which is a frame or two
    /// behind this one.
    pub gpu_ns: Option<u128>,
}

/// One frame as the profiler measured it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FrameSample {
    /// Frames this window drew with the profiler on, before this one.
    pub index: u64,
    /// Since the previous drawn frame began.
    pub interval_ns: Option<u128>,
    pub durations: FrameStageDurations,
    pub render: Option<RenderSample>,
    /// Share of the window the scene reported as changed, the overlay left out.
    pub damage_fraction: f32,
    pub primitives: usize,
    pub replayed_primitives: usize,
    /// Allocations from rebuild through present, the overlay's own left out.
    /// `None` without [`CountingAllocator`](super::CountingAllocator).
    pub allocations: Option<u64>,
}

/// Per-window profiler state, alive only while the profiler is on.
#[derive(Debug)]
pub struct Profiler {
    frames: u64,
    previous_start: Option<Instant>,
    /// The previous frame asked for the next one at the display's rate.
    next_frame_expected: bool,
    dropped: u64,
    allocations_at_start: u64,
    overlay_allocations: u64,
    /// Where this window's overlay starts in the scene it was drawn into.
    overlay_start: Option<usize>,
    current: FrameSample,
    last: Option<FrameSample>,
}

impl Profiler {
    pub(crate) fn new() -> Self {
        profiler_started();
        Self {
            frames: 0,
            previous_start: None,
            next_frame_expected: false,
            dropped: 0,
            allocations_at_start: 0,
            overlay_allocations: 0,
            overlay_start: None,
            current: FrameSample::default(),
            last: None,
        }
    }

    /// The last frame measured from start to end.
    pub fn last_frame(&self) -> Option<&FrameSample> {
        self.last.as_ref()
    }

    /// Frames missed while an animation asked for every display refresh.
    pub fn dropped_frames(&self) -> u64 {
        self.dropped
    }

    /// Start measuring a frame that will be drawn, `expected_interval` after
    /// the previous one if that asked for the next refresh.
    pub(crate) fn begin_frame(&mut self, expected_interval: Duration) {
        let now = Instant::now();
        let interval = self.previous_start.map(|start| now.duration_since(start));
        if self.next_frame_expected
            && let Some(interval) = interval
            && !expected_interval.is_zero()
        {
            let refreshes = interval.as_secs_f64() / expected_interval.as_secs_f64();
            if refreshes > LATE_FACTOR {
                self.dropped += (refreshes.round() as u64).saturating_sub(1);
            }
        }
        self.previous_start = Some(now);
        self.overlay_allocations = 0;
        self.current = FrameSample {
            index: self.frames,
            interval_ns: interval.map(|interval| interval.as_nanos()),
            ..FrameSample::default()
        };
        self.allocations_at_start = allocations();
    }

    /// What the backend did to present this frame.
    pub fn record_render(&mut self, render: RenderSample) {
        self.current.render = Some(render);
    }

    /// Record what the scene holds after paint, then draw the overlay over it,
    /// showing the last complete frame.
    pub(crate) fn paint_overlay(&mut self, scene: &mut Scene, viewport: Size) {
        let allocations_before = allocations();
        // The previous overlay is redrawn by this one, which covers the same
        // panel, so it is not part of what the app changed.
        if let Some(start) = self.overlay_start {
            scene.truncate_previous(start);
        }
        let window = Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height);
        self.current.damage_fraction = damage_fraction(scene.damage(window), window);
        self.current.primitives = scene.len();
        self.current.replayed_primitives = scene.replayed_len();

        self.overlay_start = Some(scene.len());
        let panel = Bounds::from_xywh(
            (viewport.width - PANEL_WIDTH - MARGIN).max(0.0),
            MARGIN,
            PANEL_WIDTH,
            PADDING * 2.0 + LINE_HEIGHT * LINES as f32,
        );
        scene.insert(Primitive::Quad {
            bounds: panel,
            background: Rgba::new(0.05, 0.06, 0.08, 0.86),
            border_color: Rgba::new(1.0, 1.0, 1.0, 0.18),
            border_widths: Edges::all(1.0),
            corner_radii: Corners::all(6.0),
        });
        let lines = self.lines();
        for (row, line) in lines.into_iter().enumerate() {
            scene.insert(Primitive::Text {
                bounds: Bounds::from_xywh(
                    panel.origin.x + PADDING,
                    panel.origin.y + PADDING + LINE_HEIGHT * row as f32,
                    PANEL_WIDTH - PADDING * 2.0,
                    LINE_HEIGHT,
                ),
                content: line,
                color: Rgba::new(0.88, 0.94, 1.0, 1.0),
                font_size: FONT_SIZE,
                font_weight: 400,
                font_family: Some(FONT_FAMILY.to_string()),
                line_height: LINE_HEIGHT,
                align: TextAlign::Left,
            });
        }
        self.overlay_allocations = allocations() - allocations_before;
    }

    /// Close the frame `durations` timed. `next_frame_expected` when an
    /// animation asked for the next display refresh.
    pub(crate) fn end_frame(&mut self, durations: FrameStageDurations, next_frame_expected: bool) {
        let counted = allocations() - self.allocations_at_start;
        // The overlay itself always allocates, so a count of 0 means nothing counts.
        self.current.allocations =
            (allocations() > 0).then(|| counted.saturating_sub(self.overlay_allocations));
        self.current.durations = durations;
        self.next_frame_expected = next_frame_expected;
        self.last = Some(self.current);
        self.frames += 1;
    }

    fn lines(&self) -> [String; LINES] {
        let mut lines: [String; LINES] = Default::default();
        let Some(frame) = self.last.as_ref() else {
            lines[0] = String::from("rui profiler: measuring…");
            return lines;
        };
        let d = &frame.durations;
        let _ = write!(lines[0], "rui profiler  frame {}", frame.index);
        let _ = match frame.interval_ns {
            Some(interval) => write!(
                lines[1],
                "interval {}  {:.0} fps  dropped {}",
                ms(interval),
                1e9 / interval.max(1) as f64,
                self.dropped
            ),
            None => write!(lines[1], "interval -  dropped {}", self.dropped),
        };
        let _ = write!(
            lines[2],
            "prepare {}  rebuild {}",
            ms(d.prepare_ns),
            ms(d.rebuild_ns)
        );
        let _ = write!(
            lines[3],
            "layout {}  paint {}",
            ms(d.layout_ns),
            ms(d.paint_ns)
        );
        let _ = match frame.render {
            Some(render) => write!(
                lines[4],
                "present {}  wait {}  render {}",
                ms(d.present_ns),
                ms(render.drawable_wait_ns),
                ms(render.render_ns)
            ),
            None => write!(lines[4], "present {}  (no backend)", ms(d.present_ns)),
        };
        let _ = match frame.render {
            Some(render) => write!(
                lines[5],
                "gpu {}  draws {}  instances {}",
                render.gpu_ns.map_or_else(|| String::from("-"), ms),
                render.draw_calls,
                render.instances
            ),
            None => write!(lines[5], "gpu -  draws -"),
        };
        let _ = write!(
            lines[6],
            "damage {:.1}%  redrawn {}",
            frame.damage_fraction * 100.0,
            frame.render.map_or_else(
                || String::from("-"),
                |render| format!("{:.2} Mpx", render.redrawn_pixels as f64 / 1e6)
            )
        );
        let _ = write!(
            lines[7],
            "prims {} (replayed {})  allocs {}",
            frame.primitives,
            frame.replayed_primitives,
            frame
                .allocations
                .map_or_else(|| String::from("n/a"), |count| count.to_string())
        );
        lines
    }
}

impl Drop for Profiler {
    fn drop(&mut self) {
        profiler_stopped();
    }
}

fn ms(ns: u128) -> String {
    format!("{:.2}ms", ns as f64 / 1e6)
}

fn damage_fraction(damage: Reach, window: Bounds) -> f32 {
    let area = window.size.area();
    if area <= 0.0 {
        return 0.0;
    }
    match damage {
        Reach::Nothing => 0.0,
        Reach::Unbounded => 1.0,
        Reach::Within(bounds) => bounds
            .intersection(&window)
            .map_or(0.0, |bounds| (bounds.size.area() / area).min(1.0)),
    }
}
