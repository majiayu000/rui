//! Scene graph - collects primitives for rendering

use crate::core::ElementId;
use crate::core::geometry::{Bounds, Point};
use crate::renderer::primitives::Primitive;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

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

/// Size of a hit-index cell, in logical pixels. Wider than tall because rows, lines
/// and buttons are; powers of two keep the division that maps a point to its cell
/// exact.
const HIT_CELL_WIDTH: f32 = 256.0;
const HIT_CELL_HEIGHT: f32 = 64.0;

/// Regions spanning more cells than this are scanned on every query instead.
const HIT_MAX_CELLS_PER_REGION: i64 = 256;

/// Multiplicative hash for cell coordinates. SipHash made registering a long list
/// several times slower than painting it.
#[derive(Debug, Default)]
struct CellHasher(u64);

impl Hasher for CellHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.write_u32(u32::from(*byte));
        }
    }

    fn write_u32(&mut self, value: u32) {
        self.0 = (self.0.rotate_left(5) ^ u64::from(value)).wrapping_mul(0x517c_c1b7_2722_0a95);
    }

    fn write_i32(&mut self, value: i32) {
        self.write_u32(value as u32);
    }
}

/// Sparse uniform grid over hit regions, keyed by registration order.
#[derive(Debug, Default)]
struct HitIndex {
    /// `(layer index, slot in layer)` for each region, indexed by its order.
    locations: Vec<(usize, usize)>,
    cells: HashMap<(i32, i32), Vec<usize>, BuildHasherDefault<CellHasher>>,
    /// Regions too large (or not finite) for the grid.
    oversized: Vec<usize>,
}

impl HitIndex {
    fn clear(&mut self) {
        self.locations.clear();
        // Keep last frame's cells and their buffers; drop cells that frame left empty.
        self.cells.retain(|_, orders| {
            let used = !orders.is_empty();
            orders.clear();
            used
        });
        self.oversized.clear();
    }

    /// The cell holding `(x, y)`, or `None` outside the grid's `i32` range.
    fn cell_of(x: f32, y: f32) -> Option<(i32, i32)> {
        let cell = |value: f32, size: f32| {
            let cell = (value / size).floor();
            (cell >= i32::MIN as f32 && cell < i32::MAX as f32).then_some(cell as i32)
        };
        Some((cell(x, HIT_CELL_WIDTH)?, cell(y, HIT_CELL_HEIGHT)?))
    }

    fn insert(&mut self, order: usize, location: (usize, usize), bounds: Bounds) {
        debug_assert_eq!(order, self.locations.len());
        self.locations.push(location);

        // Same edges `Bounds::contains` tests against, so every contained point maps
        // to a cell this region is filed under.
        let x1 = bounds.origin.x;
        let y1 = bounds.origin.y;
        let x2 = x1 + bounds.size.width;
        let y2 = y1 + bounds.size.height;
        let (Some((cx1, cy1)), Some((cx2, cy2))) = (Self::cell_of(x1, y1), Self::cell_of(x2, y2))
        else {
            self.oversized.push(order);
            return;
        };
        let span = |from: i32, to: i32| (i64::from(to) - i64::from(from) + 1).max(0);
        if span(cx1, cx2) * span(cy1, cy2) > HIT_MAX_CELLS_PER_REGION {
            self.oversized.push(order);
            return;
        }
        for cy in cy1..=cy2 {
            for cx in cx1..=cx2 {
                self.cells.entry((cx, cy)).or_default().push(order);
            }
        }
    }

    /// Orders of regions that may contain `point`; a superset of the true hits.
    fn candidates(&self, point: Point) -> impl Iterator<Item = usize> + '_ {
        Self::cell_of(point.x, point.y)
            .and_then(|cell| self.cells.get(&cell))
            .into_iter()
            .flatten()
            .chain(self.oversized.iter())
            .copied()
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

    accessibility_regions: HashMap<ElementId, Bounds>,

    next_hit_order: usize,

