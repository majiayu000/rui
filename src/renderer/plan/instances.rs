//! Instance layouts the planner fills and every backend's shaders read. Their
//! fields, in order, are the shader structs of the same name.

use crate::core::geometry::Bounds;
use crate::core::style::Corners;
use bytemuck::{Pod, Zeroable};

/// A glyph, color glyph or path sprite drawn from an atlas page.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct GpuGlyph {
    pub bounds: [f32; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct GpuImage {
    pub bounds: [f32; 4],
    pub corner_radii: [f32; 4],
    pub color: [f32; 4],
    pub opacity: f32,
    pub _padding: [f32; 3],
}

impl GpuImage {
    pub(crate) fn new(
        bounds: Bounds,
        corner_radii: Corners,
        color: [f32; 4],
        opacity: f32,
    ) -> Self {
        Self {
            bounds: [bounds.x(), bounds.y(), bounds.width(), bounds.height()],
            corner_radii: [
                corner_radii.top_left,
                corner_radii.top_right,
                corner_radii.bottom_right,
                corner_radii.bottom_left,
            ],
            color,
            opacity,
            _padding: [0.0; 3],
        }
    }
}
