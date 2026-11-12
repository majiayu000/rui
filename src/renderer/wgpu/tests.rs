//! The wgpu backend against the Metal one: the same scenes, planned by the
//! same `plan` code, drawn by both, compared pixel by pixel.
//!
//! Tolerance: wgpu compiles the WGSL ports through naga to MSL, while the
//! Metal backend compiles hand-written MSL, so `pow`, `exp`, `fwidth` and the
//! fast-math rounding around them can differ in the last bits. That moves
//! antialiased edges and blurred shadows by a few 8-bit levels; fills, clips,
//! draw order, glyph coverage and blending must agree. A frame passes when
//! every channel of every pixel is within [`MAX_LEVELS`] and at most
//! [`MAX_EDGE_SHARE`] of its pixels differ by more than [`EDGE_LEVELS`].
//!
//! Measured on an M-series Mac (wgpu 29 over Metal): the fixture and the
//! retained frames are identical; of the 48 random scenes 44 are identical,
//! three differ by 1 level and one by 4 levels on 0.17% of its pixels. The
//! limits leave room for other GPUs, not for a missing or misordered draw,
//! which changes far more than 1% of a frame by far more than 8 levels.

use super::{RetainedTarget, WgpuRenderer};
use crate::core::color::Rgba;
use crate::core::geometry::{Bounds, Edges, Size};
use crate::core::style::Corners;
use crate::elements::text::TextAlign;
use crate::renderer::metal::tests::{HEIGHT, WIDTH, random_scene};
use crate::renderer::metal::{self, MetalRenderer};
use crate::renderer::{PathVertex, Primitive, RendererError, Scene};
use crate::{ImageFit, ImageSource};

/// No channel may differ by more than this many 8-bit levels.
const MAX_LEVELS: u8 = 8;
/// Channels differing by more than this count as an edge difference.
const EDGE_LEVELS: u8 = 2;
/// Share of a frame's pixels that may differ by more than [`EDGE_LEVELS`].
const MAX_EDGE_SHARE: f64 = 0.01;

fn renderers() -> Option<(MetalRenderer, WgpuRenderer)> {
    let metal = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return None,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    };
    let wgpu = match WgpuRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return None,
        Err(err) => panic!("wgpu renderer should initialize: {err}"),
    };
    Some((metal, wgpu))
}

#[derive(Debug, Default)]
struct Difference {
    max: u8,
    /// Pixels with a channel past [`EDGE_LEVELS`].
    edge_pixels: usize,
    pixels: usize,
    /// The first pixel with the largest difference: x, y, Metal, wgpu.
    worst: Option<(usize, usize, [u8; 4], [u8; 4])>,
}

fn difference(metal: &[u8], wgpu: &[u8], width: usize) -> Difference {
    assert_eq!(metal.len(), wgpu.len(), "captures differ in size");
    let mut diff = Difference {
        pixels: metal.len() / 4,
        ..Default::default()
    };
    for (index, (a, b)) in metal.chunks_exact(4).zip(wgpu.chunks_exact(4)).enumerate() {
        let channel = a
            .iter()
            .zip(b)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        if channel > EDGE_LEVELS {
            diff.edge_pixels += 1;
        }
        if channel > diff.max {
            diff.max = channel;
            diff.worst = Some((
                index % width,
                index / width,
                a.try_into().unwrap(),
                b.try_into().unwrap(),
            ));
        }
    }
    diff
}

fn assert_close(metal: &[u8], wgpu: &[u8], width: usize, what: &str) {
    let diff = difference(metal, wgpu, width);
    let share = diff.edge_pixels as f64 / diff.pixels.max(1) as f64;
    assert!(
        diff.max <= MAX_LEVELS && share <= MAX_EDGE_SHARE,
        "{what}: wgpu differs from Metal by up to {} levels, {:.3}% of pixels past {EDGE_LEVELS}; \
         worst (x, y, metal, wgpu): {:?}",
        diff.max,
        share * 100.0,
        diff.worst
    );
}

