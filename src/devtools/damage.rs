//! The damage and replay visualizer: tints what changed this frame.

use crate::core::color::Rgba;
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::Corners;
use crate::renderer::{Primitive, Reach, Scene};

/// Subtrees replayed from the previous frame.
const REPLAYED: Rgba = Rgba::new(0.15, 0.85, 0.35, 0.22);
/// Subtrees that could have been replayed but were painted again.
const REPAINTED: Rgba = Rgba::new(1.0, 0.25, 0.2, 0.85);
/// The damage the renderer draws again.
const DAMAGE: Rgba = Rgba::new(1.0, 0.1, 0.9, 0.9);

/// Fill each memoizable subtree replayed this frame, outline each one painted
/// again, and outline `damage`.
pub(super) fn paint(scene: &mut Scene, damage: Reach, window: Bounds) {
    let tints: Vec<Primitive> = scene
        .paint_records()
        .iter()
        // Left out of this frame: nothing drawn to tint.
        .filter(|record| record.painted && record.start < record.end)
        .map(|record| {
            let bounds = Bounds::new(record.origin, record.unrounded_size);
            if record.replayed {
                quad(bounds, REPLAYED, Rgba::TRANSPARENT, 0.0)
            } else {
                quad(bounds, Rgba::TRANSPARENT, REPAINTED, 1.0)
            }
        })
        .collect();
    for tint in tints {
        scene.insert(tint);
    }
    let damaged = match damage {
        Reach::Nothing => return,
        Reach::Unbounded => Some(window),
        Reach::Within(bounds) => bounds.intersection(&window),
    };
    if let Some(bounds) = damaged {
        scene.insert(quad(
            bounds,
            Rgba::new(DAMAGE.r, DAMAGE.g, DAMAGE.b, 0.06),
            DAMAGE,
            2.0,
        ));
    }
}

fn quad(bounds: Bounds, background: Rgba, border: Rgba, width: f32) -> Primitive {
    Primitive::Quad {
        bounds,
        background,
        border_color: border,
        border_widths: Edges::all(width),
        corner_radii: Corners::ZERO,
    }
}
