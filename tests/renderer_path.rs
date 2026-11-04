#![cfg(target_os = "macos")]

use rui::core::color::Rgba;
use rui::core::geometry::Size;
use rui::renderer::{PathVertex, Primitive, RendererError, Scene};
use rui::testing::{FrameCaptureBackend, MetalFrameCaptureBackend};

const WIDTH: usize = 80;
const HEIGHT: usize = 60;

/// Pixel at (x, y) of a frame holding one green path, or None without a Metal device.
fn draw(vertices: &[(f32, f32)], stroke_width: Option<f32>) -> Option<Vec<u8>> {
    let mut backend = match MetalFrameCaptureBackend::new() {
        Ok(backend) => backend,
        Err(RendererError::BackendUnavailable(_)) => return None,
        Err(err) => panic!("metal capture backend should initialize: {err}"),
    };
    let mut scene = Scene::new();
    scene.insert(Primitive::Path {
        vertices: vertices
            .iter()
            .map(|(x, y)| PathVertex::new(*x, *y))
            .collect(),
        color: Rgba::new(0.0, 1.0, 0.0, 1.0),
        stroke_width,
    });
    scene.finish();
    let frame = backend
        .capture_frame(&scene, Size::new(WIDTH as f32, HEIGHT as f32))
        .expect("capture");
    Some(frame.pixels)
}

fn is_green(pixels: &[u8], x: usize, y: usize) -> bool {
    let at = (y * WIDTH + x) * 4;
    pixels[at] < 40 && pixels[at + 1] > 215 && pixels[at + 2] < 40
}

#[test]
fn filled_path_covers_its_inside_and_nothing_else() {
    let Some(pixels) = draw(&[(10.0, 10.0), (50.0, 10.0), (10.0, 50.0)], None) else {
        return;
    };
    assert!(is_green(&pixels, 15, 15), "inside the triangle");
    assert!(is_green(&pixels, 25, 25), "inside, near the long edge");
    assert!(!is_green(&pixels, 45, 45), "past the long edge");
    assert!(!is_green(&pixels, 70, 55), "far outside");
}

#[test]
fn stroked_path_covers_the_line_and_nothing_else() {
    let Some(pixels) = draw(&[(10.0, 30.0), (70.0, 30.0)], Some(8.0)) else {
        return;
    };
    assert!(is_green(&pixels, 40, 30), "on the line");
    assert!(is_green(&pixels, 40, 27), "within half the width");
    assert!(!is_green(&pixels, 40, 20), "beyond the width");
    assert!(!is_green(&pixels, 75, 30), "past the end");
}
