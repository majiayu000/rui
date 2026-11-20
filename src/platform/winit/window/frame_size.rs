//! Logical viewport and surface size changes queued before a frame.

use crate::core::geometry::Size;
use crate::platform::window::PlatformWindowEvent;

pub(super) struct FrameSize {
    pub(super) logical: Size,
    pub(super) surface_changed: bool,
}

impl FrameSize {
    pub(super) fn new(logical: Size) -> Self {
        Self {
            logical,
            surface_changed: false,
        }
    }

    pub(super) fn update(
        &mut self,
        event: &PlatformWindowEvent,
        current_logical_size: impl FnOnce() -> Size,
    ) {
        self.logical = match event {
            PlatformWindowEvent::Resized(size) => *size,
            PlatformWindowEvent::ScaleFactorChanged(_) => current_logical_size(),
            _ => return,
        };
        self.surface_changed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_scale_updates_surface_even_when_logical_size_is_unchanged() {
        let logical = Size::new(800.0, 600.0);
        let mut frame = FrameSize::new(logical);
        frame.update(&PlatformWindowEvent::ScaleFactorChanged(2.0), || logical);
        frame.update(&PlatformWindowEvent::Resized(logical), || unreachable!());
        assert_eq!(frame.logical, logical);
        assert!(frame.surface_changed);
    }

    #[test]
    fn new_scale_without_resize_reads_the_current_logical_size() {
        let mut frame = FrameSize::new(Size::new(800.0, 600.0));
        let logical = Size::new(400.0, 300.0);
        frame.update(&PlatformWindowEvent::ScaleFactorChanged(2.0), || logical);
        assert_eq!(frame.logical, logical);
        assert!(frame.surface_changed);
    }
}
