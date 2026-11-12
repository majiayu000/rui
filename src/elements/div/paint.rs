//! What a [`Div`] paints, and whether any of it can be seen.

use super::Div;
use crate::core::ElementId;
use crate::core::geometry::Bounds;
use crate::core::style::Background;
use crate::elements::element::{PaintContext, add_reach, child_bounds_in, no_reach};
use crate::renderer::{Primitive, quad_reach};
use taffy::TaffyTree;

impl Div {
    pub(super) fn paint_div(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();

        // Paint shadow first (behind the element)
        if let Some(ref shadow) = self.style.shadow {
            let shadow_bounds = Bounds::from_xywh(
                bounds.x() + shadow.offset_x - shadow.spread_radius,
                bounds.y() + shadow.offset_y - shadow.spread_radius,
                bounds.width() + shadow.spread_radius * 2.0,
                bounds.height() + shadow.spread_radius * 2.0,
            );
            cx.paint(Primitive::Shadow {
                bounds: shadow_bounds,
                corner_radii: self.style.border.radius,
                blur_radius: shadow.blur_radius,
                color: shadow.color.to_rgba(),
            });
        }

        // Paint background
        match &self.style.background {
            Background::None => {}
            Background::Solid(color) => {
                cx.paint(Primitive::Quad {
                    bounds,
                    background: color.to_rgba(),
                    border_color: self.style.border.color.to_rgba(),
                    border_widths: self.style.border.width,
                    corner_radii: self.style.border.radius,
                });
            }
            Background::LinearGradient { start, end, angle } => {
                cx.paint(Primitive::LinearGradient {
                    bounds,
                    start: start.to_rgba(),
                    end: end.to_rgba(),
                    angle: *angle,
                    border_color: self.style.border.color.to_rgba(),
                    border_widths: self.style.border.width,
                    corner_radii: self.style.border.radius,
                });
            }
            Background::RadialGradient { inner, outer } => {
                cx.paint(Primitive::RadialGradient {
                    bounds,
                    inner: inner.to_rgba(),
                    outer: outer.to_rgba(),
                    border_color: self.style.border.color.to_rgba(),
                    border_widths: self.style.border.width,
                    corner_radii: self.style.border.radius,
                });
            }
        }

        // Paint children
        // Note: In a full implementation, we'd get child bounds from the layout tree
        for (child, node) in self
            .children
            .iter_mut()
            .zip(self.child_nodes.iter().copied())
        {
            let child_bounds = cx.child_bounds(node).unwrap_or(bounds);
            let mut child_cx = cx.with_bounds(child_bounds);
            child.paint(&mut child_cx);
        }
    }

    pub(super) fn unseen_div_reach(
        &self,
        taffy: &TaffyTree<ElementId>,
        bounds: Bounds,
        visible: &Bounds,
    ) -> Option<Bounds> {
        // How far a shadow reaches is left to its paint.
        if self.style.shadow.is_some() {
            return None;
        }
        // Without a background nothing of its own is painted, not even a border.
        let mut reach = no_reach();
        if !matches!(self.style.background, Background::None) {
            reach = quad_reach(bounds);
            if reach.intersects(visible) {
                return None;
            }
        }
        for (child, node) in self.children.iter().zip(self.child_nodes.iter().copied()) {
            let child_bounds = child_bounds_in(taffy, bounds, node).unwrap_or(bounds);
            reach = add_reach(reach, child.unseen_reach(taffy, child_bounds, visible)?);
        }
        Some(reach)
    }
}
