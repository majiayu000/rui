//! Scene graph - collects primitives for rendering

use crate::core::ElementId;
use crate::core::geometry::{Bounds, Point};
use crate::renderer::primitives::Primitive;
use smallvec::SmallVec;

/// Draw order for scene layers and hit regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ZIndex(pub i32);

impl ZIndex {
    pub const ROOT: Self = Self(0);

    pub const fn new(value: i32) -> Self {
        Self(value)
    }
}

impl From<i32> for ZIndex {
    fn from(value: i32) -> Self {
        Self(value)
    }
}

/// Identifier for a scene layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayerId(usize);

impl LayerId {
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A pointer-hit region registered during painting.
#[derive(Debug, Clone, PartialEq)]
pub struct HitRegion {
    pub id: ElementId,
    pub bounds: Bounds,
    pub visible_bounds: Bounds,
    pub z_index: ZIndex,
    order: usize,
}

impl HitRegion {
    fn contains(&self, point: Point) -> bool {
        self.visible_bounds.contains(point)
    }
}

/// A render layer groups primitives and hit regions at a z-index.
#[derive(Debug, Clone)]
pub struct Layer {
    id: LayerId,
    order: usize,
    z_index: ZIndex,
    clip_bounds: Option<Bounds>,
    primitive_indices: Vec<usize>,
    hit_regions: Vec<HitRegion>,
}

impl Layer {
    fn new(id: LayerId, order: usize, z_index: ZIndex, clip_bounds: Option<Bounds>) -> Self {
        Self {
            id,
            order,
            z_index,
            clip_bounds,
            primitive_indices: Vec::new(),
            hit_regions: Vec::new(),
        }
    }

    pub fn id(&self) -> LayerId {
        self.id
    }

    pub fn z_index(&self) -> ZIndex {
        self.z_index
    }

    pub fn clip_bounds(&self) -> Option<Bounds> {
        self.clip_bounds
    }

    pub fn primitive_indices(&self) -> &[usize] {
        &self.primitive_indices
    }

    pub fn hit_regions(&self) -> &[HitRegion] {
        &self.hit_regions
    }
}

/// A scene collects all primitives to be rendered in a frame
pub struct Scene {
    /// All primitives in draw order
    pub(crate) primitives: Vec<Primitive>,

    /// Layer stack for nested clipping
    layer_stack: SmallVec<[Bounds; 8]>,

    /// Render layers for future z-order and event targeting.
    layers: Vec<Layer>,

    /// Active render-layer stack.
    active_layers: SmallVec<[LayerId; 8]>,

    next_hit_order: usize,
}

impl Scene {
    pub fn new() -> Self {
        let root_id = LayerId(0);
        Self {
            primitives: Vec::new(),
            layer_stack: SmallVec::new(),
            layers: vec![Layer::new(root_id, 0, ZIndex::ROOT, None)],
            active_layers: smallvec::smallvec![root_id],
            next_hit_order: 0,
        }
    }

    /// Clear the scene for a new frame
    pub fn clear(&mut self) {
        self.primitives.clear();
        self.layer_stack.clear();
        self.layers.clear();
        self.active_layers.clear();
        let root_id = LayerId(0);
        self.layers.push(Layer::new(root_id, 0, ZIndex::ROOT, None));
        self.active_layers.push(root_id);
        self.next_hit_order = 0;
    }

    /// Insert a primitive into the scene
    pub fn insert(&mut self, primitive: Primitive) {
        let primitive_index = self.primitives.len();
        self.primitives.push(primitive);
        if let Some(layer) = self.active_layer_mut() {
            layer.primitive_indices.push(primitive_index);
        }
    }

    /// Push a render layer for future z-ordered painting and hit testing.
    pub fn push_render_layer(
        &mut self,
        z_index: impl Into<ZIndex>,
        clip_bounds: Option<Bounds>,
    ) -> LayerId {
        let id = LayerId(self.layers.len());
        let order = self.layers.len();
        let layer = Layer::new(id, order, z_index.into(), clip_bounds);
        self.layers.push(layer);
        self.active_layers.push(id);
        id
    }

    /// Pop the current render layer.
    pub fn pop_render_layer(&mut self) -> Option<LayerId> {
        if self.active_layers.len() <= 1 {
            return None;
        }
        self.active_layers.pop()
    }

