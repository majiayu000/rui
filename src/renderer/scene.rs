//! Scene graph - collects primitives for rendering

use crate::core::ElementId;
use crate::core::geometry::{Bounds, Point, Size};
use crate::renderer::primitives::{Primitive, Reach};
use smallvec::SmallVec;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};

/// Source of [`Scene::frame`] ids, unique across scenes so a range recorded in
/// one scene is never replayed from another.
static NEXT_SCENE_FRAME: AtomicU64 = AtomicU64::new(1);

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

/// Queries in one frame after which `hit_test` builds the grid. Building costs
/// about as much as 30 linear scans (`benches/hit_test.rs`), so building once that
/// many have run keeps a frame within twice the cost of the better choice, and a
/// frame with a handful of queries never builds.
const HIT_INDEX_MIN_QUERIES: usize = 32;

/// Scenes with fewer regions always scan: a grid lookup costs about as much as
/// scanning a few dozen regions.
const HIT_INDEX_MIN_REGIONS: usize = 64;

/// `(layer index, slot in layer)` of a hit region.
type RegionSlot = (usize, usize);

/// Sparse uniform grid over hit regions, built on demand from the layers.
#[derive(Debug, Default)]
struct HitIndex {
    built: bool,
    /// The regions overlapping each cell.
    cells: HashMap<(i32, i32), Vec<RegionSlot>, BuildHasherDefault<CellHasher>>,
    /// Regions too large (or not finite) for the grid.
    oversized: Vec<RegionSlot>,
}

impl HitIndex {
    fn clear(&mut self) {
        self.built = false;
        // Keep the last build's cells and their buffers; drop cells it left empty.
        self.cells.retain(|_, regions| {
            let used = !regions.is_empty();
            regions.clear();
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

    fn build(&mut self, layers: &[Layer]) {
        // Empty the cells in place: their buffers are what makes a rebuild cheap.
        self.cells.values_mut().for_each(Vec::clear);
        self.oversized.clear();
        for (layer_index, layer) in layers.iter().enumerate() {
            for (slot, region) in layer.hit_regions.iter().enumerate() {
                self.insert((layer_index, slot), region.visible_bounds);
            }
        }
        self.built = true;
    }

    fn insert(&mut self, location: RegionSlot, bounds: Bounds) {
        // Same edges `Bounds::contains` tests against, so every contained point maps
        // to a cell this region is filed under.
        let x1 = bounds.origin.x;
        let y1 = bounds.origin.y;
        let x2 = x1 + bounds.size.width;
        let y2 = y1 + bounds.size.height;
        let (Some((cx1, cy1)), Some((cx2, cy2))) = (Self::cell_of(x1, y1), Self::cell_of(x2, y2))
        else {
            self.oversized.push(location);
            return;
        };
        let span = |from: i32, to: i32| (i64::from(to) - i64::from(from) + 1).max(0);
        if span(cx1, cx2) * span(cy1, cy2) > HIT_MAX_CELLS_PER_REGION {
            self.oversized.push(location);
            return;
        }
        for cy in cy1..=cy2 {
            for cx in cx1..=cx2 {
                self.cells.entry((cx, cy)).or_default().push(location);
            }
        }
    }

    /// Regions that may contain `point`; a superset of the true hits.
    fn candidates(&self, point: Point) -> impl Iterator<Item = RegionSlot> + '_ {
        Self::cell_of(point.x, point.y)
            .and_then(|cell| self.cells.get(&cell))
            .into_iter()
            .flatten()
            .chain(self.oversized.iter())
            .copied()
    }
}

/// The region containing `point` with the highest `(z_index, order)`.
fn topmost<'a>(regions: impl Iterator<Item = &'a HitRegion>, point: Point) -> Option<ElementId> {
    regions
        .filter(|region| region.contains(point))
        .max_by_key(|region| (region.z_index, region.order))
        .map(|region| region.id)
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

    /// Primitives of the frame before, which [`Scene::replay`] moves ranges out of.
    previous_primitives: Vec<Primitive>,

    /// What memoizable subtrees painted this frame and the one before, kept
    /// here rather than in each element so elements stay small and painting
    /// allocates nothing per element.
    records: Vec<PaintRecord>,
    previous_records: Vec<PaintRecord>,

    /// Id of the frame being painted, and of the one in `previous_primitives`.
    frame: u64,
    previous_frame: u64,

    /// Primitives this frame took from the previous one by [`Scene::replay`].
    replayed: usize,

    /// Ranges of this frame replayed in place and in order, which drew exactly
    /// what they drew in the previous frame.
    unchanged: Vec<Range<usize>>,
    /// End of the last range replayed from the previous frame, to notice a
    /// replay that changed order.
    replayed_from: usize,
    /// Where moved or reordered replays drew, before and after.
    moved: Reach,

    /// Layer stack for nested clipping
    layer_stack: SmallVec<[Bounds; 8]>,

    /// Render layers for future z-order and event targeting.
    layers: Vec<Layer>,

    /// Active render-layer stack.
    active_layers: SmallVec<[LayerId; 8]>,

    accessibility_regions: HashMap<ElementId, Bounds>,

    next_hit_order: usize,

    /// Hit tests since the last `clear`, which decide when to build `hit_index`.
    hit_queries: Cell<usize>,

    hit_index: RefCell<HitIndex>,
}

impl Scene {
    pub fn new() -> Self {
        let root_id = LayerId(0);
        Self {
            primitives: Vec::new(),
            previous_primitives: Vec::new(),
            records: Vec::new(),
            previous_records: Vec::new(),
            frame: NEXT_SCENE_FRAME.fetch_add(1, Ordering::Relaxed),
            previous_frame: 0,
            replayed: 0,
            unchanged: Vec::new(),
            replayed_from: 0,
            moved: Reach::Nothing,
            layer_stack: SmallVec::new(),
            layers: vec![Layer::new(root_id, 0, ZIndex::ROOT, None)],
            active_layers: smallvec::smallvec![root_id],
            accessibility_regions: HashMap::new(),
            next_hit_order: 0,
            hit_queries: Cell::new(0),
            hit_index: RefCell::default(),
        }
    }

