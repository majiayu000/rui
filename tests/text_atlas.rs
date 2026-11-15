#![cfg(target_os = "macos")]

use rui::core::color::Rgba;
use rui::core::geometry::{Bounds, Size};
use rui::elements::text::TextAlign;
use rui::renderer::metal::MetalRenderer;
use rui::renderer::text::BaseDirection;
use rui::renderer::{Primitive, RendererError, Scene};

const VIEWPORT: Size = Size {
    width: 120.0,
    height: 60.0,
};

fn text(content: &str, x: f32, color: Rgba) -> Primitive {
    Primitive::Text {
        bounds: Bounds::from_xywh(x, 10.0, 50.0, 40.0),
        content: content.into(),
        color,
        font_size: 28.0,
        font_weight: 400,
        font_family: None,
        line_height: 1.2,
        align: TextAlign::Left,
        direction: BaseDirection::Auto,
    }
}

/// Bounding box (min_x, min_y, max_x, max_y) of pixels whose green channel is
/// above `threshold`, in device pixels.
fn lit_box(pixels: &[u8], width: usize, threshold: u8) -> Option<(usize, usize, usize, usize)> {
    let mut lit = None::<(usize, usize, usize, usize)>;
    for (index, pixel) in pixels.chunks_exact(4).enumerate() {
        if pixel[1] <= threshold {
            continue;
        }
        let (x, y) = (index % width, index / width);
        lit = Some(match lit {
            None => (x, y, x, y),
            Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x), d.max(y)),
        });
    }
    lit
}

fn capture(scene: &Scene, scale: f32) -> Option<Vec<u8>> {
    let mut renderer = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return None,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    };
    Some(
        renderer
            .capture_frame_pixels_at_scale(scene, VIEWPORT, scale)
            .expect("capture"),
    )
}

#[test]
fn glyphs_from_the_atlas_land_inside_their_text_bounds() {
    let mut scene = Scene::new();
    scene.insert(text("HI", 5.0, Rgba::new(0.0, 1.0, 0.0, 1.0)));
    scene.insert(text("HI", 65.0, Rgba::new(0.0, 1.0, 0.0, 1.0)));
    scene.finish();
    let Some(pixels) = capture(&scene, 1.0) else {
        return;
    };

    let width = VIEWPORT.width as usize;
    let (min_x, min_y, max_x, max_y) = lit_box(&pixels, width, 128).expect("text drew pixels");
    assert!(min_x >= 5 && max_x < 115, "ink spans {min_x}..{max_x}");
    assert!(min_y >= 10 && max_y < 50, "ink spans {min_y}..{max_y}");

    // Both strings drew, from shared glyphs, with identical pixels.
    let column = |x0: usize| {
        (10..50)
            .flat_map(|y| (x0..x0 + 50).map(move |x| (y * width + x) * 4 + 1))
            .map(|at| pixels[at])
            .collect::<Vec<_>>()
    };
    assert!(column(5).iter().any(|&g| g > 200));
    assert_eq!(column(5), column(65));
}

#[test]
fn text_is_rasterized_at_device_scale() {
    let mut scene = Scene::new();
    scene.insert(text("H", 10.0, Rgba::new(0.0, 1.0, 0.0, 1.0)));
    scene.finish();
    let (Some(one), Some(two)) = (capture(&scene, 1.0), capture(&scene, 2.0)) else {
        return;
    };

    let (ax, ay, bx, by) = lit_box(&one, VIEWPORT.width as usize, 128).expect("1x ink");
    let (cx, cy, dx, dy) = lit_box(&two, VIEWPORT.width as usize * 2, 128).expect("2x ink");
    let (w1, h1) = ((bx - ax + 1) as f32, (by - ay + 1) as f32);
    let (w2, h2) = ((dx - cx + 1) as f32, (dy - cy + 1) as f32);
    assert!((w2 / w1 - 2.0).abs() < 0.25, "2x width {w2} vs 1x {w1}");
    assert!((h2 / h1 - 2.0).abs() < 0.25, "2x height {h2} vs 1x {h1}");
    assert!((cx as f32 - 2.0 * ax as f32).abs() <= 2.0);
    assert!((cy as f32 - 2.0 * ay as f32).abs() <= 2.0);

    // A 1x string stretched to 2x is mostly soft edges (about four partly covered
    // pixels per solid one before the atlas); glyphs drawn at device scale are
    // mostly solid.
    let partial = two
        .chunks_exact(4)
        .filter(|pixel| pixel[1] > 60 && pixel[1] < 200)
        .count();
    let solid = two.chunks_exact(4).filter(|pixel| pixel[1] >= 200).count();
    assert!(
        solid > 0 && partial < solid,
        "{partial} partial, {solid} solid"
    );
}

#[test]
fn text_sits_on_one_baseline_whatever_its_ink() {
    // Wrapped lines are drawn one primitive each; a short line must not float
    // to a different baseline than a tall one.
    let green = Rgba::new(0.0, 1.0, 0.0, 1.0);
    let mut scene = Scene::new();
    scene.insert(text("xx", 5.0, green));
    scene.insert(text("Xx", 65.0, green));
    scene.finish();
    let Some(pixels) = capture(&scene, 1.0) else {
        return;
    };

    let width = VIEWPORT.width as usize;
    let half = |x0: usize| {
        let pixels: Vec<u8> = pixels
            .chunks_exact(4 * width)
            .flat_map(|row| row[x0 * 4..(x0 + 55) * 4].to_vec())
            .collect();
        lit_box(&pixels, 55, 128).expect("text drew pixels")
    };
    let (short, tall) = (half(0), half(60));
    assert_eq!(short.3, tall.3, "baselines differ: {short:?} vs {tall:?}");
    assert!(
        tall.1 < short.1,
        "capital should reach higher: {short:?} vs {tall:?}"
    );
}