    /// Get the current render layer ID.
    pub fn current_render_layer(&self) -> LayerId {
        self.active_layers.last().copied().unwrap_or(LayerId(0))
    }

    /// Get a layer by ID.
    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.get(id.index())
    }

    /// Iterate layers in creation order.
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// Return layers sorted by z-index, preserving creation order for ties.
    pub fn layers_in_z_order(&self) -> Vec<&Layer> {
        let mut layers: Vec<&Layer> = self.layers.iter().collect();
        layers.sort_by_key(|layer| (layer.z_index, layer.order));
        layers
    }

    /// Register a pointer hit region on the active layer.
    ///
    /// Returns false when the region is fully clipped.
    pub fn register_hit_region(&mut self, id: ElementId, bounds: Bounds) -> bool {
        let z_index = self
            .layer(self.current_render_layer())
            .map(|layer| layer.z_index())
            .unwrap_or(ZIndex::ROOT);
        self.register_hit_region_at(id, bounds, z_index)
    }

    /// Register a pointer hit region with an explicit z-index override.
    ///
    /// Returns false when the region is fully clipped.
    pub fn register_hit_region_at(
        &mut self,
        id: ElementId,
        bounds: Bounds,
        z_index: impl Into<ZIndex>,
    ) -> bool {
        let z_index = z_index.into();
        let visible_bounds = match self.visible_bounds_for(bounds) {
            Some(bounds) => bounds,
            None => return false,
        };

        let region = HitRegion {
            id,
            bounds,
            visible_bounds,
            z_index,
            order: self.next_hit_order,
        };
        self.next_hit_order += 1;

        if let Some(layer) = self.active_layer_mut() {
            layer.hit_regions.push(region);
        }

        true
    }

