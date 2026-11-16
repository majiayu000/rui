//! Where accessibility nodes sit on screen, registered in paint order so a
//! replayed subtree can bring its regions along with its primitives.

use crate::core::ElementId;
use crate::core::geometry::Bounds;
use std::collections::HashMap;
use std::ops::Range;

#[derive(Default)]
pub(super) struct AccessibilityRegions {
    /// This frame's regions in paint order, and the previous frame's.
    order: Vec<(ElementId, Bounds)>,
    previous: Vec<(ElementId, Bounds)>,
    by_id: HashMap<ElementId, Bounds>,
}

impl AccessibilityRegions {
    /// Start a frame; the frame just painted becomes the one replays copy from.
    pub(super) fn clear(&mut self) {
        std::mem::swap(&mut self.order, &mut self.previous);
        self.order.clear();
        self.by_id.clear();
    }

    pub(super) fn register(&mut self, id: ElementId, bounds: Bounds) {
        self.order.push((id, bounds));
        self.by_id.insert(id, bounds);
    }

    pub(super) fn len(&self) -> usize {
        self.order.len()
    }

    pub(super) fn by_id(&self) -> &HashMap<ElementId, Bounds> {
        &self.by_id
    }

    /// Copy the previous frame's regions in `range`, moved by `dx, dy`, and
    /// return where they are in this frame.
    pub(super) fn replay(&mut self, range: Range<usize>, dx: f32, dy: f32) -> Range<usize> {
        let start = self.order.len();
        let Some(regions) = self.previous.get(range) else {
            return start..start;
        };
        for &(id, mut bounds) in regions {
            bounds.origin.x += dx;
            bounds.origin.y += dy;
            self.order.push((id, bounds));
            self.by_id.insert(id, bounds);
        }
        start..self.order.len()
    }
}
