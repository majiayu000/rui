use rui::renderer::{Primitive, Scene, ZIndex};
use rui::{Bounds, Corners, Edges, ElementId, Point, Rgba};

fn quad_with_key(key: f32) -> Primitive {
    Primitive::Quad {
        bounds: Bounds::from_xywh(key, key + 1.0, 10.0, 10.0),
        background: Rgba::new(key, 0.0, 0.0, 1.0),
        border_color: Rgba::TRANSPARENT,
        border_widths: Edges::ZERO,
        corner_radii: Corners::ZERO,
    }
}

fn quad_key(primitive: &Primitive) -> f32 {
    match primitive {
        Primitive::Quad { bounds, .. } => bounds.x(),
        other => panic!("expected quad primitive, got {other:?}"),
    }
}

fn primitive_names(scene: &Scene) -> Vec<&'static str> {
    scene
        .primitives()
        .iter()
        .map(|primitive| match primitive {
            Primitive::Quad { .. } => "quad",
            Primitive::PushClip { .. } => "push_clip",
            Primitive::PopClip => "pop_clip",
            Primitive::Shadow { .. } => "shadow",
            Primitive::LinearGradient { .. } => "linear_gradient",
            Primitive::RadialGradient { .. } => "radial_gradient",
            Primitive::Text { .. } => "text",
            Primitive::Image { .. } => "image",
            Primitive::Path { .. } => "path",
        })
        .collect()
}

#[test]
fn scene_foundations_insert_preserves_primitive_order() {
    let mut scene = Scene::new();

    scene.insert(quad_with_key(1.0));
    scene.insert(quad_with_key(2.0));
    scene.insert(quad_with_key(3.0));

    let keys: Vec<f32> = scene.primitives().iter().map(quad_key).collect();
    assert_eq!(keys, vec![1.0, 2.0, 3.0]);
}

#[test]
fn scene_foundations_push_and_pop_emit_clip_primitives() {
    let mut scene = Scene::new();
    let clip = Bounds::from_xywh(10.0, 20.0, 30.0, 40.0);

    scene.push_layer(clip);
    scene.pop_layer();

    assert_eq!(scene.len(), 2);
    match &scene.primitives()[0] {
        Primitive::PushClip {
            bounds,
            corner_radii,
        } => {
            assert_eq!(*bounds, clip);
            assert!(corner_radii.is_zero());
        }
        other => panic!("expected push clip primitive, got {other:?}"),
    }
    assert!(matches!(scene.primitives()[1], Primitive::PopClip));
}

#[test]
fn scene_foundations_current_clip_tracks_stack() {
    let mut scene = Scene::new();
    let outer = Bounds::from_xywh(0.0, 0.0, 100.0, 100.0);
    let inner = Bounds::from_xywh(25.0, 25.0, 40.0, 40.0);

    assert_eq!(scene.current_clip(), None);

    scene.push_layer(outer);
    assert_eq!(scene.current_clip(), Some(outer));

    scene.push_layer(inner);
    assert_eq!(scene.current_clip(), Some(inner));

    scene.pop_layer();
    assert_eq!(scene.current_clip(), Some(outer));

    scene.pop_layer();
    assert_eq!(scene.current_clip(), None);
}

#[test]
fn scene_foundations_len_is_empty_and_clear_reflect_scene_state() {
    let mut scene = Scene::new();
    let clip = Bounds::from_xywh(0.0, 0.0, 20.0, 20.0);
    let elevated = scene.push_render_layer(ZIndex::new(5), None);

    assert!(scene.is_empty());
    assert_eq!(scene.len(), 0);
    assert_eq!(scene.current_render_layer(), elevated);

    scene.insert(quad_with_key(1.0));
    assert!(!scene.is_empty());
    assert_eq!(scene.len(), 1);

    scene.push_layer(clip);
    assert_eq!(scene.len(), 2);
    assert_eq!(scene.current_clip(), Some(clip));

    scene.clear();
    assert!(scene.is_empty());
    assert_eq!(scene.len(), 0);
    assert_eq!(scene.current_clip(), None);
    assert!(scene.layer(elevated).is_none());
    assert_eq!(scene.layers().len(), 1);
    assert!(scene.primitives().is_empty());
}

