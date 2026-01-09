//! Scene graph - collects primitives for rendering

use crate::core::geometry::Bounds;
use crate::renderer::primitives::{GpuQuad, GpuShadow, Primitive};
use smallvec::SmallVec;

/// Draw order type
pub type DrawOrder = u32;

/// A scene collects all primitives to be rendered in a frame
#[derive(Default)]
pub struct Scene {
    /// All primitives in draw order
    pub(crate) primitives: Vec<(DrawOrder, Primitive)>,

    /// Current draw order
    order: DrawOrder,

    /// Layer stack for nested clipping
    layer_stack: SmallVec<[Bounds; 8]>,

    /// Collected quads for batched rendering
    pub(crate) quads: Vec<GpuQuad>,

    /// Collected shadows for batched rendering
    pub(crate) shadows: Vec<GpuShadow>,

    /// Text items (handled separately)
    pub(crate) text_items: Vec<TextItem>,
}

/// Text rendering item
#[derive(Debug, Clone)]
pub struct TextItem {
    pub bounds: Bounds,
    pub content: String,
    pub color: [f32; 4],
    pub font_size: f32,
    pub font_weight: u16,
    pub font_family: Option<String>,
    pub order: DrawOrder,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear the scene for a new frame
    pub fn clear(&mut self) {
        self.primitives.clear();
        self.order = 0;
        self.layer_stack.clear();
        self.quads.clear();
        self.shadows.clear();
        self.text_items.clear();
    }

    /// Insert a primitive into the scene
    pub fn insert(&mut self, primitive: Primitive) {
        let order = self.order;
        self.order += 1;

        // Also add to batched collections for GPU
        match &primitive {
            Primitive::Quad {
                bounds,
                background,
                border_color,
                border_widths,
                corner_radii,
            } => {
                self.quads.push(GpuQuad::from_primitive(
                    *bounds,
                    *background,
                    *border_color,
                    *border_widths,
                    *corner_radii,
                ));
            }
            Primitive::Shadow {
                bounds,
                corner_radii,
                blur_radius,
                color,
            } => {
                self.shadows.push(GpuShadow::from_primitive(
                    *bounds,
                    *corner_radii,
                    *blur_radius,
                    *color,
                ));
            }
            Primitive::Text {
                bounds,
                content,
                color,
                font_size,
                font_weight,
                font_family,
                ..
            } => {
                self.text_items.push(TextItem {
                    bounds: *bounds,
                    content: content.clone(),
                    color: color.to_array(),
                    font_size: *font_size,
                    font_weight: *font_weight,
                    font_family: font_family.clone(),
                    order,
                });
            }
            _ => {}
        }

        self.primitives.push((order, primitive));
    }

    /// Push a clipping layer
    pub fn push_layer(&mut self, bounds: Bounds) {
        self.layer_stack.push(bounds);
        self.insert(Primitive::PushClip {
            bounds,
            corner_radii: Default::default(),
        });
    }

    /// Pop the current clipping layer
    pub fn pop_layer(&mut self) {
        self.layer_stack.pop();
        self.insert(Primitive::PopClip);
    }

    /// Get the current clipping bounds
    pub fn current_clip(&self) -> Option<Bounds> {
        self.layer_stack.last().copied()
    }

    /// Get number of primitives
    pub fn len(&self) -> usize {
        self.primitives.len()
    }

    /// Check if scene is empty
    pub fn is_empty(&self) -> bool {
        self.primitives.is_empty()
    }

    /// Sort primitives by draw order (usually already sorted)
    pub fn finish(&mut self) {
        // Primitives are already in order, but we could sort here if needed
        // self.primitives.sort_by_key(|(order, _)| *order);
    }

    /// Iterate over quads
    pub fn quads(&self) -> &[GpuQuad] {
        &self.quads
    }

    /// Iterate over shadows
    pub fn shadows(&self) -> &[GpuShadow] {
        &self.shadows
    }

    /// Iterate over text items
    pub fn text_items(&self) -> &[TextItem] {
        &self.text_items
    }
}
