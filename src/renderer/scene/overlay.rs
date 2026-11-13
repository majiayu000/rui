//! Overlay layers: floating content (menus, popovers, tooltips, dialogs)
//! painted after the rest of the frame and hit before it.
//!
//! An element paints its overlay where it is in the tree, between
//! [`Scene::begin_overlay`] and [`Scene::end_overlay`]. Primitives painted in
//! between are held aside and appended, in z order, when the scene is
//! finished, so the overlay draws over everything painted after its owner.
//! Its hit regions go on a render layer above the page, so it is hit first,
//! and ancestor clips do not apply to it.

use super::*;
use std::rc::Rc;

/// Z-index of a top-level overlay. Overlays opened inside an overlay stack
/// one above the overlay they were opened in.
pub const OVERLAY_Z: ZIndex = ZIndex(1 << 20);

/// An overlay painted in a frame, and how it is dismissed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Overlay {
    /// The element that opened the overlay and closes it.
    pub owner: ElementId,
    /// Where the overlay draws.
    pub bounds: Bounds,
    /// Focus stays inside the overlay while it is open.
    pub modal: bool,
    /// A press outside the owner closes it.
    pub light_dismiss: bool,
    /// Escape closes it while it is the topmost overlay that Escape closes.
    pub escape: bool,
}

impl Overlay {
    /// A menu or popover: closed by a press outside it and by Escape.
    pub fn popup(owner: ElementId, bounds: Bounds) -> Self {
        Self {
            owner,
            bounds,
            modal: false,
            light_dismiss: true,
            escape: true,
        }
    }

    /// A dialog, trapping focus when `modal`; closed by Escape.
    pub fn dialog(owner: ElementId, bounds: Bounds, modal: bool) -> Self {
        Self {
            owner,
            bounds,
            modal,
            light_dismiss: false,
            escape: true,
        }
    }

    /// A tooltip: it only shows, and goes away with the hover that showed it.
    pub fn tooltip(owner: ElementId, bounds: Bounds) -> Self {
        Self {
            owner,
            bounds,
            modal: false,
            light_dismiss: false,
            escape: false,
        }
    }
}

/// Primitives of one overlay, held until the scene is finished.
#[derive(Debug)]
struct Deferred {
    z_index: ZIndex,
    layer: LayerId,
    primitives: Vec<Primitive>,
}

#[derive(Debug, Default)]
pub(super) struct OverlayLayers {
    /// Overlays begun this frame, with their z-index, in the order begun.
    painted: Vec<(ZIndex, Overlay)>,
    deferred: Vec<Deferred>,
    /// Overlays being painted: the deferred entry, and the clips outside it.
    active: SmallVec<[(usize, SmallVec<[Bounds; 8]>); 2]>,
    /// The finished frame's overlays, topmost last.
    stack: Option<Rc<[Overlay]>>,
}

impl OverlayLayers {
    pub(super) fn clear(&mut self) {
        self.painted.clear();
        self.deferred.clear();
        self.active.clear();
        self.stack = None;
    }

    /// Where a primitive inserted now goes, while an overlay is painted.
    pub(super) fn target(&mut self) -> Option<&mut Vec<Primitive>> {
        let (index, _) = self.active.last()?;
        Some(&mut self.deferred[*index].primitives)
    }
}

impl Scene {
    /// Start painting `overlay`: until [`Self::end_overlay`], primitives draw
    /// over the rest of the frame and hit regions are hit before it, with no
    /// ancestor clip.
    pub fn begin_overlay(&mut self, overlay: Overlay) {
        let depth = self.overlays.active.len() as i32;
        let z_index = ZIndex(OVERLAY_Z.0 + depth);
        let layer = self.push_render_layer(z_index, None);
        let clips = std::mem::take(&mut self.layer_stack);
        self.overlays.deferred.push(Deferred {
            z_index,
            layer,
            primitives: Vec::new(),
        });
        let index = self.overlays.deferred.len() - 1;
        self.overlays.active.push((index, clips));
        self.overlays.painted.push((z_index, overlay));
    }

