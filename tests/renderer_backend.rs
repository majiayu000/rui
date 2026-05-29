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
        content: String::from("RUI"),
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
        ],
        color: Rgba::new(1.0, 0.80, 0.10, 1.0),
        stroke_width: None,
    });
    scene.insert(Primitive::Path {
        vertices: vec![
            PathVertex::new(42.0, 58.0),
            PathVertex::new(58.0, 48.0),
            PathVertex::new(72.0, 58.0),
        ],
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