    hit_index: HitIndex,
}

impl Scene {
    pub fn new() -> Self {
        let root_id = LayerId(0);
        Self {
            primitives: Vec::new(),
            layer_stack: SmallVec::new(),
            layers: vec![Layer::new(root_id, 0, ZIndex::ROOT, None)],
            active_layers: smallvec::smallvec![root_id],
            accessibility_regions: HashMap::new(),
            next_hit_order: 0,
            hit_index: HitIndex::default(),
        }
    }

    /// Clear the scene for a new frame
    pub fn clear(&mut self) {
        self.primitives.clear();
        self.layer_stack.clear();
        self.layers.clear();
        self.active_layers.clear();
        self.accessibility_regions.clear();
        let root_id = LayerId(0);
        self.layers.push(Layer::new(root_id, 0, ZIndex::ROOT, None));
        self.active_layers.push(root_id);
        self.next_hit_order = 0;
        self.hit_index.clear();
    }

    /// Insert a primitive into the scene
    pub fn insert(&mut self, primitive: Primitive) {
        let primitive_index = self.primitives.len();
        self.primitives.push(primitive);
        if let Some(layer) = self.active_layer_mut() {
            layer.primitive_indices.push(primitive_index);
        }
    }

    pub(crate) fn accessibility_regions(&self) -> &HashMap<ElementId, Bounds> {
        &self.accessibility_regions
    }

    pub(crate) fn register_accessibility_region(&mut self, id: ElementId, bounds: Bounds) {
        self.accessibility_regions.insert(id, bounds);
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
        self.register_accessibility_region(id, bounds);
        let visible_bounds = match self.visible_bounds_for(bounds) {
            Some(bounds) => bounds,
            None => return false,
        };

        let layer_index = self.current_render_layer().index();
        let Some(layer) = self.layers.get_mut(layer_index) else {
            return false;
        };
        let order = self.next_hit_order;
        self.next_hit_order += 1;
        layer.hit_regions.push(HitRegion {
            id,
            bounds,
            visible_bounds,
            z_index,
            order,
        });
        self.hit_index.insert(
            order,
            (layer_index, layer.hit_regions.len() - 1),
            visible_bounds,
        );

        true
    }

