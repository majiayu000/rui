#![cfg(target_os = "macos")]

use rui::core::color::Rgba;
use rui::core::geometry::{Bounds, Edges, Size};
use rui::core::style::Corners;
use rui::elements::text::TextAlign;
use rui::renderer::{PathVertex, Primitive, PrimitiveKind, RendererError, Scene};
use rui::testing::{FrameCaptureBackend, MetalFrameCaptureBackend};
use rui::{ImageFit, ImageSource};

fn representative_scene() -> Scene {
    let mut scene = Scene::new();
    scene.insert(Primitive::Shadow {
        bounds: Bounds::from_xywh(8.0, 8.0, 24.0, 18.0),
        corner_radii: Corners::all(4.0),
        blur_radius: 5.0,
        color: Rgba::new(0.0, 0.0, 0.0, 0.45),
    });
    scene.insert(Primitive::Quad {
        bounds: Bounds::from_xywh(10.0, 10.0, 24.0, 18.0),
        background: Rgba::new(0.95, 0.15, 0.10, 1.0),
        border_color: Rgba::WHITE,
        border_widths: Edges::all(1.0),
        corner_radii: Corners::all(4.0),
    });
    scene.insert(Primitive::LinearGradient {
        bounds: Bounds::from_xywh(40.0, 10.0, 24.0, 18.0),
        start: Rgba::new(0.10, 0.45, 0.95, 1.0),
        end: Rgba::new(0.20, 0.90, 0.50, 1.0),
        angle: 35.0,
        border_color: Rgba::TRANSPARENT,
        border_widths: Edges::ZERO,
        corner_radii: Corners::all(3.0),
    });
    scene.insert(Primitive::RadialGradient {
        bounds: Bounds::from_xywh(70.0, 10.0, 20.0, 18.0),
        inner: Rgba::WHITE,
        outer: Rgba::new(0.40, 0.20, 0.80, 1.0),
        border_color: Rgba::TRANSPARENT,
        border_widths: Edges::ZERO,
        corner_radii: Corners::ZERO,
    });
    scene.insert(Primitive::Image {
        bounds: Bounds::from_xywh(10.0, 36.0, 20.0, 18.0),
        source: ImageSource::Data {
            data: vec![
                255, 255, 0, 255, 0, 0, 255, 255, 0, 255, 255, 255, 255, 0, 255, 255,
            ],
            width: 2,
            height: 2,
        },
        fit: ImageFit::Cover,
        corner_radii: Corners::all(2.0),
        opacity: 0.75,
    });
    scene.insert(Primitive::Text {
        bounds: Bounds::from_xywh(36.0, 34.0, 44.0, 16.0),
        content: "RUI".into(),
        color: Rgba::WHITE,
        font_size: 12.0,
        font_weight: 700,
        font_family: None,
        line_height: 1.1,
        align: TextAlign::Center,
    });
    scene.insert(Primitive::PushClip {
        bounds: Bounds::from_xywh(82.0, 34.0, 12.0, 16.0),
        corner_radii: Corners::ZERO,
    });
    scene.insert(Primitive::Quad {
        bounds: Bounds::from_xywh(78.0, 34.0, 20.0, 16.0),
        background: Rgba::new(0.10, 0.85, 0.65, 1.0),
        border_color: Rgba::TRANSPARENT,
        border_widths: Edges::ZERO,
        corner_radii: Corners::ZERO,
    });
    scene.insert(Primitive::PopClip);
    scene.insert(Primitive::Path {
        vertices: vec![
            PathVertex::new(12.0, 58.0),
            PathVertex::new(28.0, 58.0),
            PathVertex::new(20.0, 48.0),
        ]
        .into(),
        offset: Default::default(),
        color: Rgba::new(1.0, 0.80, 0.10, 1.0),
        stroke_width: None,
    });
    scene.insert(Primitive::Path {
        vertices: vec![
            PathVertex::new(42.0, 58.0),
            PathVertex::new(58.0, 48.0),
            PathVertex::new(72.0, 58.0),
        ]
        .into(),
        offset: Default::default(),
        color: Rgba::new(0.10, 0.95, 1.0, 1.0),
        stroke_width: Some(3.0),
    });
    scene.finish();
    scene
}

