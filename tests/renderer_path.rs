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

#[test]
fn stroked_corner_is_mitered() {
    // Right, then down, 8 wide: the outer corner reaching (64, 16) is the miter's,
    // which butt-ended segments alone leave empty.
    let Some(pixels) = draw(&[(10.0, 20.0), (60.0, 20.0), (60.0, 55.0)], Some(8.0)) else {
        return;
    };
    assert!(
        is_green(&pixels, 62, 17),
        "in the miter, outside both segments"
    );
    assert!(!is_green(&pixels, 66, 13), "past the miter tip");
}

#[test]
fn an_edge_on_a_tile_boundary_crossing_no_column_is_drawn() {
    // Too large for one instance, so drawn as tiles; its left edge lies on the
    // boundary of the second column of 16 px tiles.
    let Some(pixels) = draw(
        &[(16.0, 2.0), (90.0, 2.0), (90.0, 58.0), (16.0, 58.0)],
        None,
    ) else {
        return;
    };
    assert!(is_green(&pixels, 16, 30), "just right of the edge");
    assert!(!is_green(&pixels, 15, 30), "just left of it");
    assert!(is_green(&pixels, 60, 30), "inside");
}