    /// End the overlay begun last.
    pub fn end_overlay(&mut self) {
        let Some((_, clips)) = self.overlays.active.pop() else {
            return;
        };
        self.layer_stack = clips;
        self.pop_render_layer();
    }

    /// Overlays of the finished frame, topmost last.
    pub fn overlays(&self) -> &[Overlay] {
        self.overlays.stack.as_deref().unwrap_or(&[])
    }

    pub(crate) fn overlay_stack(&self) -> Option<Rc<[Overlay]>> {
        self.overlays.stack.clone()
    }

    /// Append the overlays' primitives over the frame, lowest first.
    pub(super) fn flush_overlays(&mut self) {
        while !self.overlays.active.is_empty() {
            self.end_overlay();
        }
        if self.overlays.painted.is_empty() {
            return;
        }
        let mut deferred = std::mem::take(&mut self.overlays.deferred);
        deferred.sort_by_key(|deferred| deferred.z_index);
        for deferred in deferred {
            for primitive in deferred.primitives {
                let index = self.primitives.len();
                self.primitives.push(primitive);
                self.layers[deferred.layer.index()]
                    .primitive_indices
                    .push(index);
            }
        }
        let mut painted = std::mem::take(&mut self.overlays.painted);
        painted.sort_by_key(|(z_index, _)| *z_index);
        self.overlays.stack = Some(painted.into_iter().map(|(_, overlay)| overlay).collect());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::color::Rgba;
    use crate::core::geometry::Edges;
    use crate::core::style::Corners;

    fn quad(x: f32) -> Primitive {
        Primitive::Quad {
            bounds: Bounds::from_xywh(x, 0.0, 10.0, 10.0),
            background: Rgba::new(1.0, 1.0, 1.0, 1.0),
            border_color: Rgba::new(0.0, 0.0, 0.0, 0.0),
            border_widths: Edges::ZERO,
            corner_radii: Corners::ZERO,
        }
    }

    fn first_x(primitive: &Primitive) -> f32 {
        match primitive {
            Primitive::Quad { bounds, .. } => bounds.x(),
            _ => panic!("expected quad"),
        }
    }

    #[test]
    fn overlay_paints_after_later_siblings_and_hits_first() {
        let mut scene = Scene::new();
        let owner = ElementId::new();
        let below = ElementId::new();
        let region = Bounds::from_xywh(0.0, 0.0, 10.0, 10.0);
        scene.push_layer(Bounds::from_xywh(0.0, 0.0, 1.0, 1.0));
        scene.begin_overlay(Overlay::popup(owner, region));
        scene.insert(quad(1.0));
        assert!(
            scene.register_hit_region(owner, region),
            "ancestor clips do not apply"
        );
        scene.end_overlay();
        scene.pop_layer();
        scene.insert(quad(2.0));
        scene.register_hit_region(below, region);
        scene.finish();

        let quads: Vec<f32> = scene
            .primitives()
            .iter()
            .filter(|primitive| matches!(primitive, Primitive::Quad { .. }))
            .map(first_x)
            .collect();
        assert_eq!(quads, [2.0, 1.0]);
        assert_eq!(scene.hit_test(Point::new(5.0, 5.0)), Some(owner));
        assert_eq!(scene.overlays().len(), 1);
    }

    #[test]
    fn nested_overlays_stack_above_their_parent() {
        let mut scene = Scene::new();
        let (outer, inner) = (ElementId::new(), ElementId::new());
        let region = Bounds::from_xywh(0.0, 0.0, 10.0, 10.0);
        scene.begin_overlay(Overlay::dialog(outer, region, true));
        scene.begin_overlay(Overlay::popup(inner, region));
        scene.insert(quad(2.0));
        scene.end_overlay();
        scene.insert(quad(1.0));
        scene.end_overlay();
        scene.finish();

        let xs: Vec<f32> = scene.primitives().iter().map(first_x).collect();
        assert_eq!(xs, [1.0, 2.0]);
        let owners: Vec<ElementId> = scene.overlays().iter().map(|o| o.owner).collect();
        assert_eq!(owners, [outer, inner]);
    }
}
