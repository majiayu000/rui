//! Oracle for draw planning: frames drawn by overlap order match the same frames
//! drawn in scene order, over seeded random scenes.

use super::MetalRenderer;
use crate::core::color::Rgba;
use crate::core::geometry::{Bounds, Edges, Size};
use crate::core::style::Corners;
use crate::elements::text::TextAlign;
use crate::renderer::text::BaseDirection;
use crate::renderer::{PathVertex, Primitive, RendererError, Scene};
use crate::{ImageFit, ImageSource};

pub(crate) const WIDTH: f32 = 160.0;
pub(crate) const HEIGHT: f32 = 120.0;

/// xorshift64*, enough to vary scenes without a dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    fn range(&mut self, low: f32, high: f32) -> f32 {
        low + self.unit() * (high - low)
    }

    fn color(&mut self) -> Rgba {
        let alpha = if self.below(2) == 0 { 1.0 } else { self.unit() };
        Rgba::new(self.unit(), self.unit(), self.unit(), alpha)
    }

    /// Bounds within the frame and a little past it, often on whole or half pixels.
    fn bounds(&mut self, max_size: f32) -> Bounds {
        let snap = |rng: &mut Self, value: f32| match rng.below(3) {
            0 => value.round(),
            1 => (value * 2.0).round() / 2.0,
            _ => value,
        };
        let x = self.range(-20.0, WIDTH);
        let x = snap(self, x);
        let y = self.range(-20.0, HEIGHT);
        let y = snap(self, y);
        let width = self.range(1.0, max_size);
        let width = snap(self, width);
        let height = self.range(1.0, max_size);
        let height = snap(self, height);
        Bounds::from_xywh(x, y, width, height)
    }
}

pub(crate) fn random_scene(seed: u64) -> Scene {
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
    let mut scene = Scene::new();
    let mut clips = 0;
    for _ in 0..120 {
        let primitive = match rng.below(20) {
            0..=6 => Primitive::Quad {
                bounds: rng.bounds(80.0),
                background: rng.color(),
                border_color: rng.color(),
                border_widths: Edges::all(rng.below(3) as f32),
                corner_radii: Corners::all(rng.range(0.0, 12.0)),
            },
            7 => Primitive::LinearGradient {
                bounds: rng.bounds(80.0),
                start: rng.color(),
                end: rng.color(),
                angle: rng.range(0.0, 360.0),
                border_color: rng.color(),
                border_widths: Edges::ZERO,
                corner_radii: Corners::all(rng.range(0.0, 12.0)),
            },
            8..=9 => Primitive::Shadow {
                bounds: rng.bounds(60.0),
                corner_radii: Corners::all(rng.range(0.0, 10.0)),
                blur_radius: rng.range(0.0, 10.0),
                color: rng.color(),
            },
            10..=13 => Primitive::Text {
                bounds: rng.bounds(90.0),
                content: ["row 12", "Wg@", "fix the check", "5 min", "ÅÉ ñ"][rng.below(5) as usize]
                    .into(),
                color: rng.color(),
                font_size: rng.range(8.0, 24.0),
                font_weight: 400,
                font_family: None,
                line_height: 1.2,
                align: [TextAlign::Left, TextAlign::Center, TextAlign::Right]
                    [rng.below(3) as usize],
                direction: BaseDirection::Auto,
            },
            14 => {
                let (width, height) = (1 + rng.below(6) as u32, 1 + rng.below(6) as u32);
                let data = (0..width * height * 4)
                    .map(|_| rng.below(256) as u8)
                    .collect();
                Primitive::Image {
                    bounds: rng.bounds(50.0),
                    source: ImageSource::Data {
                        data,
                        width,
                        height,
                    },
                    fit: [
                        ImageFit::Fill,
                        ImageFit::Contain,
                        ImageFit::Cover,
                        ImageFit::None,
                        ImageFit::ScaleDown,
                    ][rng.below(5) as usize],
                    corner_radii: Corners::all(rng.range(0.0, 8.0)),
                    opacity: rng.range(0.3, 1.0),
                }
            }
            15 => {
                let count = 3 + rng.below(4) as usize;
                let vertices = (0..count)
                    .map(|_| PathVertex::new(rng.range(-10.0, WIDTH), rng.range(-10.0, HEIGHT)))
                    .collect();
                Primitive::Path {
                    vertices,
                    offset: Default::default(),
                    color: rng.color(),
                    stroke_width: (rng.below(2) == 0).then(|| rng.range(1.0, 5.0)),
                }
            }
            16..=17 if clips < 3 => {
                clips += 1;
                Primitive::PushClip {
                    bounds: rng.bounds(120.0),
                    corner_radii: Corners::all(0.0),
                }
            }
            _ if clips > 0 => {
                clips -= 1;
                Primitive::PopClip
            }
            _ => continue,
        };
        scene.insert(primitive);
    }
    for _ in 0..clips {
        scene.insert(Primitive::PopClip);
    }
    scene
}