    /// Clear the scene for a new frame
    pub fn clear(&mut self) {
        // The frame just painted becomes the one later paints replay from.
        std::mem::swap(&mut self.primitives, &mut self.previous_primitives);
        self.primitives.clear();
        std::mem::swap(&mut self.records, &mut self.previous_records);
        self.records.clear();
        self.previous_frame = self.frame;
        self.frame = NEXT_SCENE_FRAME.fetch_add(1, Ordering::Relaxed);
        self.replayed = 0;
        self.unchanged.clear();
        self.replayed_from = 0;
        self.moved = Reach::Nothing;
        self.layer_stack.clear();
        self.layers.clear();
        self.active_layers.clear();
        self.accessibility_regions.clear();
        let root_id = LayerId(0);
        self.layers.push(Layer::new(root_id, 0, ZIndex::ROOT, None));
        self.active_layers.push(root_id);
        self.next_hit_order = 0;
        self.hit_queries.set(0);
        self.hit_index.get_mut().clear();
    }

    /// Insert a primitive into the scene
    pub fn insert(&mut self, primitive: Primitive) {
        let primitive_index = self.primitives.len();
        self.primitives.push(primitive);
        if let Some(layer) = self.active_layer_mut() {
            layer.primitive_indices.push(primitive_index);
        }
    }

    /// Keep `record` for the next frame and return its index in this one.
    pub(crate) fn record(&mut self, record: PaintRecord) -> u32 {
        self.records.push(record);
        (self.records.len() - 1) as u32
    }

    /// Record `index` of this frame, or of the previous one.
    pub(crate) fn paint_record(&self, frame: u64, index: u32) -> Option<PaintRecord> {
        let records = if frame == self.frame {
            &self.records
        } else if frame == self.previous_frame {
            &self.previous_records
        } else {
            return None;
        };
        records.get(index as usize).copied()
    }

    /// Id of the frame being painted. Ranges recorded now can be replayed in
    /// the next frame, while [`Self::previous_frame`] returns this id.
    pub fn frame(&self) -> u64 {
        self.frame
    }

    /// Id of the frame [`Self::replay`] reads from.
    pub fn previous_frame(&self) -> u64 {
        self.previous_frame
    }