/// Large glyphs at six sizes from `first`: most of an atlas page at scale 2.
fn glyph_page_scene(first: usize) -> Scene {
    let mut scene = Scene::new();
    for (index, size) in (first..first + 6).enumerate() {
        scene.insert(Primitive::Text {
            bounds: Bounds::from_xywh(0.0, index as f32 * 10.0, 400.0, 10.0),
            content: "WgQ@#&".into(),
            color: Rgba::new(1.0, 1.0, 1.0, 1.0),
            font_size: 60.0 + size as f32,
            font_weight: 400,
            font_family: None,
            line_height: 1.2,
            align: TextAlign::Left,
            direction: BaseDirection::Auto,
        });
    }
    scene
}

#[test]
fn evicted_atlas_pages_draw_what_a_fresh_atlas_draws() {
    use rui::renderer::{RendererResourceKind, RendererResourceLimits};

    let limits = RendererResourceLimits {
        glyph_max_bytes: 3 * 1024 * 1024,
        ..RendererResourceLimits::default()
    };
    let mut renderer = match MetalRenderer::with_resource_limits(limits) {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    };
    let viewport = Size::new(400.0, 60.0);
    // Each scene needs most of a page, so with room for three, frames soon
    // empty pages earlier frames drew from, and draw again what they held.
    for (frame, first) in [0, 8, 16, 0, 24, 8, 0, 32, 16].into_iter().enumerate() {
        let scene = glyph_page_scene(first);
        let pixels = renderer
            .capture_frame_pixels_at_scale(&scene, viewport, 2.0)
            .unwrap_or_else(|err| panic!("frame {frame}: {err}"));
        let mut fresh = MetalRenderer::new().expect("renderer");
        let expected = fresh
            .capture_frame_pixels_at_scale(&scene, viewport, 2.0)
            .expect("capture");
        assert!(
            pixels == expected,
            "frame {frame} (sizes from {first}) differs from a fresh atlas"
        );
    }
    let glyphs = renderer
        .diagnostics()
        .resource(RendererResourceKind::Glyph)
        .cloned()
        .expect("glyph stats");
    assert!(glyphs.evicted_entries > 0, "{glyphs:?}");
    assert!(glyphs.live_bytes <= 3 * 1024 * 1024, "{glyphs:?}");
}

/// Device pixels whose channels differ: colored, which white text on black is not.
fn colored_box(pixels: &[u8], width: usize) -> Option<(usize, usize, usize, usize)> {
    let mut found = None::<(usize, usize, usize, usize)>;
    for (index, pixel) in pixels.chunks_exact(4).enumerate() {
        let (low, high) = (pixel[..3].iter().min(), pixel[..3].iter().max());
        if high.zip(low).is_some_and(|(high, low)| high - low > 80) {
            let (x, y) = (index % width, index / width);
            found = Some(match found {
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                None => (x, y, x, y),
            });
        }
    }
    found
}

#[test]
fn emoji_draw_in_color_beside_text_in_two_draws() {
    let mut renderer = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    };
    let frame = |alpha: f32| {
        let mut scene = Scene::new();
        scene.insert(Primitive::Quad {
            bounds: Bounds::from_xywh(0.0, 0.0, 120.0, 60.0),
            background: Rgba::new(0.0, 0.0, 0.0, 1.0),
            border_color: Rgba::new(0.0, 0.0, 0.0, 0.0),
            border_widths: rui::core::geometry::Edges::ZERO,
            corner_radii: rui::core::style::Corners::all(0.0),
        });
        scene.insert(text("Hi \u{1F600}", 10.0, Rgba::new(1.0, 1.0, 1.0, alpha)));
        scene
    };
    let pixels = renderer
        .capture_frame_pixels_at_scale(&frame(1.0), VIEWPORT, 2.0)
        .expect("capture");
    let width = (VIEWPORT.width * 2.0) as usize;
    let emoji_font = std::path::Path::new("/System/Library/Fonts/Apple Color Emoji.ttc");
    let Some((x0, y0, x1, y1)) = colored_box(&pixels, width) else {
        assert!(
            !emoji_font.exists(),
            "Apple Color Emoji is installed but drew no color"
        );
        return;
    };
    // The quad and "Hi " draw with the simple pipeline, the emoji after them.
    assert_eq!(
        renderer.last_frame_stats().draw_calls,
        2,
        "the emoji follows the simple draw"
    );
    // Right of "Hi ", within the 40-pixel line at 10 + 0..40, about 28 px tall.
    let (x0, y0, x1, y1) = (x0 / 2, y0 / 2, x1 / 2, y1 / 2);
    assert!(x0 > 30 && x1 < 70, "emoji at x {x0}..{x1}");
    assert!(y0 >= 10 && y1 <= 50, "emoji at y {y0}..{y1}");
    assert!((18..=32).contains(&(y1 - y0)), "emoji {} px tall", y1 - y0);

    // The text's opacity dims the emoji too.
    let faded = renderer
        .capture_frame_pixels_at_scale(&frame(0.5), VIEWPORT, 2.0)
        .expect("capture");
    let brightest = |pixels: &[u8]| {
        pixels
            .chunks_exact(4)
            .map(|pixel| u32::from(pixel[0]) + u32::from(pixel[1]) + u32::from(pixel[2]))
            .max()
            .unwrap_or(0)
    };
    assert!(brightest(&faded) < brightest(&pixels));
}