/// One of each primitive the backends draw, including what the random scenes
/// leave to chance: a radial gradient, color glyphs, a rounded image, a large
/// path drawn as tiles and a small one drawn from the atlas.
fn fixture_scene() -> Scene {
    let mut scene = Scene::new();
    scene.insert(Primitive::Quad {
        bounds: Bounds::from_xywh(0.0, 0.0, WIDTH, HEIGHT),
        background: Rgba::new(0.96, 0.95, 0.92, 1.0),
        border_color: Rgba::TRANSPARENT,
        border_widths: Edges::ZERO,
        corner_radii: Corners::ZERO,
    });
    scene.insert(Primitive::Shadow {
        bounds: Bounds::from_xywh(12.0, 12.0, 60.0, 36.0),
        corner_radii: Corners::all(8.0),
        blur_radius: 8.0,
        color: Rgba::new(0.0, 0.0, 0.0, 0.4),
    });
    scene.insert(Primitive::Quad {
        bounds: Bounds::from_xywh(12.0, 12.0, 60.0, 36.0),
        background: Rgba::new(0.20, 0.45, 0.85, 1.0),
        border_color: Rgba::new(0.05, 0.10, 0.30, 1.0),
        border_widths: Edges::all(2.0),
        corner_radii: Corners::all(8.0),
    });
    scene.insert(Primitive::RadialGradient {
        bounds: Bounds::from_xywh(84.0, 10.0, 30.0, 30.0),
        inner: Rgba::WHITE,
        outer: Rgba::new(0.40, 0.20, 0.80, 1.0),
        border_color: Rgba::TRANSPARENT,
        border_widths: Edges::ZERO,
        corner_radii: Corners::all(15.0),
    });
    scene.insert(Primitive::LinearGradient {
        bounds: Bounds::from_xywh(120.0, 10.0, 32.0, 30.0),
        start: Rgba::new(1.0, 0.3, 0.1, 1.0),
        end: Rgba::new(0.1, 0.6, 1.0, 0.6),
        angle: 30.0,
        border_color: Rgba::TRANSPARENT,
        border_widths: Edges::ZERO,
        corner_radii: Corners::all(4.0),
    });
    scene.insert(Primitive::PushClip {
        bounds: Bounds::from_xywh(8.0, 56.0, 100.0, 24.0),
        corner_radii: Corners::ZERO,
    });
    scene.insert(Primitive::Text {
        bounds: Bounds::from_xywh(10.0, 56.0, 140.0, 24.0),
        content: "Wgpu & Metal \u{1F600} clipped".into(),
        color: Rgba::new(0.1, 0.1, 0.12, 1.0),
        font_size: 14.0,
        font_weight: 400,
        font_family: None,
        line_height: 1.2,
        align: TextAlign::Left,
    });
    scene.insert(Primitive::PopClip);
    scene.insert(Primitive::Image {
        bounds: Bounds::from_xywh(116.0, 50.0, 36.0, 36.0),
        source: ImageSource::Data {
            data: (0..8 * 8)
                .flat_map(|i| [(i * 31) as u8, (i * 7) as u8, 200, 255])
                .collect(),
            width: 8,
            height: 8,
        },
        fit: ImageFit::Cover,
        corner_radii: Corners::all(6.0),
        opacity: 0.9,
    });
    // Large: analytic tiles. Small: rasterized into the atlas once seen.
    scene.insert(Primitive::Path {
        vertices: [(10.0, 116.0), (60.0, 84.0), (110.0, 116.0), (60.0, 100.0)]
            .map(|(x, y)| PathVertex::new(x, y))
            .into(),
        offset: Default::default(),
        color: Rgba::new(0.1, 0.6, 0.3, 0.9),
        stroke_width: None,
    });
    scene.insert(Primitive::Path {
        vertices: [(120.0, 112.0), (130.0, 94.0), (140.0, 112.0), (150.0, 96.0)]
            .map(|(x, y)| PathVertex::new(x, y))
            .into(),
        offset: Default::default(),
        color: Rgba::new(0.8, 0.2, 0.2, 1.0),
        stroke_width: Some(2.0),
    });
    scene.insert(Primitive::Text {
        bounds: Bounds::from_xywh(0.0, 84.0, WIDTH, 16.0),
        content: "light on dark".into(),
        color: Rgba::WHITE,
        font_size: 11.0,
        font_weight: 700,
        font_family: None,
        line_height: 1.2,
        align: TextAlign::Center,
    });
    scene
}

