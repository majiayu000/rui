//! What a scroll view keeps across frames: its offset and the sizes it scrolls
//! between.

use crate::core::geometry::Size;

/// Scroll state
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub offset_x: f32,
    pub offset_y: f32,
    pub content_size: Size,
    pub viewport_size: Size,
    pub scrollbar_hovered: bool,
    pub scrollbar_dragging: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollMetrics {
    pub offset_x: f32,
    pub offset_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl ScrollState {
    pub fn metrics(&self) -> Option<ScrollMetrics> {
        if !self.has_measured_metrics() {
            return None;
        }

        Some(ScrollMetrics {
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            max_x: self.max_scroll_x(),
            max_y: self.max_scroll_y(),
        })
    }

    fn has_measured_metrics(&self) -> bool {
        self.content_size.width > 0.0
            || self.content_size.height > 0.0
            || self.viewport_size.width > 0.0
            || self.viewport_size.height > 0.0
    }

    pub fn max_scroll_x(&self) -> f32 {
        (self.content_size.width - self.viewport_size.width).max(0.0)
    }

    pub fn max_scroll_y(&self) -> f32 {
        (self.content_size.height - self.viewport_size.height).max(0.0)
    }

    pub fn scroll_to(&mut self, x: f32, y: f32) {
        self.offset_x = x.clamp(0.0, self.max_scroll_x());
        self.offset_y = y.clamp(0.0, self.max_scroll_y());
    }

    pub fn scroll_by(&mut self, dx: f32, dy: f32) {
        self.scroll_to(self.offset_x + dx, self.offset_y + dy);
    }

    #[cfg(test)]
    pub fn scroll_to_top(&mut self) {
        self.offset_y = 0.0;
    }

    #[cfg(test)]
    pub fn scroll_to_bottom(&mut self) {
        self.offset_y = self.max_scroll_y();
    }

    pub fn can_scroll_up(&self) -> bool {
        self.offset_y > 0.0
    }

    pub fn can_scroll_down(&self) -> bool {
        self.offset_y < self.max_scroll_y()
    }

    pub fn can_scroll_left(&self) -> bool {
        self.offset_x > 0.0
    }

    pub fn can_scroll_right(&self) -> bool {
        self.offset_x < self.max_scroll_x()
    }

    /// Calculate scrollbar thumb position and size (0-1 range)
    pub fn scrollbar_y(&self) -> (f32, f32) {
        if self.content_size.height <= self.viewport_size.height {
            return (0.0, 1.0); // Full size, no scroll
        }
        let ratio = self.viewport_size.height / self.content_size.height;
        let thumb_size = ratio.clamp(0.1, 1.0);
        let scroll_ratio = self.offset_y / self.max_scroll_y();
        let thumb_pos = scroll_ratio * (1.0 - thumb_size);
        (thumb_pos, thumb_size)
    }

    pub fn scrollbar_x(&self) -> (f32, f32) {
        if self.content_size.width <= self.viewport_size.width {
            return (0.0, 1.0);
        }
        let ratio = self.viewport_size.width / self.content_size.width;
        let thumb_size = ratio.clamp(0.1, 1.0);
        let scroll_ratio = self.offset_x / self.max_scroll_x();
        let thumb_pos = scroll_ratio * (1.0 - thumb_size);
        (thumb_pos, thumb_size)
    }
}
