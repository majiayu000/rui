//! Rendering primitives - the basic shapes the GPU can draw

use crate::core::color::Rgba;
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::Corners;
use crate::{ImageFit, ImageSource};
use crate::elements::text::TextAlign;
use bytemuck::{Pod, Zeroable};

/// GPU-renderable primitives
#[derive(Debug, Clone)]
pub enum Primitive {
    /// A filled/stroked rectangle with optional rounded corners
    Quad {
        bounds: Bounds,
        background: Rgba,
        border_color: Rgba,
        border_widths: Edges,
        corner_radii: Corners,
    },

    /// A shadow behind an element
    Shadow {
        bounds: Bounds,
        corner_radii: Corners,
        blur_radius: f32,
        color: Rgba,
    },

    /// Linear gradient fill
    LinearGradient {
        bounds: Bounds,
        start: Rgba,
        end: Rgba,
        angle: f32,
        border_color: Rgba,
        border_widths: Edges,
        corner_radii: Corners,
    },

    /// Radial gradient fill
    RadialGradient {
        bounds: Bounds,
        inner: Rgba,
        outer: Rgba,
        border_color: Rgba,
        border_widths: Edges,
        corner_radii: Corners,
    },

    /// Text rendering
    Text {
        bounds: Bounds,
        content: String,
        color: Rgba,
        font_size: f32,
        font_weight: u16,
        font_family: Option<String>,
        line_height: f32,
        align: TextAlign,
    },

    /// Image rendering
    Image {
        bounds: Bounds,
        source: ImageSource,
        fit: ImageFit,
        corner_radii: Corners,
        opacity: f32,
    },

    /// Path (for custom vector graphics)
    Path {
        vertices: Vec<PathVertex>,
        color: Rgba,
        stroke_width: Option<f32>,
    },

    /// Clipping mask push
    PushClip {
        bounds: Bounds,
        corner_radii: Corners,
    },

    /// Clipping mask pop
    PopClip,
}

/// Vertex for path rendering
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct PathVertex {
    pub position: [f32; 2],
}

impl PathVertex {
    pub fn new(x: f32, y: f32) -> Self {
        Self { position: [x, y] }
    }
}

/// GPU-ready quad data (matches shader layout)
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct GpuQuad {
    // Bounds (origin x, y, size width, height)
    pub bounds: [f32; 4],
    // Background color
    pub background: [f32; 4],
    // Border color
    pub border_color: [f32; 4],
    // Border widths (top, right, bottom, left)
    pub border_widths: [f32; 4],
    // Corner radii (top_left, top_right, bottom_right, bottom_left)
    pub corner_radii: [f32; 4],
    // Gradient start color (for gradients)
    pub gradient_start: [f32; 4],
    // Gradient end color (for gradients)
    pub gradient_end: [f32; 4],
    // Gradient params: x = fill_type (0 solid, 1 linear, 2 radial), y = angle (radians)
    pub gradient_params: [f32; 4],
}

impl GpuQuad {
    pub fn solid(
        bounds: Bounds,
        background: Rgba,
        border_color: Rgba,
        border_widths: Edges,
        corner_radii: Corners,
    ) -> Self {
        Self {
            bounds: [bounds.x(), bounds.y(), bounds.width(), bounds.height()],
            background: background.to_array(),
            border_color: border_color.to_array(),
            border_widths: [
                border_widths.top,
                border_widths.right,
                border_widths.bottom,
                border_widths.left,
            ],
            corner_radii: [
                corner_radii.top_left,
                corner_radii.top_right,
                corner_radii.bottom_right,
                corner_radii.bottom_left,
            ],
            gradient_start: [0.0; 4],
            gradient_end: [0.0; 4],
            gradient_params: [0.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn linear_gradient(
        bounds: Bounds,
        start: Rgba,
        end: Rgba,
        angle_radians: f32,
        border_color: Rgba,
        border_widths: Edges,
        corner_radii: Corners,
    ) -> Self {
        Self {
            bounds: [bounds.x(), bounds.y(), bounds.width(), bounds.height()],
            background: [0.0; 4],
            border_color: border_color.to_array(),
            border_widths: [
                border_widths.top,
                border_widths.right,
                border_widths.bottom,
                border_widths.left,
            ],
            corner_radii: [
                corner_radii.top_left,
                corner_radii.top_right,
                corner_radii.bottom_right,
                corner_radii.bottom_left,
            ],
            gradient_start: start.to_array(),
            gradient_end: end.to_array(),
            gradient_params: [1.0, angle_radians, 0.0, 0.0],
        }
    }

    pub fn radial_gradient(
        bounds: Bounds,
        inner: Rgba,
        outer: Rgba,
        border_color: Rgba,
        border_widths: Edges,
        corner_radii: Corners,
    ) -> Self {
        Self {
            bounds: [bounds.x(), bounds.y(), bounds.width(), bounds.height()],
            background: [0.0; 4],
            border_color: border_color.to_array(),
            border_widths: [
                border_widths.top,
                border_widths.right,
                border_widths.bottom,
                border_widths.left,
            ],
            corner_radii: [
                corner_radii.top_left,
                corner_radii.top_right,
                corner_radii.bottom_right,
                corner_radii.bottom_left,
            ],
            gradient_start: inner.to_array(),
            gradient_end: outer.to_array(),
            gradient_params: [2.0, 0.0, 0.0, 0.0],
        }
    }
}

/// GPU-ready shadow data
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct GpuShadow {
    pub bounds: [f32; 4],
    pub corner_radii: [f32; 4],
    pub blur_radius: f32,
    pub color: [f32; 4],
    pub _padding: [f32; 3],
}

impl GpuShadow {
    pub fn from_primitive(bounds: Bounds, corner_radii: Corners, blur_radius: f32, color: Rgba) -> Self {
        Self {
            bounds: [bounds.x(), bounds.y(), bounds.width(), bounds.height()],
            corner_radii: [
                corner_radii.top_left,
                corner_radii.top_right,
                corner_radii.bottom_right,
                corner_radii.bottom_left,
            ],
            blur_radius,
            color: color.to_array(),
            _padding: [0.0; 3],
        }
    }
}
