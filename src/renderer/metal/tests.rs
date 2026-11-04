//! Oracle for draw planning: frames drawn by overlap order match the same frames
//! drawn in scene order, over seeded random scenes.

use super::MetalRenderer;
use crate::core::color::Rgba;
use crate::core::geometry::{Bounds, Edges, Size};
use crate::core::style::Corners;
use crate::elements::text::TextAlign;
use crate::renderer::{PathVertex, Primitive, RendererError, Scene};
use crate::{ImageFit, ImageSource};

const WIDTH: f32 = 160.0;
const HEIGHT: f32 = 120.0;

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

fn random_scene(seed: u64) -> Scene {
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
                    .to_string(),
                color: rng.color(),
                font_size: rng.range(8.0, 24.0),
                font_weight: 400,
                font_family: None,
                line_height: 1.2,
                align: [TextAlign::Left, TextAlign::Center, TextAlign::Right]
                    [rng.below(3) as usize],
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
                    ][rng.below(4) as usize],
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