    /// Hit test a point, returning the topmost registered element.
    pub fn hit_test(&self, point: Point) -> Option<ElementId> {
        self.layers
            .iter()
            .flat_map(|layer| layer.hit_regions.iter())
            .filter(|region| region.contains(point))
            .max_by_key(|region| (region.z_index, region.order))
            .map(|region| region.id)
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

    fn active_layer_mut(&mut self) -> Option<&mut Layer> {
        let id = self.current_render_layer();
        self.layers.get_mut(id.index())
    }

    fn visible_bounds_for(&self, bounds: Bounds) -> Option<Bounds> {
        let mut visible = bounds;

        if let Some(render_clip) = self
            .layer(self.current_render_layer())
            .and_then(|layer| layer.clip_bounds())
        {
            visible = visible.intersection(&render_clip)?;
        }

        for clip in &self.layer_stack {
            visible = visible.intersection(clip)?;
        }

        Some(visible)
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::color::Rgba;
    use crate::core::geometry::Edges;
    use crate::core::style::Corners;

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds {
        Bounds::from_xywh(x, y, width, height)
    }

    fn point(x: f32, y: f32) -> Point {
        Point::new(x, y)
    }

    fn quad(x: f32) -> Primitive {
        Primitive::Quad {
            bounds: bounds(x, 0.0, 10.0, 10.0),
            background: Rgba::new(1.0, 1.0, 1.0, 1.0),
            border_color: Rgba::new(0.0, 0.0, 0.0, 0.0),
            border_widths: Edges::ZERO,
            corner_radii: Corners::ZERO,
        }
    }

    #[test]
    fn insert_preserves_flat_primitive_order() {
        let mut scene = Scene::new();

        scene.insert(quad(1.0));
        scene.insert(quad(2.0));

        assert_eq!(scene.primitives().len(), 2);
        match &scene.primitives()[0] {
            Primitive::Quad { bounds, .. } => assert_eq!(bounds.x(), 1.0),
            _ => panic!("expected quad"),
        }
        match &scene.primitives()[1] {
            Primitive::Quad { bounds, .. } => assert_eq!(bounds.x(), 2.0),
            _ => panic!("expected quad"),
        }
    }

    #[test]
    fn push_layer_emits_clip_primitives_and_tracks_current_clip() {
        let mut scene = Scene::new();
        let clip = bounds(1.0, 2.0, 30.0, 40.0);

        scene.push_layer(clip);

        assert_eq!(scene.current_clip(), Some(clip));
        assert_eq!(scene.len(), 1);
        assert!(matches!(scene.primitives()[0], Primitive::PushClip { .. }));

        scene.pop_layer();

        assert_eq!(scene.current_clip(), None);
        assert_eq!(scene.len(), 2);
        assert!(matches!(scene.primitives()[1], Primitive::PopClip));
    }

    #[test]
    fn clear_resets_layers_clips_and_primitives() {
        let mut scene = Scene::new();
        scene.push_layer(bounds(0.0, 0.0, 20.0, 20.0));
        scene.push_render_layer(ZIndex::new(5), None);
        scene.insert(quad(1.0));
        scene.register_hit_region_at(ElementId::from(1), bounds(0.0, 0.0, 5.0, 5.0), 5);

        scene.clear();

        assert!(scene.is_empty());
        assert_eq!(scene.current_clip(), None);
        assert_eq!(scene.layers().len(), 1);
        assert_eq!(scene.current_render_layer(), LayerId(0));
        assert_eq!(scene.hit_test(point(1.0, 1.0)), None);
    }

    #[test]
    fn layers_sort_by_z_index_without_reordering_flat_primitives() {
        let mut scene = Scene::new();
        let low = scene.push_render_layer(ZIndex::new(-1), None);
        scene.insert(quad(1.0));
        scene.pop_render_layer();

        let high = scene.push_render_layer(ZIndex::new(10), None);
        scene.insert(quad(2.0));
        scene.pop_render_layer();

        let ordered: Vec<LayerId> = scene
            .layers_in_z_order()
            .iter()
            .map(|layer| layer.id())
            .collect();
        assert_eq!(ordered, vec![low, LayerId(0), high]);
        assert_eq!(scene.primitives().len(), 2);
    }

    #[test]
    fn hit_test_returns_topmost_visible_region() {
        let mut scene = Scene::new();
        let bottom = ElementId::from(1);
        let top = ElementId::from(2);

        assert!(scene.register_hit_region_at(bottom, bounds(0.0, 0.0, 20.0, 20.0), 0));
        assert!(scene.register_hit_region_at(top, bounds(0.0, 0.0, 20.0, 20.0), 10));

        assert_eq!(scene.hit_test(point(5.0, 5.0)), Some(top));
    }

    #[test]
    fn hit_regions_are_clipped_by_current_clip() {
        let mut scene = Scene::new();
        let id = ElementId::from(1);

        scene.push_layer(bounds(0.0, 0.0, 10.0, 10.0));
        assert!(scene.register_hit_region_at(id, bounds(5.0, 5.0, 20.0, 20.0), 0));

        assert_eq!(scene.hit_test(point(6.0, 6.0)), Some(id));
        assert_eq!(scene.hit_test(point(16.0, 16.0)), None);
    }

    #[test]
    fn hit_regions_are_clipped_by_all_active_clips() {
        let mut scene = Scene::new();
        let id = ElementId::from(1);

        scene.push_layer(bounds(0.0, 0.0, 20.0, 20.0));
        scene.push_layer(bounds(5.0, 5.0, 5.0, 5.0));

        assert!(scene.register_hit_region_at(id, bounds(0.0, 0.0, 20.0, 20.0), 0));
        assert_eq!(scene.hit_test(point(6.0, 6.0)), Some(id));
        assert_eq!(scene.hit_test(point(2.0, 2.0)), None);
        assert_eq!(scene.hit_test(point(12.0, 12.0)), None);
    }

    #[test]
    fn hit_region_can_inherit_active_layer_z_index() {
        let mut scene = Scene::new();
        let root = ElementId::from(1);
        let layered = ElementId::from(2);

        assert!(scene.register_hit_region(root, bounds(0.0, 0.0, 20.0, 20.0)));
        scene.push_render_layer(ZIndex::new(5), None);
        assert!(scene.register_hit_region(layered, bounds(0.0, 0.0, 20.0, 20.0)));

        assert_eq!(scene.hit_test(point(1.0, 1.0)), Some(layered));
    }

    #[test]
    fn fully_clipped_hit_region_is_not_registered() {
        let mut scene = Scene::new();

        scene.push_layer(bounds(0.0, 0.0, 10.0, 10.0));

        assert!(!scene.register_hit_region_at(ElementId::from(1), bounds(20.0, 20.0, 5.0, 5.0), 0));
        assert_eq!(scene.hit_test(point(21.0, 21.0)), None);
    }
}
