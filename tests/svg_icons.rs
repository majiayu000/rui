//! SVG icons drawn on the GPU: fills cover what they wind around, holes stay
//! open, and strokes cover their joins and caps.

use rui::renderer::RendererError;
use rui::renderer::metal::MetalRenderer;
use rui::testing::mount;
use rui::{Color, Size, div, svg};

fn renderer() -> Option<MetalRenderer> {
    match MetalRenderer::new() {
        Ok(renderer) => Some(renderer),
        Err(RendererError::BackendUnavailable(_)) => None,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    }
}

const VIEWPORT: Size = Size::new(64.0, 64.0);

/// The frame with `markup` drawn 40 x 40 at (12, 12) in red, at scale 1.
fn draw(renderer: &mut MetalRenderer, markup: &'static str) -> Vec<u8> {
    let session = mount(VIEWPORT, move |_| {
        div().p(12.0).child(
            svg(markup)
                .size(Size::new(40.0, 40.0))
                .color(Color::hex(0xff0000)),
        )
    })
    .expect("mount");
    renderer
        .capture_frame_pixels_at_scale(session.scene(), VIEWPORT, 1.0)
        .expect("capture")
}

fn pixel(pixels: &[u8], x: usize, y: usize) -> [u8; 4] {
    let at = (y * VIEWPORT.width as usize + x) * 4;
    pixels[at..at + 4].try_into().unwrap()
}

const RED: [u8; 4] = [255, 0, 0, 255];

#[test]
fn a_filled_square_covers_exactly_its_box() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let pixels = draw(
        &mut renderer,
        r#"<svg viewBox="0 0 10 10"><path d="M0 0H10V10H0Z"/></svg>"#,
    );
    let background = pixel(&pixels, 2, 2);
    assert_ne!(background, RED);
    for y in 0..64 {
        for x in 0..64 {
            let inside = (12..52).contains(&x) && (12..52).contains(&y);
            let expected = if inside { RED } else { background };
            assert_eq!(pixel(&pixels, x, y), expected, "pixel ({x}, {y})");
        }
    }
}

#[test]
fn a_subpath_wound_the_other_way_is_a_hole() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    // Outer square clockwise; inner square clockwise, then counterclockwise.
    let solid = draw(
        &mut renderer,
        r#"<svg viewBox="0 0 10 10"><path d="M0 0H10V10H0Z M3 3H7V7H3Z"/></svg>"#,
    );
    let ring = draw(
        &mut renderer,
        r#"<svg viewBox="0 0 10 10"><path d="M0 0H10V10H0Z M3 3V7H7V3Z"/></svg>"#,
    );
    let background = pixel(&ring, 2, 2);
    assert_eq!(pixel(&solid, 32, 32), RED, "same winding fills the middle");
    assert_eq!(
        pixel(&ring, 32, 32),
        background,
        "opposite winding leaves a hole"
    );
    for (x, y) in [(14, 14), (50, 32), (32, 50), (14, 50)] {
        assert_eq!(pixel(&ring, x, y), RED, "ring at ({x}, {y})");
    }
}

#[test]
fn strokes_cover_their_joins_and_caps() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    // An L from (2, 2) down to (2, 8) and right to (8, 8), 2 units wide: 8 px.
    let miter = draw(
        &mut renderer,
        r##"<svg viewBox="0 0 10 10"><polyline points="2 2 2 8 8 8" fill="none" stroke="#000" stroke-width="2"/></svg>"##,
    );
    let round = draw(
        &mut renderer,
        r##"<svg viewBox="0 0 10 10"><polyline points="2 2 2 8 8 8" fill="none" stroke="#000" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/></svg>"##,
    );
    let background = pixel(&miter, 1, 1);
    // The outer corner of the join: (1, 9) in units is (16, 48) in pixels.
    assert_eq!(pixel(&miter, 17, 47), RED, "the miter fills the corner");
    assert_ne!(pixel(&round, 17, 47), RED, "a round join cuts the corner");
    // Next to the corner along both legs, both cover.
    for pixels in [&miter, &round] {
        assert_eq!(pixel(pixels, 20, 40), RED);
        assert_eq!(pixel(pixels, 28, 47), RED);
    }
    // Butt caps end at the points; round caps reach a unit past them.
    assert_eq!(pixel(&miter, 20, 18), background, "butt cap");
    assert_eq!(pixel(&round, 20, 18), RED, "round cap");
    assert_eq!(pixel(&round, 46, 44), RED, "round cap at the other end");
    // Inside the L, nothing.
    assert_eq!(pixel(&miter, 36, 30), background);
}