    /// Hit test a point, returning the topmost registered element.
    ///
    /// Only regions filed under the point's grid cell are tested, so the cost does
    /// not grow with the number of regions elsewhere in the scene.
    pub fn hit_test(&self, point: Point) -> Option<ElementId> {
        self.hit_index
            .candidates(point)
            .map(|order| {
                let (layer, slot) = self.hit_index.locations[order];
                &self.layers[layer].hit_regions[slot]
            })
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
    fn hit_regions_also_supply_accessibility_geometry() {
        let mut scene = Scene::new();
        let id = ElementId::from(22);
        let region = bounds(5.0, 6.0, 20.0, 30.0);

        assert!(scene.register_hit_region(id, region));
        assert_eq!(scene.accessibility_regions().get(&id), Some(&region));
        scene.clear();
        assert!(scene.accessibility_regions().is_empty());
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
        let id = ElementId::from(1);
        let region = bounds(20.0, 20.0, 5.0, 5.0);

        scene.push_layer(bounds(0.0, 0.0, 10.0, 10.0));

        assert!(!scene.register_hit_region_at(id, region, 0));
        assert_eq!(scene.hit_test(point(21.0, 21.0)), None);
        assert_eq!(scene.accessibility_regions().get(&id), Some(&region));
    }

    /// The linear scan `hit_test` answered with before the grid index.
    fn linear_hit_test(scene: &Scene, point: Point) -> Option<ElementId> {
        scene
            .layers()
            .iter()
            .flat_map(|layer| layer.hit_regions())
            .filter(|region| region.contains(point))
            .max_by_key(|region| (region.z_index, region.order))
            .map(|region| region.id)
    }

    struct XorShift(u64);

    impl XorShift {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn below(&mut self, bound: u64) -> u64 {
            self.next() % bound
        }

        /// A coordinate on a coarse lattice, so edges often coincide with each
        /// other, with query points, and with grid cell boundaries.
        fn coord(&mut self, span: u64) -> f32 {
            (self.below(span) as f32 - (span / 4) as f32) * 8.0
        }

        fn bounds(&mut self) -> Bounds {
            let (x, y) = (self.coord(200), self.coord(200));
            match self.below(10) {
                0 => bounds(x, y, 0.0, 0.0),
                1 => bounds(
                    x,
                    y,
                    self.coord(2_000).abs() * 4.0,
                    self.coord(2_000).abs() * 4.0,
                ),
                _ => bounds(x, y, self.coord(40).abs(), self.coord(40).abs()),
            }
        }
    }

    fn random_scene(rng: &mut XorShift, scene: &mut Scene) -> Vec<Bounds> {
        let mut registered = Vec::new();
        for _ in 0..rng.below(400) {
            match rng.below(12) {
                0 => {
                    let clip = (rng.below(2) == 0).then(|| rng.bounds());
                    scene.push_render_layer(rng.below(5) as i32 - 2, clip);
                }
                1 => {
                    scene.pop_render_layer();
                }
                2 => scene.push_layer(rng.bounds()),
                3 if scene.current_clip().is_some() => scene.pop_layer(),
                4 => {
                    let region = rng.bounds();
                    if scene.register_hit_region(ElementId::from(rng.below(64)), region) {
                        registered.push(region);
                    }
                }
                _ => {
                    let region = rng.bounds();
                    let z_index = rng.below(5) as i32 - 2;
                    if scene.register_hit_region_at(ElementId::from(rng.below(64)), region, z_index)
                    {
                        registered.push(region);
                    }
                }
            }
        }
        registered
    }

    #[test]
    fn indexed_hit_test_matches_linear_scan_on_random_scenes() {
        let mut rng = XorShift(0x9e37_79b9_7f4a_7c15);
        let mut scene = Scene::new();
        let mut hits = 0;
        for _ in 0..300 {
            scene.clear();
            let registered = random_scene(&mut rng, &mut scene);
            let mut points: Vec<Point> = (0..64)
                .map(|_| point(rng.coord(200), rng.coord(200)))
                .collect();
            for region in &registered {
                let (x, y) = (region.origin.x, region.origin.y);
                let (w, h) = (region.size.width, region.size.height);
                points.extend([point(x, y), point(x + w, y + h), point(x + w / 2.0, y)]);
            }
            for point in points {
                let expected = linear_hit_test(&scene, point);
                hits += expected.is_some() as usize;
                assert_eq!(scene.hit_test(point), expected, "at {point:?}");
            }
        }
        assert!(
            hits > 1_000,
            "random scenes should produce real hits, got {hits}"
        );
    }

    #[test]
    fn indexed_hit_test_handles_empty_and_non_finite_input() {
        let mut scene = Scene::new();
        assert_eq!(scene.hit_test(point(0.0, 0.0)), None);
        assert_eq!(scene.hit_test(point(f32::NAN, 1.0)), None);

        let huge = ElementId::from(1);
        let infinite = ElementId::from(2);
        assert!(scene.register_hit_region_at(huge, bounds(-1e30, -1e30, 2e30, 2e30), 0));
        assert!(scene.register_hit_region_at(infinite, bounds(0.0, 0.0, f32::INFINITY, 10.0), 1));
        for point in [
            point(5.0, 5.0),
            point(-1e29, 1e29),
            point(1e20, 5.0),
            point(f32::INFINITY, 5.0),
            point(f32::NAN, f32::NAN),
        ] {
            assert_eq!(
                scene.hit_test(point),
                linear_hit_test(&scene, point),
                "at {point:?}"
            );
        }
        assert_eq!(scene.hit_test(point(5.0, 5.0)), Some(infinite));
    }
}
