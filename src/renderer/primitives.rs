//! Rendering primitives - the basic shapes the GPU can draw

use crate::core::color::Rgba;
use crate::core::geometry::{Bounds, Edges, Size};
use crate::core::style::Corners;
use crate::elements::fit_bounds;
use crate::elements::text::TextAlign;
use crate::{ImageFit, ImageSource};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveKind {
    Quad,
    Shadow,
    LinearGradient,
    RadialGradient,
    Text,
    Image,
    Path,
    PushClip,
    PopClip,
}

impl PrimitiveKind {
    pub const ALL: [Self; 9] = [
        Self::Quad,
        Self::Shadow,
        Self::LinearGradient,
        Self::RadialGradient,
        Self::Text,
        Self::Image,
        Self::Path,
        Self::PushClip,
        Self::PopClip,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Quad => "quad",
            Self::Shadow => "shadow",
            Self::LinearGradient => "linear_gradient",
            Self::RadialGradient => "radial_gradient",
            Self::Text => "text",
            Self::Image => "image",
            Self::Path => "path",
            Self::PushClip => "push_clip",
            Self::PopClip => "pop_clip",
        }
    }
}

impl Primitive {
    /// Whether [`Self::translate`] by `(dx, dy)` gives bit for bit the
    /// coordinates painting at the moved origin computes.
    ///
    /// Paint computes a coordinate as an integral origin plus an offset, so
    /// moving it by a whole number is exact unless the sum changes binade,
    /// where rounding to the coarser grid can land on a neighbouring float.
    pub fn translates_exactly(&self, dx: f32, dy: f32) -> bool {
        fn exact(value: f32, by: f32) -> bool {
            if by == 0.0 {
                return true;
            }
            let moved = value + by;
            by.fract() == 0.0
                && moved.is_finite()
                && ((value.fract() == 0.0 && moved.abs() < 16_777_216.0)
                    || (value.is_sign_negative() == moved.is_sign_negative()
                        && value.abs().to_bits() >> 23 == moved.abs().to_bits() >> 23))
        }
        let bounds = |bounds: &Bounds| exact(bounds.origin.x, dx) && exact(bounds.origin.y, dy);
        match self {
            Self::Quad { bounds: b, .. }
            | Self::Shadow { bounds: b, .. }
            | Self::LinearGradient { bounds: b, .. }
            | Self::RadialGradient { bounds: b, .. }
            | Self::Text { bounds: b, .. }
            | Self::Image { bounds: b, .. }
            | Self::PushClip { bounds: b, .. } => bounds(b),
            Self::Path { vertices, .. } => vertices
                .iter()
                .all(|vertex| exact(vertex.position[0], dx) && exact(vertex.position[1], dy)),
            Self::PopClip => true,
        }
    }

    /// Move the primitive by `(dx, dy)`.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        let shift = |bounds: &mut Bounds| {
            bounds.origin.x += dx;
            bounds.origin.y += dy;
        };
        match self {
            Self::Quad { bounds, .. }
            | Self::Shadow { bounds, .. }
            | Self::LinearGradient { bounds, .. }
            | Self::RadialGradient { bounds, .. }
            | Self::Text { bounds, .. }
            | Self::Image { bounds, .. }
            | Self::PushClip { bounds, .. } => shift(bounds),
            Self::Path { vertices, .. } => {
                for vertex in vertices {
                    vertex.position[0] += dx;
                    vertex.position[1] += dy;
                }
            }
            Self::PopClip => {}
        }
    }

    pub fn kind(&self) -> PrimitiveKind {
        match self {
            Self::Quad { .. } => PrimitiveKind::Quad,
            Self::Shadow { .. } => PrimitiveKind::Shadow,
            Self::LinearGradient { .. } => PrimitiveKind::LinearGradient,
            Self::RadialGradient { .. } => PrimitiveKind::RadialGradient,
            Self::Text { .. } => PrimitiveKind::Text,
            Self::Image { .. } => PrimitiveKind::Image,
            Self::Path { .. } => PrimitiveKind::Path,
            Self::PushClip { .. } => PrimitiveKind::PushClip,
            Self::PopClip => PrimitiveKind::PopClip,
        }
    }

    /// Whether this primitive can put any pixel inside `visible`.
    ///
    /// Conservative: `true` whenever that cannot be ruled out from the primitive
    /// alone. Text is centred on its ink, which can be wider than its bounds,
    /// so it is not ruled out horizontally.
    pub fn may_touch(&self, visible: &Bounds) -> bool {
        match self.reach() {
            Reach::Nothing => false,
            Reach::Within(reach) => reach.intersects(visible),
            Reach::Unbounded => true,
        }
    }

    /// Everything the primitive can draw on.
    pub fn reach(&self) -> Reach {
        // Antialiasing reaches about one pixel past an edge.
        const EDGE: f32 = 1.0;
        Reach::Within(match self {
            Self::Quad { bounds, .. }
            | Self::LinearGradient { bounds, .. }
            | Self::RadialGradient { bounds, .. } => outset(*bounds, EDGE, EDGE),
            Self::Shadow {
                bounds,
                blur_radius,
                ..
            } => {
                let blur = blur_radius.max(0.0) * 3.0 + EDGE;
                outset(*bounds, blur, blur)
            }
            Self::Text {
                bounds, font_size, ..
            } => outset(*bounds, f32::INFINITY, font_size.max(0.0) * 2.0 + EDGE),
            Self::Path {
                vertices,
                stroke_width,
                ..
            } => {
                let Some(first) = vertices.first() else {
                    return Reach::Nothing;
                };
                let (mut x0, mut y0) = (first.position[0], first.position[1]);
                let (mut x1, mut y1) = (x0, y0);
                for vertex in vertices {
                    x0 = x0.min(vertex.position[0]);
                    y0 = y0.min(vertex.position[1]);
                    x1 = x1.max(vertex.position[0]);
                    y1 = y1.max(vertex.position[1]);
                }
                let pad = stroke_width.unwrap_or(0.0).max(0.0) * 0.5 + EDGE;
                outset(Bounds::from_xywh(x0, y0, x1 - x0, y1 - y0), pad, pad)
            }
            // The renderer clips an image to its bounds, so a fit that lands
            // past them (cover, none) draws no further. With its size known, a
            // fit that letterboxes (contain, scale down) draws less.
            Self::Image {
                bounds,
                source,
                fit,
                ..
            } => {
                let drawn = match source {
                    ImageSource::Data { width, height, .. } => {
                        let size = Size::new(*width as f32, *height as f32);
                        fit_bounds(*bounds, size, *fit).intersection(bounds)
                    }
                    ImageSource::Decoded(image) => {
                        let size = Size::new(image.width() as f32, image.height() as f32);
                        fit_bounds(*bounds, size, *fit).intersection(bounds)
                    }
                    ImageSource::File(_) | ImageSource::Texture(_) => Some(*bounds),
                };
                match drawn {
                    Some(drawn) if !drawn.is_empty() => outset(drawn, EDGE, EDGE),
                    _ => return Reach::Nothing,
                }
            }
            Self::PushClip { .. } | Self::PopClip => return Reach::Unbounded,
        })
    }
}