#[test]
fn scene_foundations_finish_preserves_z_order() {
    let mut scene = Scene::new();

    scene.insert(quad_with_key(10.0));
    scene.insert(quad_with_key(20.0));
    scene.push_layer(Bounds::from_xywh(0.0, 0.0, 100.0, 100.0));
    scene.insert(quad_with_key(30.0));
    scene.pop_layer();
    scene.insert(quad_with_key(40.0));

    scene.finish();

    assert_eq!(
        primitive_names(&scene),
        vec!["quad", "quad", "push_clip", "quad", "pop_clip", "quad"]
    );

    let quad_keys: Vec<f32> = scene
        .primitives()
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Quad { .. } => Some(quad_key(primitive)),
            _ => None,
        })
        .collect();
    assert_eq!(quad_keys, vec![10.0, 20.0, 30.0, 40.0]);
}

#[test]
fn scene_foundations_layers_order_by_z_index_then_creation_order() {
    let mut scene = Scene::new();
    let root = scene.current_render_layer();

    let low = scene.push_render_layer(ZIndex::new(-1), None);
    scene.insert(quad_with_key(1.0));
    assert_eq!(scene.pop_render_layer(), Some(low));

    let high = scene.push_render_layer(ZIndex::new(10), None);
    scene.insert(quad_with_key(2.0));
    assert_eq!(scene.pop_render_layer(), Some(high));

    let high_tie = scene.push_render_layer(ZIndex::new(10), None);
    scene.insert(quad_with_key(3.0));
    assert_eq!(scene.pop_render_layer(), Some(high_tie));

    let ordered_layer_ids: Vec<_> = scene
        .layers_in_z_order()
        .iter()
        .map(|layer| layer.id())
        .collect();
    assert_eq!(ordered_layer_ids, vec![low, root, high, high_tie]);

    assert_eq!(
        scene.layer(high).map(|layer| layer.z_index()),
        Some(ZIndex::new(10))
    );
    assert_eq!(scene.pop_render_layer(), None);
}

#[test]
fn scene_foundations_hit_regions_order_by_z_index_then_registration_order() {
    let mut scene = Scene::new();
    let bottom = ElementId::from(1);
    let top = ElementId::from(2);
    let latest_tie = ElementId::from(3);
    let bounds = Bounds::from_xywh(0.0, 0.0, 20.0, 20.0);
    let point = Point::new(5.0, 5.0);

    assert!(scene.register_hit_region_at(bottom, bounds, ZIndex::new(0)));
    assert!(scene.register_hit_region_at(top, bounds, ZIndex::new(10)));
    assert!(scene.register_hit_region_at(latest_tie, bounds, ZIndex::new(10)));

    assert_eq!(scene.hit_test(point), Some(latest_tie));
}

#[test]
fn scene_foundations_hit_region_clipping_uses_current_clip_intersections() {
    let mut scene = Scene::new();
    let outer = Bounds::from_xywh(0.0, 0.0, 100.0, 100.0);
    let inner = Bounds::from_xywh(25.0, 25.0, 40.0, 40.0);
    let clipped_by_outer = ElementId::from(1);
    let clipped_by_inner = ElementId::from(2);
    let fully_clipped = ElementId::from(3);
    let after_inner_pop = ElementId::from(4);
    let after_all_pops = ElementId::from(5);

    scene.push_layer(outer);
    assert!(scene.register_hit_region_at(
        clipped_by_outer,
        Bounds::from_xywh(80.0, 80.0, 40.0, 40.0),
        0,
    ));
    assert_eq!(
        scene.hit_test(Point::new(90.0, 90.0)),
        Some(clipped_by_outer)
    );
    assert_eq!(scene.hit_test(Point::new(110.0, 110.0)), None);

    scene.push_layer(inner);
    assert!(scene.register_hit_region_at(
        clipped_by_inner,
        Bounds::from_xywh(10.0, 10.0, 80.0, 80.0),
        10,
    ));
    assert!(!scene.register_hit_region_at(
        fully_clipped,
        Bounds::from_xywh(80.0, 80.0, 5.0, 5.0),
        20,
    ));
    assert_eq!(
        scene.hit_test(Point::new(30.0, 30.0)),
        Some(clipped_by_inner)
    );
    assert_eq!(scene.hit_test(Point::new(70.0, 70.0)), None);

    scene.pop_layer();
    assert!(scene.register_hit_region_at(
        after_inner_pop,
        Bounds::from_xywh(5.0, 5.0, 5.0, 5.0),
        5,
    ));
    assert_eq!(scene.hit_test(Point::new(6.0, 6.0)), Some(after_inner_pop));

    scene.pop_layer();
    assert!(scene.register_hit_region_at(
        after_all_pops,
        Bounds::from_xywh(150.0, 150.0, 10.0, 10.0),
        0,
    ));
    assert_eq!(
        scene.hit_test(Point::new(155.0, 155.0)),
        Some(after_all_pops)
    );
}
