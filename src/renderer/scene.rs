//! Scene graph - collects primitives for rendering

use crate::core::geometry::Bounds;
use crate::renderer::primitives::Primitive;
use smallvec::SmallVec;

/// A scene collects all primitives to be rendered in a frame
#[derive(Default)]
pub struct Scene {
    /// All primitives in draw order
    pub(crate) primitives: Vec<Primitive>,

    /// Layer stack for nested clipping
    layer_stack: SmallVec<[Bounds; 8]>,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear the scene for a new frame
    pub fn clear(&mut self) {
        self.primitives.clear();
        self.layer_stack.clear();
    }

    /// Insert a primitive into the scene
    pub fn insert(&mut self, primitive: Primitive) {
        self.primitives.push(primitive);
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
        // Primitives are already in order of insertion.
    }

    /// Iterate over primitives
    pub fn primitives(&self) -> &[Primitive] {
        &self.primitives
    }
}