#[test]
fn metal_frame_capture_renders_representative_scene_pixels() {
    let mut backend = match MetalFrameCaptureBackend::new() {
        Ok(backend) => backend,
        Err(RendererError::BackendUnavailable(_)) => return,
        Err(err) => panic!("metal capture backend should initialize or report unavailable: {err}"),
    };
    let viewport = Size::new(96.0, 64.0);
    let frame = match backend.capture_frame(&representative_scene(), viewport) {
        Ok(frame) => frame,
        Err(err) => panic!("metal capture should render representative scene: {err}"),
    };

    assert_eq!(frame.viewport_size, viewport);
    assert_eq!(frame.pixels.len(), 96 * 64 * 4);
    let first = &frame.pixels[0..4];
    assert!(
        frame.pixels.chunks_exact(4).any(|pixel| pixel != first),
        "captured frame should contain rendered content beyond clear color"
    );
}

#[test]
fn metal_diagnostics_report_path_as_supported_when_backend_exists() {
    let renderer = match rui::renderer::metal::MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return,
        Err(err) => panic!("metal renderer should initialize or report unavailable: {err}"),
    };
    let diagnostics = renderer.diagnostics();

    assert_eq!(diagnostics.unsupported_primitive(PrimitiveKind::Path), None);
}

fn solid(bounds: Bounds, background: Rgba) -> Primitive {
    Primitive::Quad {
        bounds,
        background,
        border_color: Rgba::new(0.0, 0.0, 0.0, 0.0),
        border_widths: Edges::ZERO,
        corner_radii: Corners::all(0.0),
    }
}

fn pixel(pixels: &[u8], width: usize, x: usize, y: usize) -> [u8; 3] {
    let at = (y * width + x) * 4;
    [pixels[at], pixels[at + 1], pixels[at + 2]]
}

fn assert_near(got: [u8; 3], want: [u8; 3], what: &str) {
    let close = got.iter().zip(want).all(|(g, w)| g.abs_diff(w) <= 2);
    assert!(close, "{what}: got {got:?}, want {want:?}");
}

fn capture(scene: &Scene, width: f32, height: f32) -> Option<Vec<u8>> {
    let mut backend = match MetalFrameCaptureBackend::new() {
        Ok(backend) => backend,
        Err(RendererError::BackendUnavailable(_)) => return None,
        Err(err) => panic!("metal capture backend should initialize: {err}"),
    };
    let frame = backend
        .capture_frame(scene, Size::new(width, height))
        .expect("capture");
    Some(frame.pixels)
}

/// Sizes on both sides of the 4 KB inline limit, each quad with its own colour, so a
/// wrong instance stride or a dropped instance shows at that quad's pixel.
#[test]
fn metal_quads_in_one_run_each_draw_their_own_instance() {
    for count in [1usize, 4, 30, 400] {
        let mut scene = Scene::new();
        let colour = |index: usize| {
            [
                (index % 16 * 16 + 8) as u8,
                (index / 16 * 16 + 8) as u8,
                200u8,
            ]
        };
        for index in 0..count {
            let [r, g, b] = colour(index);
            scene.insert(solid(
                Bounds::from_xywh(
                    (index % 20) as f32 * 8.0,
                    (index / 20) as f32 * 8.0,
                    8.0,
                    8.0,
                ),
                Rgba::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0),
            ));
        }
        scene.finish();
        let Some(pixels) = capture(&scene, 160.0, 160.0) else {
            return;
        };
        for index in 0..count {
            let (x, y) = (index % 20 * 8 + 4, index / 20 * 8 + 4);
            assert_near(
                pixel(&pixels, 160, x, y),
                colour(index),
                &format!("quad {index} of {count}"),
            );
        }
    }
}

