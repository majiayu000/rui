#![cfg(target_os = "macos")]

use rui::core::color::Rgba;
use rui::core::geometry::{Bounds, Edges, Point, Size};
use rui::core::style::Corners;
use rui::elements::text::TextAlign;
use rui::renderer::{PathVertex, Picture, Primitive, RendererError, Scene};
use rui::testing::{FrameCaptureBackend, MetalFrameCaptureBackend};

const VIEWPORT: Size = Size {
    width: 240.0,
    height: 180.0,
};

fn backend() -> Option<MetalFrameCaptureBackend> {
    match MetalFrameCaptureBackend::new() {
        Ok(backend) => Some(backend),
        Err(RendererError::BackendUnavailable(_)) => None,
        Err(err) => panic!("metal capture backend should initialize: {err}"),
    }
}

fn quad(bounds: Bounds, color: Rgba, radius: f32) -> Primitive {
    Primitive::Quad {
        bounds,
        background: color,
        border_color: Rgba::new(1.0, 1.0, 1.0, 1.0),
        border_widths: Edges::all(if radius > 0.0 { 1.0 } else { 0.0 }),
        corner_radii: Corners::all(radius),
    }
}

/// Quads, a large fill and stroke (tiles, not atlas sprites), text and a clip.
fn content() -> Vec<Primitive> {
    let star: Vec<PathVertex> = (0..10)
        .map(|point| {
            let angle = point as f32 * std::f32::consts::PI / 5.0;
            let radius = if point % 2 == 0 { 80.0 } else { 35.0 };
            PathVertex::new(110.0 + angle.sin() * radius, 90.0 - angle.cos() * radius)
        })
        .collect();
    let chart: Vec<PathVertex> = (0..40)
        .map(|step| PathVertex::new(step as f32 * 6.0, 120.0 + (step as f32 * 0.5).sin() * 40.0))
        .collect();
    let mut primitives: Vec<Primitive> = (0..12)
        .map(|cell| {
            let bounds = Bounds::from_xywh(
                (cell % 4) as f32 * 30.0,
                (cell / 4) as f32 * 20.0,
                28.0,
                18.0,
            );
            quad(
                bounds,
                Rgba::new(0.2, 0.1 * cell as f32 % 1.0, 0.6, 1.0),
                (cell % 3) as f32 * 3.0,
            )
        })
        .collect();
    primitives.push(Primitive::Path {
        vertices: star.into(),
        offset: Point::ZERO,
        color: Rgba::new(0.9, 0.5, 0.1, 0.7),
        stroke_width: None,
    });
    primitives.push(Primitive::PushClip {
        bounds: Bounds::from_xywh(20.0, 60.0, 160.0, 80.0),
        corner_radii: Corners::ZERO,
    });
    primitives.push(Primitive::Path {
        vertices: chart.into(),
        offset: Point::ZERO,
        color: Rgba::new(0.2, 0.9, 0.5, 1.0),
        stroke_width: Some(3.0),
    });
    primitives.push(Primitive::Text {
        bounds: Bounds::from_xywh(30.0, 70.0, 150.0, 20.0),
        content: "kept picture".into(),
        color: Rgba::WHITE,
        font_size: 14.0,
        font_weight: 400,
        font_family: None,
        line_height: 18.0,
        align: TextAlign::Left,
    });
    primitives.push(Primitive::PopClip);
    primitives.push(quad(
        Bounds::from_xywh(150.0, 10.0, 60.0, 40.0),
        Rgba::new(0.1, 0.4, 0.9, 0.8),
        8.0,
    ));
    primitives
}

fn scene(primitives: impl IntoIterator<Item = Primitive>) -> Scene {
    let mut scene = Scene::new();
    scene.insert(quad(
        Bounds::from_xywh(0.0, 0.0, VIEWPORT.width, VIEWPORT.height),
        Rgba::new(0.05, 0.05, 0.08, 1.0),
        0.0,
    ));
    for primitive in primitives {
        scene.insert(primitive);
    }
    scene.insert(quad(
        Bounds::from_xywh(100.0, 100.0, 50.0, 50.0),
        Rgba::new(0.8, 0.2, 0.3, 0.5),
        0.0,
    ));
    scene.finish();
    scene
}

#[test]
fn a_picture_drawn_again_from_its_kept_plan_matches_its_primitives() {
    let (Some(mut kept), Some(mut flat)) = (backend(), backend()) else {
        return;
    };
    let offset = Point::new(7.0, 5.0);
    let picture = Picture::new(content());
    let pictured = scene([Primitive::Picture { picture, offset }]);
    let moved = content().into_iter().map(|mut primitive| {
        primitive.translate(offset.x, offset.y);
        primitive
    });
    let expected = flat
        .capture_frame(&scene(moved), VIEWPORT)
        .expect("capture");
    // The first frame plans the picture, the second keeps the plan, and the
    // later ones copy it.
    for frame in 0..4 {
        let pixels = kept
            .capture_frame(&pictured, VIEWPORT)
            .expect("capture")
            .pixels;
        assert!(pixels == expected.pixels, "frame {frame} differs");
    }
}

#[test]
#[should_panic(expected = "leaves a clip pushed")]
fn a_picture_must_balance_its_clips() {
    Picture::new(vec![Primitive::PushClip {
        bounds: Bounds::from_xywh(0.0, 0.0, 1.0, 1.0),
        corner_radii: Corners::ZERO,
    }]);
}