/// What a primitive can draw on; see [`Primitive::reach`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Reach {
    /// Nothing, as an empty path.
    Nothing,
    /// Only pixels inside these bounds.
    Within(Bounds),
    /// Cannot be bounded from the primitive alone: a clip changes what other
    /// primitives show.
    Unbounded,
}

fn outset(bounds: Bounds, dx: f32, dy: f32) -> Bounds {
    let (x, width) = if dx.is_infinite() {
        (f32::MIN / 4.0, f32::MAX / 2.0)
    } else {
        (bounds.x() - dx, bounds.width() + dx * 2.0)
    };
    Bounds::from_xywh(x, bounds.y() - dy, width, bounds.height() + dy * 2.0)
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
    // Gradient params: x = fill_type (0 solid, 1 linear, 2 radial), y = angle (radians),
    // z = 1 for a plain quad whose edges lie on device pixels
    pub gradient_params: [f32; 4],
}

impl GpuQuad {
    /// A solid fill with square corners and no border.
    pub(crate) fn is_plain(&self) -> bool {
        self.corner_radii == [0.0; 4]
            && self.border_widths == [0.0; 4]
            && self.gradient_params[0] < 0.5
    }

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
    pub color: [f32; 4],
    pub blur_radius: f32,
    pub _padding: [f32; 3],
}