#[test]
fn wgpu_draws_the_fixture_as_metal_does() {
    let Some((mut metal, mut wgpu)) = renderers() else {
        return;
    };
    let viewport = Size::new(WIDTH, HEIGHT);
    let scene = fixture_scene();
    let first = wgpu
        .capture_frame_pixels(&scene, viewport)
        .expect("wgpu capture");
    let colors: std::collections::HashSet<&[u8]> = first.chunks_exact(4).collect();
    assert!(colors.len() > 100, "the fixture draws something to compare");
    for scale in [1.0, 2.0] {
        // The second capture draws small paths from the atlas.
        for pass in ["first", "second"] {
            let expected = metal
                .capture_frame_pixels_at_scale(&scene, viewport, scale)
                .expect("metal capture");
            let actual = wgpu
                .capture_frame_pixels_at_scale(&scene, viewport, scale)
                .expect("wgpu capture");
            assert_close(
                &expected,
                &actual,
                (WIDTH * scale) as usize,
                &format!("fixture at {scale}x, {pass} frame"),
            );
        }
    }
    assert_eq!(
        metal.last_frame_stats().draw_calls,
        wgpu.last_frame_stats().draw_calls,
        "both backends encode the planned draws"
    );
}

#[test]
fn wgpu_draws_random_scenes_as_metal_does() {
    let Some((mut metal, mut wgpu)) = renderers() else {
        return;
    };
    let viewport = Size::new(WIDTH, HEIGHT);
    for seed in 0..48 {
        let scene = random_scene(seed);
        let scale = if seed % 3 == 0 { 2.0 } else { 1.0 };
        for _ in 0..2 {
            let expected = metal
                .capture_frame_pixels_at_scale(&scene, viewport, scale)
                .expect("metal capture");
            let actual = wgpu
                .capture_frame_pixels_at_scale(&scene, viewport, scale)
                .expect("wgpu capture");
            assert_close(
                &expected,
                &actual,
                (WIDTH * scale) as usize,
                &format!("seed {seed} at {scale}x"),
            );
        }
        let (metal_stats, wgpu_stats) = (metal.last_frame_stats(), wgpu.last_frame_stats());
        assert_eq!(
            (metal_stats.draw_calls, metal_stats.instances),
            (wgpu_stats.draw_calls, wgpu_stats.instances),
            "seed {seed}: both backends encode the planned draws"
        );
    }
}

/// Replays the frame `scene` holds in place and paints one quad over it, so a
/// retained target redraws only that quad's damage.
fn change_one_row(scene: &mut Scene) {
    let len = scene.len();
    scene.clear();
    scene
        .replay(0..len, 0.0, 0.0)
        .expect("the frame replays in place");
    scene.insert(Primitive::Quad {
        bounds: Bounds::from_xywh(30.0, 40.0, 70.0, 14.0),
        background: Rgba::new(0.9, 0.3, 0.2, 0.8),
        border_color: Rgba::TRANSPARENT,
        border_widths: Edges::ZERO,
        corner_radii: Corners::all(3.0),
    });
}

#[test]
fn wgpu_redraws_retained_damage_as_metal_does() {
    let Some((mut metal, mut wgpu)) = renderers() else {
        return;
    };
    let viewport = Size::new(WIDTH, HEIGHT);
    let scale = 2.0;
    let width = (WIDTH * scale) as usize;
    let mut metal_target = metal::RetainedTarget::new();
    let mut wgpu_target = RetainedTarget::new();
    let (mut metal_scene, mut wgpu_scene) = (fixture_scene(), fixture_scene());
    for frame in ["full frame", "damaged frame"] {
        if frame == "damaged frame" {
            change_one_row(&mut metal_scene);
            change_one_row(&mut wgpu_scene);
        }
        let expected = metal
            .capture_retained_pixels_at_scale(&metal_scene, &mut metal_target, viewport, scale)
            .expect("metal retained frame");
        let actual = wgpu
            .capture_retained_pixels_at_scale(&wgpu_scene, &mut wgpu_target, viewport, scale)
            .expect("wgpu retained frame");
        assert_close(&expected, &actual, width, &format!("retained {frame}"));
        assert_eq!(
            metal.last_frame_stats().redrawn_pixels,
            wgpu.last_frame_stats().redrawn_pixels,
            "{frame}: both redraw the same damage"
        );
    }
    let full = (width * (HEIGHT * scale) as usize) as u64;
    assert!(
        wgpu.last_frame_stats().redrawn_pixels < full / 4,
        "the damaged frame redrew only the damage"
    );
    // And the retained result is the frame a full redraw draws.
    let redrawn = wgpu
        .capture_frame_pixels_at_scale(&wgpu_scene, viewport, scale)
        .expect("wgpu full capture");
    let retained = wgpu
        .capture_retained_pixels_at_scale(&wgpu_scene, &mut RetainedTarget::new(), viewport, scale)
        .expect("wgpu retained capture");
    assert!(
        redrawn == retained,
        "a fresh retained target draws the full frame"
    );
}