#[test]
fn overlap_order_draws_what_scene_order_draws() {
    let mut renderer = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    };
    let viewport = Size::new(WIDTH, HEIGHT);
    let (mut ordered_draws, mut scene_draws) = (0, 0);
    for seed in 0..48 {
        let scene = random_scene(seed);
        let scale = if seed % 3 == 0 { 2.0 } else { 1.0 };

        // Small paths drawn again come from the atlas; draw once so both
        // captures below draw them the same way.
        renderer
            .capture_frame_pixels_at_scale(&scene, viewport, scale)
            .expect("warm-up capture");
        renderer.set_scene_order(true);
        let expected = renderer
            .capture_frame_pixels_at_scale(&scene, viewport, scale)
            .expect("scene order capture");
        scene_draws += renderer.last_frame_stats().draw_calls;

        renderer.set_scene_order(false);
        let actual = renderer
            .capture_frame_pixels_at_scale(&scene, viewport, scale)
            .expect("overlap order capture");
        ordered_draws += renderer.last_frame_stats().draw_calls;

        if let Some(at) = expected.iter().zip(&actual).position(|(a, b)| a != b) {
            let width = (WIDTH * scale) as usize;
            let pixel = at / 4;
            panic!(
                "seed {seed}: pixel ({}, {}) differs: scene order {:?}, overlap order {:?}",
                pixel % width,
                pixel / width,
                &expected[pixel * 4..pixel * 4 + 4],
                &actual[pixel * 4..pixel * 4 + 4],
            );
        }
    }
    assert!(
        ordered_draws < scene_draws,
        "overlap order drew {ordered_draws} draws, scene order {scene_draws}"
    );
}

/// Coverage of each device pixel by the exact shape, from 16 x 16 samples: a
/// fill's nonzero winding, or a stroke's segment rectangles.
fn reference_coverage(
    points: &[(f32, f32)],
    stroke_width: Option<f32>,
    scale: f32,
    width: usize,
    height: usize,
) -> Vec<f32> {
    const SAMPLES: usize = 16;
    let points: Vec<(f32, f32)> = points
        .iter()
        .map(|&(x, y)| (x * scale, y * scale))
        .collect();
    let inside = |x: f32, y: f32| -> bool {
        match stroke_width {
            None => {
                let mut winding = 0;
                for (index, &(ax, ay)) in points.iter().enumerate() {
                    let (bx, by) = points[(index + 1) % points.len()];
                    let cross = (bx - ax) * (y - ay) - (by - ay) * (x - ax);
                    if ay <= y && by > y && cross > 0.0 {
                        winding += 1;
                    } else if ay > y && by <= y && cross < 0.0 {
                        winding -= 1;
                    }
                }
                winding != 0
            }
            Some(stroke) => {
                let half = stroke * scale * 0.5;
                let segment = points.windows(2).any(|pair| {
                    let ((ax, ay), (bx, by)) = (pair[0], pair[1]);
                    let length = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
                    let (ux, uy) = ((bx - ax) / length, (by - ay) / length);
                    let along = (x - ax) * ux + (y - ay) * uy;
                    let across = (y - ay) * ux - (x - ax) * uy;
                    (0.0..=length).contains(&along) && across.abs() <= half
                });
                // A miter join: within half the width of both segments' lines,
                // past both segments' ends at the vertex, on the outer side,
                // and within the miter limit of 4 widths from the vertex.
                let join = points.windows(3).any(|three| {
                    let unit = |(ax, ay): (f32, f32), (bx, by): (f32, f32)| {
                        let length = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
                        ((bx - ax) / length, (by - ay) / length)
                    };
                    let (corner, d0, d1) =
                        (three[1], unit(three[0], three[1]), unit(three[1], three[2]));
                    let (dx, dy) = (x - corner.0, y - corner.1);
                    let cos_angle = -(d0.0 * d1.0 + d0.1 * d1.1);
                    let miter = 1.0 / ((1.0 - cos_angle) * 0.5).sqrt();
                    let past_both = dx * d0.0 + dy * d0.1 >= 0.0 && dx * d1.0 + dy * d1.1 <= 0.0;
                    let near_both = (dy * d0.0 - dx * d0.1).abs() <= half
                        && (dy * d1.0 - dx * d1.1).abs() <= half;
                    miter <= 4.0 && past_both && near_both
                });
                segment || join
            }
        }
    };
    let mut coverage = Vec::with_capacity(width * height);
    for row in 0..height {
        for column in 0..width {
            let mut hits = 0;
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let x = column as f32 + (sx as f32 + 0.5) / SAMPLES as f32;
                    let y = row as f32 + (sy as f32 + 0.5) / SAMPLES as f32;
                    hits += usize::from(inside(x, y));
                }
            }
            coverage.push(hits as f32 / (SAMPLES * SAMPLES) as f32);
        }
    }
    coverage
}

