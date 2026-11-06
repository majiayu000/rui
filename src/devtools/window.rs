//! A window's devtools: what is drawn over its scene after paint.

use super::{DevtoolsFlags, Profiler, allocations, damage};
use crate::core::frame_pipeline::FrameStageDurations;
use crate::core::geometry::{Bounds, Size};
use crate::renderer::Scene;
use std::time::Duration;

/// Alive while any tool is on in this window.
#[derive(Debug, Default)]
pub(crate) struct WindowDevtools {
    profiler: Option<Profiler>,
    damage: bool,
    /// Where the overlay starts in the scene it was last drawn into.
    overlay_start: Option<usize>,
}

impl WindowDevtools {
    pub(crate) fn profiler(&self) -> Option<&Profiler> {
        self.profiler.as_ref()
    }

    pub(crate) fn profiler_mut(&mut self) -> Option<&mut Profiler> {
        self.profiler.as_mut()
    }

    /// Start or stop tools as `flags` ask, before a frame that will be drawn.
    pub(crate) fn begin_frame(&mut self, flags: DevtoolsFlags, expected_interval: Duration) {
        self.damage = flags.damage;
        match (flags.profiler, &mut self.profiler) {
            (true, Some(profiler)) => profiler.begin_frame(expected_interval),
            (true, None) => self
                .profiler
                .insert(Profiler::new())
                .begin_frame(expected_interval),
            (false, _) => self.profiler = None,
        }
    }

    /// Draw the overlay over what the app painted.
    ///
    /// The previous overlay stays in the scene's previous frame, so the
    /// renderer redraws where it was; it is left out only of the damage the
    /// tools report.
    pub(crate) fn paint_overlay(&mut self, scene: &mut Scene, viewport: Size) {
        let allocations_before = allocations();
        let window = Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height);
        let damage = match self.overlay_start.take() {
            Some(start) => scene.damage_before_previous(window, start),
            None => scene.damage(window),
        };
        if let Some(profiler) = self.profiler.as_mut() {
            profiler.record_scene(scene, damage, window);
        }
        self.overlay_start = Some(scene.len());
        if self.damage {
            damage::paint(scene, damage, window);
        }
        if let Some(profiler) = self.profiler.as_mut() {
            profiler.paint_panel(scene, viewport);
            profiler.exclude_allocations(allocations() - allocations_before);
        }
    }

    pub(crate) fn end_frame(&mut self, durations: FrameStageDurations, next_frame_expected: bool) {
        if let Some(profiler) = self.profiler.as_mut() {
            profiler.end_frame(durations, next_frame_expected);
        }
    }
}