impl GpuShadow {
    pub fn from_primitive(
        bounds: Bounds,
        corner_radii: Corners,
        blur_radius: f32,
        color: Rgba,
    ) -> Self {
        Self {
            bounds: [bounds.x(), bounds.y(), bounds.width(), bounds.height()],
            corner_radii: [
                corner_radii.top_left,
                corner_radii.top_right,
                corner_radii.bottom_right,
                corner_radii.bottom_left,
            ],
            color: color.to_array(),
            blur_radius,
            _padding: [0.0; 3],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> Bounds {
        Bounds::from_xywh(0.0, 0.0, 100.0, 100.0)
    }

    fn quad(bounds: Bounds) -> Primitive {
        Primitive::Quad {
            bounds,
            background: Rgba::new(1.0, 1.0, 1.0, 1.0),
            border_color: Rgba::new(0.0, 0.0, 0.0, 0.0),
            border_widths: Edges::ZERO,
            corner_radii: Corners::all(0.0),
        }
    }

    fn text(bounds: Bounds) -> Primitive {
        Primitive::Text {
            bounds,
            content: "label".to_string(),
            color: Rgba::new(1.0, 1.0, 1.0, 1.0),
            font_size: 14.0,
            font_weight: 400,
            font_family: None,
            line_height: 1.2,
            align: TextAlign::Left,
        }
    }

    #[test]
    fn quads_outside_are_ruled_out_and_edge_antialiasing_is_kept() {
        assert!(!quad(Bounds::from_xywh(0.0, 200.0, 50.0, 20.0)).may_touch(&view()));
        assert!(quad(Bounds::from_xywh(0.0, 99.0, 50.0, 20.0)).may_touch(&view()));
        // Ends half a pixel above the view: its antialiased edge can still land inside.
        assert!(quad(Bounds::from_xywh(0.0, -20.5, 50.0, 20.0)).may_touch(&view()));
    }

    #[test]
    fn shadow_blur_reaching_into_view_is_kept() {
        let shadow = |y| Primitive::Shadow {
            bounds: Bounds::from_xywh(0.0, y, 50.0, 20.0),
            corner_radii: Corners::all(0.0),
            blur_radius: 10.0,
            color: Rgba::new(0.0, 0.0, 0.0, 0.5),
        };
        assert!(shadow(120.0).may_touch(&view()));
        assert!(!shadow(200.0).may_touch(&view()));
    }

    #[test]
    fn text_is_only_ruled_out_vertically() {
        // Narrow bounds far to the side: centred ink may still overflow into view.
        assert!(text(Bounds::from_xywh(400.0, 10.0, 1.0, 20.0)).may_touch(&view()));
        assert!(!text(Bounds::from_xywh(0.0, 400.0, 100.0, 20.0)).may_touch(&view()));
    }

    #[test]
    fn paths_use_their_vertex_extent_and_stroke() {
        let path = |stroke_width| Primitive::Path {
            vertices: vec![PathVertex::new(110.0, 10.0), PathVertex::new(150.0, 10.0)],
            color: Rgba::new(1.0, 1.0, 1.0, 1.0),
            stroke_width,
        };
        assert!(!path(None).may_touch(&view()));
        assert!(path(Some(24.0)).may_touch(&view()));
        let empty = Primitive::Path {
            vertices: Vec::new(),
            color: Rgba::new(1.0, 1.0, 1.0, 1.0),
            stroke_width: None,
        };
        assert!(!empty.may_touch(&view()));
    }

    fn image(bounds: Bounds, source: ImageSource, fit: ImageFit) -> Primitive {
        Primitive::Image {
            bounds,
            source,
            fit,
            corner_radii: Corners::all(0.0),
            opacity: 1.0,
        }
    }

    fn data(width: u32, height: u32) -> ImageSource {
        ImageSource::Data {
            data: vec![0; (width * height * 4) as usize],
            width,
            height,
        }
    }

    #[test]
    fn images_reach_no_further_than_their_bounds() {
        let bounds = Bounds::from_xywh(10.0, 20.0, 40.0, 20.0);
        let edge = Reach::Within(Bounds::from_xywh(9.0, 19.0, 42.0, 22.0));
        for source in [
            data(400, 10),
            data(10, 400),
            ImageSource::File("photo.png".to_string()),
            ImageSource::Texture(3),
        ] {
            for fit in [ImageFit::Fill, ImageFit::Cover] {
                assert_eq!(image(bounds, source.clone(), fit).reach(), edge, "{fit:?}");
            }
        }
        assert_eq!(image(bounds, data(400, 400), ImageFit::None).reach(), edge);
        // Unknown size: contain may fill the bounds.
        let file = ImageSource::File("photo.png".to_string());
        assert_eq!(image(bounds, file, ImageFit::Contain).reach(), edge);
    }

    #[test]
    fn letterboxed_images_reach_only_what_they_draw() {
        let bounds = Bounds::from_xywh(0.0, 0.0, 40.0, 20.0);
        // A square in a 40 x 20 box: 20 x 20 in the middle.
        let square = Reach::Within(Bounds::from_xywh(9.0, -1.0, 22.0, 22.0));
        assert_eq!(image(bounds, data(8, 8), ImageFit::Contain).reach(), square);
        assert_eq!(
            image(bounds, data(80, 80), ImageFit::ScaleDown).reach(),
            square
        );
        // Small enough to draw at its own size.
        let small = Reach::Within(Bounds::from_xywh(14.0, 4.0, 12.0, 12.0));
        assert_eq!(
            image(bounds, data(10, 10), ImageFit::ScaleDown).reach(),
            small
        );
        assert_eq!(image(bounds, data(10, 10), ImageFit::None).reach(), small);
    }

    #[test]
    fn offscreen_and_empty_images_are_ruled_out() {
        let below = Bounds::from_xywh(0.0, 300.0, 50.0, 50.0);
        for fit in [ImageFit::Cover, ImageFit::None, ImageFit::Fill] {
            assert!(!image(below, data(1, 400), fit).may_touch(&view()));
            assert!(!image(below, ImageSource::Texture(1), fit).may_touch(&view()));
        }
        let edge = Bounds::from_xywh(0.0, 100.5, 50.0, 50.0);
        assert!(image(edge, data(4, 4), ImageFit::Fill).may_touch(&view()));
        let empty = Bounds::from_xywh(10.0, 10.0, 0.0, 40.0);
        assert_eq!(
            image(empty, data(4, 4), ImageFit::Cover).reach(),
            Reach::Nothing
        );
    }

    #[test]
    fn clips_are_never_ruled_out() {
        assert!(Primitive::PopClip.may_touch(&view()));
        assert!(
            Primitive::PushClip {
                bounds: Bounds::from_xywh(500.0, 500.0, 1.0, 1.0),
                corner_radii: Corners::all(0.0),
            }
            .may_touch(&view())
        );
    }
}