fn srgb_byte_to_linear(byte: u8) -> f32 {
    let c = f32::from(byte) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[test]
fn paths_match_a_reference_raster_drawn_as_tiles_and_from_the_atlas() {
    // Coverage is compared in linear light: white over black blends to it.
    let mut renderer = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    };
    let star: Vec<(f32, f32)> = (0..10)
        .map(|point| {
            let angle = point as f32 * std::f32::consts::PI / 5.0;
            let radius = if point % 2 == 0 { 14.0 } else { 6.0 };
            (20.0 + angle.sin() * radius, 20.0 - angle.cos() * radius)
        })
        .collect();
    let pentagram: Vec<(f32, f32)> = (0..5)
        .map(|point| {
            let angle = (point * 2) as f32 * std::f32::consts::PI * 2.0 / 5.0;
            (20.3 + angle.sin() * 15.0, 20.6 - angle.cos() * 15.0)
        })
        .collect();
    // Name, points and stroke width of each path.
    type Case = (&'static str, Vec<(f32, f32)>, Option<f32>);
    let cases: Vec<Case> = vec![
        ("star", star, None),
        ("pentagram, nonzero", pentagram, None),
        (
            "notched arrow",
            vec![(3.0, 5.0), (36.0, 20.0), (3.0, 35.0), (14.5, 20.0)],
            None,
        ),
        (
            "zigzag stroke",
            vec![
                (3.0, 30.0),
                (12.0, 8.0),
                (21.0, 30.0),
                (30.0, 8.0),
                (37.0, 25.0),
            ],
            Some(3.0),
        ),
        ("hairline", vec![(2.0, 2.5), (38.0, 37.0)], Some(1.0)),
    ];
    let viewport = Size::new(40.0, 40.0);
    for (name, points, stroke_width) in cases {
        for scale in [1.0f32, 2.0] {
            let mut scene = Scene::new();
            scene.insert(Primitive::Quad {
                bounds: Bounds::from_xywh(0.0, 0.0, 40.0, 40.0),
                background: Rgba::new(0.0, 0.0, 0.0, 1.0),
                border_color: Rgba::new(0.0, 0.0, 0.0, 0.0),
                border_widths: Edges::ZERO,
                corner_radii: Corners::all(0.0),
            });
            scene.insert(Primitive::Path {
                vertices: points.iter().map(|&(x, y)| PathVertex::new(x, y)).collect(),
                offset: Default::default(),
                color: Rgba::new(1.0, 1.0, 1.0, 1.0),
                stroke_width,
            });
            let size = (40.0 * scale) as usize;
            let reference = reference_coverage(&points, stroke_width, scale, size, size);
            // The first frame draws tiles; the next rasterizes into the atlas,
            // and the one after draws the atlas sprite it placed.
            for (frame, placed) in [("tiles", 0), ("atlas", 1), ("atlas again", 1)] {
                let before = renderer.placed_path_sprites();
                let pixels = renderer
                    .capture_frame_pixels_at_scale(&scene, viewport, scale)
                    .expect("capture");
                if frame == "atlas" {
                    assert_eq!(renderer.placed_path_sprites() - before, placed, "{name}");
                }
                let (mut worst, mut total) = (0.0f32, 0.0f32);
                for (index, expected) in reference.iter().enumerate() {
                    let actual = srgb_byte_to_linear(pixels[index * 4]);
                    let error = (actual - expected).abs();
                    worst = worst.max(error);
                    total += error;
                }
                let mean = total / reference.len() as f32;
                // Pixels where the shape winds twice beside a sharp vertex count
                // the overlap twice before clamping, so the worst pixel is looser.
                assert!(
                    worst <= 0.25 && mean <= 0.002,
                    "{name} at scale {scale}, {frame}: worst error {worst:.3}, mean {mean:.4}"
                );
            }
        }
    }
}

#[test]
fn a_kept_offscreen_texture_draws_smaller_frames_as_an_exact_one_does() {
    use metal::foreign_types::ForeignType;

    let mut renderer = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    };
    let large = random_scene(7);
    renderer
        .render_offscreen_gpu_time(&large, Size::new(WIDTH, HEIGHT), 2.0)
        .expect("large frame");
    let kept = renderer.offscreen_target().expect("kept texture");

    // A smaller frame, at another aspect, in the same texture.
    let small = Size::new(WIDTH * 0.6, HEIGHT * 0.8);
    let scene = random_scene(8);
    renderer
        .render_offscreen_gpu_time(&scene, small, 2.0)
        .expect("small frame");
    let texture = renderer.offscreen_target().expect("kept texture");
    assert_eq!(texture.as_ptr(), kept.as_ptr(), "the texture is reused");
    let (width, height) = ((small.width * 2.0) as u64, (small.height * 2.0) as u64);
    let drawn = super::capture::read_texture_rgba(&texture, width, height);
    let expected = renderer
        .capture_frame_pixels_at_scale(&scene, small, 2.0)
        .expect("exact capture");
    assert!(drawn == expected, "the kept texture drew different pixels");
}

