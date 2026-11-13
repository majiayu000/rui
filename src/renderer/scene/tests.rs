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
                if scene.register_hit_region_at(ElementId::from(rng.below(64)), region, z_index) {
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