    /// Paint `range` of the previous frame's primitives again, moved by
    /// `(dx, dy)`, and return where they landed in this frame.
    ///
    /// The primitives are moved out of the previous frame, so each range can be
    /// replayed once. Fails, painting nothing, when the range is not in the
    /// previous frame or moving it would not give exactly the coordinates a
    /// repaint computes (see [`Primitive::translates_exactly`]).
    pub fn replay(&mut self, range: Range<usize>, dx: f32, dy: f32) -> Option<Range<usize>> {
        if range.end > self.previous_primitives.len() || range.start > range.end {
            return None;
        }
        let moved = dx != 0.0 || dy != 0.0;
        if moved
            && !self.previous_primitives[range.clone()]
                .iter()
                .all(|primitive| primitive.translates_exactly(dx, dy))
        {
            return None;
        }
        let start = self.primitives.len();
        // Drawn over something painted in between, or moved: its old and new
        // places change. In place and in order, nothing it covers does.
        let in_place = !moved && range.start >= self.replayed_from;
        self.replayed_from = range.end;
        for index in range {
            if !in_place {
                self.moved = union(self.moved, damage_reach(&self.previous_primitives[index]));
            }
            let mut primitive =
                std::mem::replace(&mut self.previous_primitives[index], Primitive::PopClip);
            if moved {
                primitive.translate(dx, dy);
            }
            self.insert(primitive);
        }
        self.replayed += self.primitives.len() - start;
        let end = self.primitives.len();
        if in_place {
            match self.unchanged.last_mut() {
                Some(last) if last.end == start => last.end = end,
                _ => self.unchanged.push(start..end),
            }
        } else {
            for primitive in &self.primitives[start..end] {
                self.moved = union(self.moved, damage_reach(primitive));
            }
        }
        Some(start..end)
    }

    /// Where this frame can differ from the previous one: everything painted
    /// rather than replayed in place, everything of the previous frame that was
    /// not replayed, and both places of what moved.
    ///
    /// Only meaningful for a target that shows the previous frame
    /// ([`Self::previous_frame`]); anything else needs a full redraw.
    ///
    /// Only what can be seen in `visible` counts, so content that left the
    /// view or was not painted for being out of it does not.
    pub fn damage(&self, visible: Bounds) -> Reach {
        let seen = |reach: Reach| match reach {
            Reach::Within(bounds) => bounds
                .intersection(&visible)
                .map_or(Reach::Nothing, Reach::Within),
            other => other,
        };
        let mut damage = seen(self.moved);
        let mut unchanged = self.unchanged.iter().peekable();
        let mut index = 0;
        while index < self.primitives.len() {
            if let Some(range) = unchanged.peek()
                && range.start == index
            {
                index = range.end;
                unchanged.next();
                continue;
            }
            damage = union(damage, seen(damage_reach(&self.primitives[index])));
            if damage == Reach::Unbounded {
                return damage;
            }
            index += 1;
        }
        // Replayed primitives were moved out, leaving clip pops that draw nothing.
        for primitive in &self.previous_primitives {
            damage = union(damage, seen(damage_reach(primitive)));
            if damage == Reach::Unbounded {
                return damage;
            }
        }
        damage
    }

    /// Bounds of everything the primitives in `range` can draw on; covering
    /// everything when that has no bound.
    pub(crate) fn reach_of(&self, range: Range<usize>) -> Bounds {
        let mut reach: Option<Bounds> = None;
        for primitive in &self.primitives[range] {
            match primitive.reach() {
                Reach::Nothing => {}
                Reach::Within(bounds) => {
                    reach = Some(reach.map_or(bounds, |reach| reach.union(&bounds)));
                }
                Reach::Unbounded => {
                    // Finite, so moving it and testing it stay well defined.
                    return Bounds::from_xywh(
                        f32::MIN / 4.0,
                        f32::MIN / 4.0,
                        f32::MAX / 2.0,
                        f32::MAX / 2.0,
                    );
                }
            }
        }
        reach.unwrap_or(Bounds::from_xywh(0.0, 0.0, 0.0, 0.0))
    }

    /// Drop the previous frame's primitives from `len` on, so they are not
    /// damage. For content painted again over the same place every frame.
    pub(crate) fn truncate_previous(&mut self, len: usize) {
        self.previous_primitives.truncate(len);
    }

    /// Primitives this frame replayed from the previous one instead of painting.
    pub fn replayed_len(&self) -> usize {
        self.replayed
    }

    /// Hit and accessibility regions registered so far this frame.
    pub(crate) fn region_count(&self) -> (usize, usize) {
        (self.next_hit_order, self.accessibility_regions.len())
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
        self.hit_index.get_mut().built = false;

        true
    }