#[test]
fn a_retained_frame_copied_into_a_kept_larger_texture_matches_an_exact_capture() {
    use super::RetainedTarget;

    let mut renderer = match MetalRenderer::new() {
        Ok(renderer) => renderer,
        Err(RendererError::BackendUnavailable(_)) => return,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    };
    renderer
        .render_offscreen_gpu_time(&random_scene(9), Size::new(WIDTH, HEIGHT), 2.0)
        .expect("large frame");

    let small = Size::new(WIDTH * 0.7, HEIGHT * 0.5);
    let scene = random_scene(10);
    let mut target = RetainedTarget::default();
    renderer
        .render_retained_offscreen_gpu_time(&scene, &mut target, small, 2.0)
        .expect("retained frame");
    let texture = renderer.offscreen_target().expect("kept texture");
    assert!(
        texture.width() > (small.width * 2.0) as u64,
        "the kept texture is larger"
    );
    let (width, height) = ((small.width * 2.0) as u64, (small.height * 2.0) as u64);
    let drawn = super::capture::read_texture_rgba(&texture, width, height);
    let expected = renderer
        .capture_frame_pixels_at_scale(&scene, small, 2.0)
        .expect("exact capture");
    assert!(drawn == expected, "the retained copy drew different pixels");
}

#[test]
fn kept_paths_drawn_from_the_arena_match_a_fresh_renderer() {
    use std::sync::Arc;
    let new_renderer = || match MetalRenderer::new() {
        Ok(renderer) => Some(renderer),
        Err(RendererError::BackendUnavailable(_)) => None,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    };
    let Some(mut renderer) = new_renderer() else {
        return;
    };
    // Stars too large for the atlas, so they are drawn as tiles.
    let star = |center: f32, radius: f32| -> Arc<[PathVertex]> {
        (0..10)
            .map(|point| {
                let angle = point as f32 * std::f32::consts::PI / 5.0;
                let radius = if point % 2 == 0 { radius } else { radius * 0.4 };
                PathVertex::new(center + angle.sin() * radius, center - angle.cos() * radius)
            })
            .collect()
    };
    let (small, large) = (star(90.0, 80.0), star(150.0, 140.0));
    let red = Rgba::new(1.0, 0.2, 0.1, 0.8);
    let blue = Rgba::new(0.1, 0.3, 1.0, 0.7);
    // Kept, then drawn from the arena, in another color, with the large one
    // dropped so the arena is packed, and both again.
    let mut frames = vec![vec![(&small, red), (&large, blue)]; 6];
    frames.extend(vec![vec![(&small, blue), (&large, blue)]; 2]);
    frames.extend(vec![vec![(&small, red)]; 5]);
    frames.extend(vec![vec![(&small, red), (&large, blue)]; 5]);
    let viewport = Size::new(300.0, 300.0);
    for (index, paths) in frames.iter().enumerate() {
        let mut scene = Scene::new();
        for (vertices, color) in paths {
            scene.insert(Primitive::Path {
                vertices: Arc::clone(vertices),
                offset: Default::default(),
                color: *color,
                stroke_width: None,
            });
        }
        let pixels = renderer
            .capture_frame_pixels_at_scale(&scene, viewport, 1.0)
            .expect("capture");
        let expected = new_renderer()
            .expect("available")
            .capture_frame_pixels_at_scale(&scene, viewport, 1.0)
            .expect("capture");
        assert!(pixels == expected, "frame {index} differs from a fresh one");
    }
}