#[test]
fn metal_quad_runs_keep_paint_order_around_images_and_clips() {
    let (red, green, blue) = (
        Rgba::new(1.0, 0.0, 0.0, 1.0),
        Rgba::new(0.0, 1.0, 0.0, 1.0),
        Rgba::new(0.0, 0.0, 1.0, 1.0),
    );
    let mut scene = Scene::new();
    // A later quad in the same run covers an earlier one.
    scene.insert(solid(Bounds::from_xywh(0.0, 0.0, 20.0, 20.0), red));
    scene.insert(solid(Bounds::from_xywh(0.0, 0.0, 20.0, 20.0), blue));
    // An image painted between two quads stays between them.
    scene.insert(solid(Bounds::from_xywh(30.0, 0.0, 20.0, 20.0), red));
    scene.insert(Primitive::Image {
        bounds: Bounds::from_xywh(30.0, 0.0, 40.0, 20.0),
        source: ImageSource::Data {
            data: vec![0, 255, 0, 255],
            width: 1,
            height: 1,
        },
        fit: ImageFit::Fill,
        corner_radii: Corners::ZERO,
        opacity: 1.0,
    });
    scene.insert(solid(Bounds::from_xywh(30.0, 0.0, 6.0, 6.0), blue));
    // A clip applies to the quads inside it and to no others.
    scene.insert(solid(Bounds::from_xywh(0.0, 40.0, 10.0, 10.0), red));
    scene.insert(Primitive::PushClip {
        bounds: Bounds::from_xywh(20.0, 40.0, 10.0, 10.0),
        corner_radii: Corners::all(0.0),
    });
    scene.insert(solid(Bounds::from_xywh(0.0, 40.0, 60.0, 10.0), green));
    scene.insert(Primitive::PopClip);
    scene.insert(solid(Bounds::from_xywh(40.0, 40.0, 10.0, 10.0), blue));
    scene.finish();
    let Some(pixels) = capture(&scene, 80.0, 60.0) else {
        return;
    };
    let width = 80;
    assert_near(
        pixel(&pixels, width, 10, 10),
        [0, 0, 255],
        "later quad on top",
    );
    assert_near(
        pixel(&pixels, width, 40, 5),
        [0, 255, 0],
        "image over the earlier quad",
    );
    assert_near(
        pixel(&pixels, width, 32, 2),
        [0, 0, 255],
        "later quad over the image",
    );
    assert_near(
        pixel(&pixels, width, 5, 45),
        [255, 0, 0],
        "quad before the clip is whole",
    );
    assert_near(
        pixel(&pixels, width, 25, 45),
        [0, 255, 0],
        "inside the clip",
    );
    assert_near(
        pixel(&pixels, width, 15, 45),
        pixel(&pixels, width, 78, 58),
        "clipped away",
    );
    assert_near(
        pixel(&pixels, width, 45, 45),
        [0, 0, 255],
        "quad after the clip is whole",
    );
}

#[test]
fn a_renderer_created_on_another_thread_draws_what_one_created_here_draws() {
    use rui::renderer::RendererResourceLimits;
    use rui::renderer::metal::MetalRenderer;

    let pending = MetalRenderer::spawn(RendererResourceLimits::default());
    let mut here = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    };
    let mut spawned = pending
        .wait()
        .expect("the spawned renderer should initialize");
    let scene = representative_scene();
    let viewport = Size::new(100.0, 60.0);
    let expected = here
        .capture_frame_pixels(&scene, viewport)
        .expect("capture");
    let actual = spawned
        .capture_frame_pixels(&scene, viewport)
        .expect("capture");
    assert!(
        expected == actual,
        "the spawned renderer drew different pixels"
    );
}
