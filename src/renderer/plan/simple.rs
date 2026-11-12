//! Which sprites the simple pipeline can draw: glyphs and pixel-aligned plain
//! quads that need no clip test. None does whose clip cuts nothing the scissor does not, nor a
//! quad its clip leaves whole, nor a glyph run whose extent lies within it.
//! Rectangles here are `[x0, y0, -x1, -y1]`, and lying within is each lane at
//! or above the clip's.

use super::order::Rect;

/// Whether every edge of `rect` times `scale` lies on a device pixel edge.
/// Every plain quad asks, so where there is a vector unit one instruction
/// takes all four edges.
#[cfg(target_arch = "aarch64")]
pub(super) fn on_device_pixels(rect: Rect, scale: f32) -> bool {
    use std::arch::aarch64::{vabdq_f32, vld1q_f32, vmaxvq_f32, vmulq_n_f32, vrndaq_f32};
    // SAFETY: NEON is part of every aarch64 target; `rect` holds four f32s.
    unsafe {
        let device = vmulq_n_f32(vld1q_f32(rect.as_ptr()), scale);
        vmaxvq_f32(vabdq_f32(device, vrndaq_f32(device))) < 1e-3
    }
}

#[cfg(not(target_arch = "aarch64"))]
pub(super) fn on_device_pixels(rect: Rect, scale: f32) -> bool {
    rect.iter()
        .all(|&edge| (edge * scale - (edge * scale).round()).abs() < 1e-3)
}

/// `[x0, y0, x1, y1]` as `[x0, y0, -x1, -y1]`.
pub(super) fn flipped(rect: Rect) -> Rect {
    [rect[0], rect[1], -rect[2], -rect[3]]
}

/// A clip's device pixel `edges`, flipped and in logical pixels, as the bounds
/// a sprite must lie within to need no clip test: unbounded where `frame`, the
/// scissor of every sprite draw, cuts as far, and `None` where it cuts as far
/// on every side. A sprite a rounding error past the clip covers no pixel
/// center beyond it, so logical pixels do.
pub(super) fn clip_bounds(edges: Rect, frame: Rect, scale_factor: f32) -> Option<Rect> {
    let bound = |edge: f32, frame: f32| {
        if edge > frame {
            edge / scale_factor
        } else {
            f32::NEG_INFINITY
        }
    };
    let (edges, frame) = (flipped(edges), flipped(frame));
    let bounds = [
        bound(edges[0], frame[0]),
        bound(edges[1], frame[1]),
        bound(edges[2], frame[2]),
        bound(edges[3], frame[3]),
    ];
    (bounds != [f32::NEG_INFINITY; 4]).then_some(bounds)
}

/// Whether flipped `rect` lies within flipped clip `bounds`.
pub(super) fn inside(bounds: &Rect, rect: Rect) -> bool {
    (rect[0] >= bounds[0])
        & (rect[1] >= bounds[1])
        & (rect[2] >= bounds[2])
        & (rect[3] >= bounds[3])
}

/// The flipped rectangle covering both.
pub(super) fn extend(a: Rect, b: Rect) -> Rect {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].min(b[2]),
        a[3].min(b[3]),
    ]
}
