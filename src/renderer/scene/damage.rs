//! Where a frame can differ from the one before it.

use super::{Scene, damage_reach, union};
use crate::core::geometry::Bounds;
use crate::renderer::primitives::{Primitive, Reach};

/// A range of the previous frame replayed in place, and where it landed.
#[derive(Debug, Clone, Copy)]
pub(super) struct InPlaceSource {
    pub(super) start: usize,
    pub(super) previous_start: usize,
    pub(super) previous_end: usize,
}

impl Scene {
    /// [`Self::damage`] counting only the previous frame's primitives before
    /// `end`, to leave out content drawn over the frame after its paint.
    pub(crate) fn damage_before_previous(&self, visible: Bounds, end: usize) -> Reach {
        let seen = |reach: Reach| match reach {
            Reach::Within(bounds) => bounds
                .intersection(&visible)
                .map_or(Reach::Nothing, Reach::Within),
            other => other,
        };
        let end = end.min(self.previous_primitives.len());
        let mut damage = seen(self.moved);
        let mut unchanged = self.unchanged.iter().peekable();
        // Previous primitives painted again, the same, at the same index.
        let mut repainted = Vec::new();
        let mut before = 0;
        let mut index = 0;
        while index < self.primitives.len() {
            if let Some(range) = unchanged.peek()
                && range.start == index
            {
                index = range.end;
                unchanged.next();
                continue;
            }
            while self
                .in_place_sources
                .get(before)
                .is_some_and(|source| source.start < index)
            {
                before += 1;
            }
            if index < end && self.repainted_same(index, before) {
                repainted.push(index);
            } else {
                damage = union(damage, seen(damage_reach(&self.primitives[index])));
                if damage == Reach::Unbounded {
                    return damage;
                }
            }
            index += 1;
        }
        // Replayed primitives were moved out, leaving clip pops that draw nothing.
        let mut repainted = repainted.into_iter().peekable();
        for (index, primitive) in self.previous_primitives[..end].iter().enumerate() {
            if repainted.next_if_eq(&index).is_some() {
                continue;
            }
            damage = union(damage, seen(damage_reach(primitive)));
            if damage == Reach::Unbounded {
                return damage;
            }
        }
        damage
    }

    /// Whether primitive `index`, painted rather than replayed, is the one the
    /// previous frame drew at `index` and drawn in the same order against
    /// everything replayed in place, so where only it draws nothing changes.
    /// `before` in-place replays landed before `index`.
    ///
    /// An ancestor whose subtree changed paints its background again (#343);
    /// without this that background's whole box is damage.
    fn repainted_same(&self, index: usize, before: usize) -> bool {
        let sources = &self.in_place_sources;
        // The replays before it in this frame were before it in the last, and
        // the ones after it after it.
        let in_order = before
            .checked_sub(1)
            .is_none_or(|last| sources[last].previous_end <= index)
            && sources
                .get(before)
                .is_none_or(|next| next.previous_start > index);
        in_order && draws_the_same(&self.primitives[index], &self.previous_primitives[index])
    }
}

/// Whether `a` and `b` draw exactly the same. Only boxes and shadows are
/// compared: a clip changes what other primitives draw, and text, images and
/// paths cost more to compare than they save. Anything else counts as
/// different.
fn draws_the_same(a: &Primitive, b: &Primitive) -> bool {
    match (a, b) {
        (
            Primitive::Quad {
                bounds,
                background,
                border_color,
                border_widths,
                corner_radii,
            },
            Primitive::Quad {
                bounds: b_bounds,
                background: b_background,
                border_color: b_border_color,
                border_widths: b_border_widths,
                corner_radii: b_corner_radii,
            },
        ) => {
            bounds == b_bounds
                && background == b_background
                && border_color == b_border_color
                && border_widths == b_border_widths
                && corner_radii == b_corner_radii
        }
        (
            Primitive::Shadow {
                bounds,
                corner_radii,
                blur_radius,
                color,
            },
            Primitive::Shadow {
                bounds: b_bounds,
                corner_radii: b_corner_radii,
                blur_radius: b_blur_radius,
                color: b_color,
            },
        ) => {
            bounds == b_bounds
                && corner_radii == b_corner_radii
                && blur_radius == b_blur_radius
                && color == b_color
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::color::Rgba;
    use crate::core::geometry::Edges;
    use crate::core::style::Corners;

    fn quad(x: f32, width: f32) -> Primitive {
        Primitive::Quad {
            bounds: Bounds::from_xywh(x, 0.0, width, 10.0),
            background: Rgba::new(1.0, 1.0, 1.0, 1.0),
            border_color: Rgba::new(0.0, 0.0, 0.0, 0.0),
            border_widths: Edges::ZERO,
            corner_radii: Corners::ZERO,
        }
    }

    fn window() -> Bounds {
        Bounds::from_xywh(0.0, 0.0, 1_000.0, 100.0)
    }

    #[test]
    fn a_background_painted_again_the_same_is_not_damage() {
        let mut scene = Scene::new();
        scene.insert(quad(0.0, 1_000.0));
        scene.insert(quad(10.0, 10.0));
        scene.insert(quad(30.0, 10.0));
        scene.clear();
        // The background again, the first box replayed, the second changed.
        scene.insert(quad(0.0, 1_000.0));
        assert_eq!(scene.replay(1..2, 0.0, 0.0), Some(1..2));
        scene.insert(quad(31.0, 10.0));
        // Both places of the changed box, with a pixel for antialiasing.
        assert_eq!(
            scene.damage(window()),
            Reach::Within(Bounds::from_xywh(29.0, 0.0, 13.0, 11.0))
        );
    }

    #[test]
    fn a_background_a_replay_moved_across_is_damage() {
        let mut scene = Scene::new();
        scene.insert(quad(10.0, 10.0));
        scene.insert(quad(0.0, 1_000.0));
        scene.clear();
        // A new box first: the same background is at the same index, but the
        // box replayed in place is now drawn over it rather than under it.
        scene.insert(quad(500.0, 10.0));
        scene.insert(quad(0.0, 1_000.0));
        assert_eq!(scene.replay(0..1, 0.0, 0.0), Some(2..3));
        assert_eq!(
            scene.damage(window()),
            Reach::Within(Bounds::from_xywh(0.0, 0.0, 1_000.0, 11.0))
        );
    }
}
