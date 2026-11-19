#![cfg(target_os = "macos")]
//! Glyphs resolved once and drawn by a canvas at pen positions of its choosing.

use rui::core::color::{Color, Rgba};
use rui::core::geometry::{Bounds, Point, Size};
use rui::elements::text::TextAlign;
use rui::elements::{canvas, div};
use rui::renderer::metal::MetalRenderer;
use rui::renderer::text::{BaseDirection, FontGlyph, GlyphResolver};
use rui::renderer::{Primitive, RendererError, Scene};
use rui::testing::mount;

const VIEWPORT: Size = Size {
    width: 160.0,
    height: 60.0,
};
const GREEN: Rgba = Rgba {
    r: 0.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};
const FONT_SIZE: f32 = 20.0;
const BASELINE: f32 = 40.0;

fn capture(scene: &Scene) -> Option<Vec<u8>> {
    let mut renderer = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return None,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    };
    Some(
        renderer
            .capture_frame_pixels_at_scale(scene, VIEWPORT, 1.0)
            .expect("capture"),
    )
}

/// (min_x, min_y, max_x, max_y) of pixels whose green is above 128 within
/// columns `x0..x1`.
fn lit_box(pixels: &[u8], x0: usize, x1: usize) -> Option<(usize, usize, usize, usize)> {
    let width = VIEWPORT.width as usize;
    let mut lit = None::<(usize, usize, usize, usize)>;
    for (index, pixel) in pixels.as_chunks::<4>().0.iter().enumerate() {
        let (x, y) = (index % width, index / width);
        if pixel[1] <= 128 || x < x0 || x >= x1 {
            continue;
        }
        lit = Some(match lit {
            None => (x, y, x, y),
            Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x), d.max(y)),
        });
    }
    lit
}

fn glyph(glyph: FontGlyph, x: f32) -> Primitive {
    Primitive::Glyph {
        glyph,
        origin: Point::new(x, BASELINE),
        font_size: FONT_SIZE,
        color: GREEN,
    }
}

fn scene(primitives: impl IntoIterator<Item = Primitive>) -> Scene {
    let mut scene = Scene::new();
    for primitive in primitives {
        scene.insert(primitive);
    }
    scene.finish();
    scene
}

#[test]
fn a_glyph_draws_its_ink_from_its_pen_on_the_baseline() {
    let mut resolver = GlyphResolver::new(None, 400);
    let bar = resolver.resolve('|').expect("the default font has a bar");
    let Some(pixels) = capture(&scene([glyph(bar, 20.0), glyph(bar, 100.0)])) else {
        return;
    };

    let advance = bar.advance(FONT_SIZE);
    let first = lit_box(&pixels, 0, 80).expect("the first bar drew");
    let second = lit_box(&pixels, 80, 160).expect("the second bar drew");
    // Ink between the pen and the next pen, above and a little below the baseline.
    assert!(
        first.0 >= 20 && (first.2 as f32) < 20.0 + advance,
        "ink {first:?}, advance {advance}"
    );
    assert!(
        (first.1 as f32) < BASELINE - FONT_SIZE * 0.5
            && (first.3 as f32) < BASELINE + FONT_SIZE * 0.5
    );
    // The same pixels, moved with the pen.
    assert_eq!(
        (second.0 - first.0, second.1, second.2 - first.2, second.3),
        (80, first.1, 80, first.3)
    );
}

#[test]
fn a_glyph_is_the_same_raster_as_its_character_in_text() {
    let mut resolver = GlyphResolver::new(None, 400);
    let letter = resolver.resolve('H').expect("the default font has H");
    let text = Primitive::Text {
        bounds: Bounds::from_xywh(100.0, 10.0, 50.0, 40.0),
        content: "H".into(),
        color: GREEN,
        font_size: FONT_SIZE,
        font_weight: 400,
        font_family: None,
        font_features: Default::default(),
        line_height: 1.2,
        align: TextAlign::Left,
        direction: BaseDirection::Auto,
    };
    let Some(pixels) = capture(&scene([glyph(letter, 20.0), text])) else {
        return;
    };
    let drawn = lit_box(&pixels, 0, 80).expect("the glyph drew");
    let shaped = lit_box(&pixels, 80, 160).expect("the text drew");
    assert_eq!(
        (drawn.2 - drawn.0, drawn.3 - drawn.1),
        (shaped.2 - shaped.0, shaped.3 - shaped.1),
        "glyph ink {drawn:?}, text ink {shaped:?}"
    );
}

/// A terminal row: a wide character centred on two cells, then a bar on the
/// next cell. One text run would put the bar where the wide glyph's own
/// advance ends instead.
#[test]
fn wide_glyphs_stay_on_the_cells_a_canvas_gives_them() {
    let mut resolver = GlyphResolver::new(None, 400);
    let cell = resolver.resolve('0').expect("digits").advance(FONT_SIZE);
    let wide = resolver.resolve('中').expect("a CJK fallback face");
    let bar = resolver.resolve('|').expect("bar");
    let session = mount(VIEWPORT, move |_| {
        div().child(
            canvas(move |bounds, painter| {
                let baseline = bounds.y() + BASELINE;
                let centred = (2.0 * cell - wide.advance(FONT_SIZE)) * 0.5;
                painter.glyph(
                    wide,
                    Point::new(bounds.x() + centred, baseline),
                    FONT_SIZE,
                    Color::from(GREEN),
                );
                painter.glyph(
                    bar,
                    Point::new(bounds.x() + 2.0 * cell, baseline),
                    FONT_SIZE,
                    Color::from(GREEN),
                );
                // Past the canvas: clipped away.
                painter.glyph(
                    bar,
                    Point::new(bounds.x() + 120.0, baseline),
                    FONT_SIZE,
                    Color::from(GREEN),
                );
            })
            .w(100.0)
            .h(VIEWPORT.height),
        )
    })
    .expect("mount");
    let glyphs = session
        .primitives()
        .iter()
        .filter(|primitive| matches!(primitive, Primitive::Glyph { .. }))
        .count();
    assert_eq!(glyphs, 3);
    let Some(pixels) = capture(&scene(session.primitives().iter().cloned())) else {
        return;
    };

    let two_cells = (2.0 * cell).ceil() as usize;
    let wide_ink = lit_box(&pixels, 0, two_cells).expect("the wide glyph drew");
    let bar_ink = lit_box(&pixels, two_cells, 100).expect("the bar drew");
    assert!(
        (wide_ink.2 as f32) < 2.0 * cell,
        "the wide glyph stays in its two cells: {wide_ink:?}, cell {cell}"
    );
    assert!(
        bar_ink.0 as f32 >= 2.0 * cell && (bar_ink.2 as f32) < 3.0 * cell,
        "the bar sits on the third cell: {bar_ink:?}, cell {cell}"
    );
    assert_eq!(lit_box(&pixels, 100, 160), None, "clipped to the canvas");
}
