#![cfg(target_os = "macos")]

//! Captures for draw batching: quads, shadows and text share one draw, images
//! split only what they overlap, and clips hold inside a shared draw.

use rui::core::color::Rgba;
use rui::core::geometry::{Bounds, Edges, Size};
use rui::core::style::Corners;
use rui::elements::text::TextAlign;
use rui::renderer::metal::MetalRenderer;
use rui::renderer::{Primitive, RendererError, Scene};
use rui::{ImageFit, ImageSource};

const WIDTH: usize = 120;
const HEIGHT: usize = 80;

fn renderer() -> Option<MetalRenderer> {
    match MetalRenderer::new() {
        Ok(renderer) => Some(renderer),
        Err(RendererError::BackendUnavailable(_)) => None,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    }
}

fn scene(primitives: Vec<Primitive>) -> Scene {
    let mut scene = Scene::new();
    for primitive in primitives {
        scene.insert(primitive);
    }
    scene
}

fn quad(x: f32, y: f32, width: f32, height: f32, background: Rgba) -> Primitive {
    Primitive::Quad {
        bounds: Bounds::from_xywh(x, y, width, height),
        background,
        border_color: Rgba::new(0.0, 0.0, 0.0, 0.0),
        border_widths: Edges::ZERO,
        corner_radii: Corners::all(0.0),
    }
}

fn label(x: f32, y: f32, content: &str) -> Primitive {
    Primitive::Text {
        bounds: Bounds::from_xywh(x, y, 100.0, 16.0),
        content: content.to_string(),
        color: Rgba::new(1.0, 1.0, 1.0, 1.0),
        font_size: 12.0,
        font_weight: 400,
        font_family: None,
        line_height: 1.2,
        align: TextAlign::Left,
    }
}

fn solid_image(x: f32, y: f32, size: f32, rgb: [u8; 3]) -> Primitive {
    let data = [rgb[0], rgb[1], rgb[2], 255].repeat(4 * 4);
    Primitive::Image {
        bounds: Bounds::from_xywh(x, y, size, size),
        source: ImageSource::Data {
            data,
            width: 4,
            height: 4,
        },
        fit: ImageFit::Fill,
        corner_radii: Corners::all(0.0),
        opacity: 1.0,
    }
}

fn pixel(pixels: &[u8], x: usize, y: usize) -> [u8; 3] {
    let at = (y * WIDTH + x) * 4;
    [pixels[at], pixels[at + 1], pixels[at + 2]]
}

fn capture(renderer: &mut MetalRenderer, scene: &Scene) -> Vec<u8> {
    renderer
        .capture_frame_pixels(scene, Size::new(WIDTH as f32, HEIGHT as f32))
        .expect("capture")
}

fn rows(count: usize) -> Scene {
    let mut primitives = Vec::new();
    for row in 0..count {
        let y = row as f32 * 8.0;
        primitives.push(Primitive::Shadow {
            bounds: Bounds::from_xywh(2.0, y + 1.0, 30.0, 6.0),
            corner_radii: Corners::all(2.0),
            blur_radius: 2.0,
            color: Rgba::new(0.0, 0.0, 0.0, 0.4),
        });
        primitives.push(quad(
            0.0,
            y,
            WIDTH as f32,
            8.0,
            Rgba::new(0.1, 0.12, 0.15, 1.0),
        ));
        primitives.push(label(4.0, y - 4.0, &format!("row {row}")));
        primitives.push(label(80.0, y - 4.0, "5 min"));
    }
    scene(primitives)
}

#[test]
fn rows_of_shadows_quads_and_text_draw_in_one_call() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    for count in [3, 10] {
        capture(&mut renderer, &rows(count));
        let stats = renderer.last_frame_stats();
        assert_eq!(stats.draw_calls, 1, "{count} rows");
        assert!(
            stats.instances as usize > count * 4,
            "{count} rows: {stats:?}"
        );
    }
}

#[test]
fn an_image_splits_only_what_overlaps_it_and_order_holds() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let red = Rgba::new(1.0, 0.0, 0.0, 1.0);
    let blue = Rgba::new(0.0, 0.0, 1.0, 1.0);
    let yellow = Rgba::new(1.0, 1.0, 0.0, 1.0);
    let frame = scene(vec![
        quad(0.0, 0.0, 40.0, 40.0, red),
        // Green, over the red quad's right half.
        solid_image(20.0, 0.0, 40.0, [0, 255, 0]),
        // Over the image's right half: must come after it.
        quad(40.0, 0.0, 40.0, 40.0, blue),
        // Over nothing: joins the red quad's draw.
        quad(0.0, 50.0, 40.0, 20.0, yellow),
    ]);
    let pixels = capture(&mut renderer, &frame);

    assert_eq!(renderer.last_frame_stats().draw_calls, 3);
    assert_eq!(pixel(&pixels, 10, 20), [255, 0, 0], "red alone");
    assert_eq!(pixel(&pixels, 30, 20), [0, 255, 0], "image over red");
    assert_eq!(pixel(&pixels, 50, 20), [0, 0, 255], "blue over image");
    assert_eq!(pixel(&pixels, 10, 60), [255, 255, 0], "yellow");
}

#[test]
fn clips_hold_inside_a_shared_draw() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let white = Rgba::new(1.0, 1.0, 1.0, 1.0);
    let frame = scene(vec![
        quad(
            0.0,
            0.0,
            WIDTH as f32,
            HEIGHT as f32,
            Rgba::new(0.0, 0.0, 0.0, 1.0),
        ),
        Primitive::PushClip {
            bounds: Bounds::from_xywh(10.0, 10.0, 20.0, 20.0),
            corner_radii: Corners::all(0.0),
        },
        quad(0.0, 0.0, 60.0, 60.0, white),
        Primitive::PopClip,
        Primitive::PushClip {
            bounds: Bounds::from_xywh(70.0, 40.0, 30.0, 20.0),
            corner_radii: Corners::all(0.0),
        },
        quad(50.0, 30.0, 70.0, 50.0, white),
        Primitive::PopClip,
    ]);
    let pixels = capture(&mut renderer, &frame);

    assert_eq!(renderer.last_frame_stats().draw_calls, 1);
    for (x, y, inside) in [
        (10, 10, true),
        (29, 29, true),
        (9, 20, false),
        (30, 20, false),
        (20, 30, false),
        (70, 40, true),
        (99, 59, true),
        (100, 50, false),
        (85, 60, false),
        (60, 35, false),
    ] {
        let expected = if inside { [255; 3] } else { [0; 3] };
        assert_eq!(pixel(&pixels, x, y), expected, "pixel ({x}, {y})");
    }
}