    /// Hit test a point, returning the topmost registered element.
    ///
    /// A frame's first queries scan every region. A frame that keeps querying a
    /// large scene builds a grid index once and then tests only the regions filed
    /// under the point's cell.
    pub fn hit_test(&self, point: Point) -> Option<ElementId> {
        let queries = self.hit_queries.get() + 1;
        self.hit_queries.set(queries);
        if queries < HIT_INDEX_MIN_QUERIES || self.next_hit_order < HIT_INDEX_MIN_REGIONS {
            let regions = self
                .layers
                .iter()
                .flat_map(|layer| layer.hit_regions.iter());
            return topmost(regions, point);
        }

        let mut index = self.hit_index.borrow_mut();
        if !index.built {
            index.build(&self.layers);
        }
        let regions = index
            .candidates(point)
            .map(|(layer, slot)| &self.layers[layer].hit_regions[slot]);
        topmost(regions, point)
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

/// What a memoizable subtree painted in a frame, for the next one to replay
/// moved by its change of origin, or to leave out while it cannot be seen.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PaintRecord {
    /// The primitives are in the frame's range `start..end`; false when the
    /// subtree was left out.
    pub(crate) painted: bool,
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) origin: Point,
    /// Unrounded layout, with its origin summed from the root as taffy sums it.
    pub(crate) unrounded_origin: Point,
    pub(crate) unrounded_size: Size,
    /// What the primitives can draw on at `origin`; everything when that has
    /// no bound.
    pub(crate) reach: Bounds,
}

/// What a primitive's appearing or going away can change on screen.
fn damage_reach(primitive: &Primitive) -> Reach {
    match primitive {
        // A clip changes only what is drawn inside it.
        Primitive::PushClip { bounds, .. } => Reach::Within(*bounds),
        Primitive::PopClip => Reach::Nothing,
        primitive => primitive.reach(),
    }
}

fn union(a: Reach, b: Reach) -> Reach {
    match (a, b) {
        (Reach::Unbounded, _) | (_, Reach::Unbounded) => Reach::Unbounded,
        (Reach::Nothing, other) | (other, Reach::Nothing) => other,
        (Reach::Within(a), Reach::Within(b)) => Reach::Within(a.union(&b)),
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

    /// The grid's answer, built regardless of the thresholds `hit_test` applies.
    fn grid_hit_test(scene: &Scene, point: Point) -> Option<ElementId> {
        let mut index = HitIndex::default();
        index.build(scene.layers());
        let regions = index
            .candidates(point)
            .map(|(layer, slot)| &scene.layers[layer].hit_regions[slot]);
        topmost(regions, point)
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
        let (mut hits, mut indexed) = (0, 0);
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
                assert_eq!(grid_hit_test(&scene, point), expected, "grid at {point:?}");
                indexed += scene.hit_index.borrow().built as usize;
            }
        }
        assert!(
            hits > 1_000,
            "random scenes should produce real hits, got {hits}"
        );
        assert!(
            indexed > 1_000,
            "hit_test should answer from the grid in large scenes, got {indexed}"
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
            let expected = linear_hit_test(&scene, point);
            assert_eq!(scene.hit_test(point), expected, "at {point:?}");
            assert_eq!(grid_hit_test(&scene, point), expected, "grid at {point:?}");
        }
        assert_eq!(scene.hit_test(point(5.0, 5.0)), Some(infinite));
    }

    #[test]
    fn hit_region_registered_after_the_grid_is_built_is_hit() {
        let mut scene = Scene::new();
        for row in 0..HIT_INDEX_MIN_REGIONS {
            let y = row as f32 * 10.0;
            assert!(scene.register_hit_region_at(
                ElementId::from(row as u64),
                bounds(0.0, y, 100.0, 10.0),
                0
            ));
        }
        for _ in 0..HIT_INDEX_MIN_QUERIES {
            assert_eq!(scene.hit_test(point(5.0, 5.0)), Some(ElementId::from(0)));
        }
        assert!(scene.hit_index.borrow().built);

        let top = ElementId::from(1_000);
        assert!(scene.register_hit_region_at(top, bounds(0.0, 0.0, 10.0, 10.0), 1));
        assert_eq!(scene.hit_test(point(5.0, 5.0)), Some(top));
    }
}
