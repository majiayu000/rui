//! What a kept texture shows, and the device pixels a frame draws again into it.
//!
//! A window's drawables rotate and do not keep their contents, so a backend
//! draws into a texture it keeps instead: only the primitives that touch the
//! scene's damage are drawn again, scissored to it, over what the texture
//! already shows, and the texture is then copied to the drawable.

use crate::core::geometry::{Bounds, Size};
use crate::renderer::{Reach, Scene};

/// A rectangle of device pixels: a clip, or the damage of a retained frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ScissorRect {
    pub x: u64,
    pub y: u64,
    pub width: u64,
    pub height: u64,
}

/// Which scene a kept texture shows. One per kept texture.
#[derive(Debug, Default)]
pub(crate) struct RetainedFrame {
    /// [`Scene::frame`] of the scene the texture shows; 0 when it shows none.
    frame: u64,
}

impl RetainedFrame {
    /// The part of the kept texture `scene` has to draw again: `None` for all
    /// of it. `kept` says whether the texture is the one drawn last time. Until
    /// [`Self::drew`], the texture shows nothing reliable.
    pub(crate) fn damage(
        &mut self,
        kept: bool,
        scene: &Scene,
        viewport_size: Size,
    ) -> Option<Bounds> {
        let shows_previous = self.frame != 0 && self.frame == scene.previous_frame();
        self.frame = 0;
        if !(kept && shows_previous) {
            return None;
        }
        let viewport = Bounds::from_xywh(0.0, 0.0, viewport_size.width, viewport_size.height);
        match scene.damage(viewport) {
            Reach::Nothing => Some(Bounds::from_xywh(0.0, 0.0, 0.0, 0.0)),
            Reach::Within(bounds) => Some(
                bounds
                    .intersection(&viewport)
                    .unwrap_or(Bounds::from_xywh(0.0, 0.0, 0.0, 0.0)),
            ),
            Reach::Unbounded => None,
        }
    }

    /// Record that the texture now shows `scene`.
    pub(crate) fn drew(&mut self, scene: &Scene) {
        self.frame = scene.frame();
    }
}

/// The device pixels covering `damage`, rounded outwards so every pixel it
/// touches is redrawn, within a `drawable_size` target.
pub(crate) fn damage_scissor(
    damage: Bounds,
    scale_factor: f32,
    drawable_size: Size,
) -> ScissorRect {
    let clamp = |value: f32, max: f32| value.clamp(0.0, max) as u64;
    let x0 = clamp((damage.x() * scale_factor).floor(), drawable_size.width);
    let y0 = clamp((damage.y() * scale_factor).floor(), drawable_size.height);
    let x1 = clamp(
        ((damage.x() + damage.width()) * scale_factor).ceil(),
        drawable_size.width,
    );
    let y1 = clamp(
        ((damage.y() + damage.height()) * scale_factor).ceil(),
        drawable_size.height,
    );
    ScissorRect {
        x: x0,
        y: y0,
        width: x1.saturating_sub(x0),
        height: y1.saturating_sub(y0),
    }
}

/// `a` and `b` overlap in.
pub(crate) fn intersect_scissors(a: ScissorRect, b: ScissorRect) -> ScissorRect {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.width).min(b.x + b.width);
    let y1 = (a.y + a.height).min(b.y + b.height);
    ScissorRect {
        x: x0,
        y: y0,
        width: x1.saturating_sub(x0),
        height: y1.saturating_sub(y0),
    }
}
