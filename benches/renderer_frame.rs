//! What the Metal renderer asks of the GPU for a frame, by scene. Prints JSON.
//!
//! `cargo bench --bench renderer_frame`
//!
//! Scenes, each in a 1440x900 window at scale 2:
//! - `list`: the 2000-row list of `compare_frame`, laid out and painted once.
//! - `text_grid`: 3000 short strings in a 60x50 grid.
//! - `cards`: 96 cards, each a shadow, a rounded bordered quad, a title and a caption.
//! - `icons_paths`: 240 rows of a stroked check icon, a filled star icon and a label in
//!   four columns, over a 360-point stroked chart line and two large concave fills.
//!
//! For each: draw calls and instances of one frame, the median instructions and
//! milliseconds of encoding and submitting it (`render_offscreen`, which does not
//! wait for the GPU), and a checksum of its pixels. A renderer change that keeps the
//! checksums drew the same frames. Instructions are the whole process's, driver
//! threads included, so the queue is drained between samples, outside the count.
//! `gpu_ms` is the median GPU time of the frame over renders that wait for it.
use rui::core::color::Rgba;
use rui::core::frame_pipeline::FramePipeline;
use rui::core::geometry::{Bounds, Edges};
use rui::core::style::Corners;
use rui::elements::element::LayoutNodes;
use rui::elements::text::TextAlign;
use rui::renderer::metal::MetalRenderer;
use rui::renderer::text::TextMeasureCache;
use rui::renderer::{PathVertex, Primitive, Scene};
use rui::{Color, ElementId, Size, div, text};
use std::time::Instant;
use taffy::prelude::TaffyTree;

const VIEWPORT: (f32, f32) = (1440.0, 900.0);
const SCALE_FACTOR: f32 = 2.0;
const WARMUP: usize = 5;
const SAMPLES: usize = 30;

unsafe extern "C" {
    fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut u64) -> i32;
}

/// Retired instructions of the whole process (`rusage_info_v4` word 31).
fn instructions() -> u64 {
    let mut words = [0u64; 64];
    // SAFETY: flavor 4 writes a rusage_info_v4, far shorter than 64 words.
    let status = unsafe { proc_pid_rusage(std::process::id() as i32, 4, words.as_mut_ptr()) };
    assert_eq!(status, 0, "proc_pid_rusage failed");
    words[31]
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

fn list_scene() -> Result<Scene, Box<dyn std::error::Error>> {
    let mut root = div().w_full().flex_col().children((0..2_000).map(|index| {
        let background = if index == 0 {
            Color::hex(0x2d3748)
        } else {
            Color::hex(0x1a202c)
        };
        div()
            .w_full()
            .h(28.0)
            .flex_row()
            .justify_between()
            .bg(background)
            .child(text(format!("task {index}: fix the failing check")))
            .child(text(format!("{} min", index % 60)))
    }));
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut nodes = LayoutNodes::new();
    let mut measurer = TextMeasureCache::new();
    let mut scene = Scene::new();
    let bounds = FramePipeline::layout_root_retained(
        &mut root,
        &mut taffy,
        &mut nodes,
        &mut measurer,
        Size::new(VIEWPORT.0, VIEWPORT.1),
    )?;
    FramePipeline::paint_root(&mut root, &taffy, &mut scene, bounds);
    Ok(scene)
}

fn label(bounds: Bounds, content: String, font_size: f32, color: Rgba) -> Primitive {
    Primitive::Text {
        bounds,
        content,
        color,
        font_size,
        font_weight: 400,
        font_family: None,
        line_height: 1.2,
        align: TextAlign::Left,
    }
}

fn text_grid_scene() -> Scene {
    let mut scene = Scene::new();
    let (columns, rows) = (60, 50);
    let (width, height) = (VIEWPORT.0 / columns as f32, VIEWPORT.1 / rows as f32);
    for index in 0..columns * rows {
        let (column, row) = (index % columns, index / columns);
        scene.insert(label(
            Bounds::from_xywh(column as f32 * width, row as f32 * height, width, height),
            format!("{index}"),
            10.0,
            Rgba::new(0.85, 0.87, 0.9, 1.0),
        ));
    }
    scene
}

fn cards_scene() -> Scene {
    let mut scene = Scene::new();
    let (columns, rows) = (12, 8);
    let (width, height) = (VIEWPORT.0 / columns as f32, VIEWPORT.1 / rows as f32);
    for index in 0..columns * rows {
        let (column, row) = (index % columns, index / columns);
        let card = Bounds::from_xywh(
            column as f32 * width + 12.0,
            row as f32 * height + 12.0,
            width - 24.0,
            height - 24.0,
        );
        scene.insert(Primitive::Shadow {
            bounds: card,
            corner_radii: Corners::all(8.0),
            blur_radius: 6.0,
            color: Rgba::new(0.0, 0.0, 0.0, 0.35),
        });
        scene.insert(Primitive::Quad {
            bounds: card,
            background: Rgba::new(0.16, 0.18, 0.22, 1.0),
            border_color: Rgba::new(0.3, 0.34, 0.4, 1.0),
            border_widths: Edges::all(1.0),
            corner_radii: Corners::all(8.0),
        });
        let line = |offset: f32| {
            Bounds::from_xywh(
                card.x() + 10.0,
                card.y() + offset,
                card.width() - 20.0,
                18.0,
            )
        };
        scene.insert(label(
            line(10.0),
            format!("Card {index}"),
            14.0,
            Rgba::new(0.95, 0.95, 0.97, 1.0),
        ));
        scene.insert(label(
            line(34.0),
            format!("{} items", index * 7 % 50),
            11.0,
            Rgba::new(0.6, 0.64, 0.7, 1.0),
        ));
    }
    scene
}

fn icons_paths_scene() -> Scene {
    let mut scene = Scene::new();
    // Two large concave fills behind everything: an arrow and a comb.
    let arrow = [
        (100.0, 300.0),
        (500.0, 300.0),
        (500.0, 150.0),
        (800.0, 450.0),
        (500.0, 750.0),
        (500.0, 600.0),
        (100.0, 600.0),
    ];
    let comb: Vec<(f32, f32)> = (0..10)
        .flat_map(|tooth| {
            let x = 900.0 + tooth as f32 * 50.0;
            [
                (x, 100.0),
                (x + 25.0, 100.0),
                (x + 25.0, 700.0),
                (x + 50.0, 700.0),
            ]
        })
        .chain([(1400.0, 800.0), (900.0, 800.0)])
        .collect();
    for (points, color) in [
        (arrow.to_vec(), Rgba::new(0.2, 0.3, 0.5, 1.0)),
        (comb, Rgba::new(0.3, 0.2, 0.4, 0.8)),
    ] {
        scene.insert(Primitive::Path {
            vertices: points.iter().map(|&(x, y)| PathVertex::new(x, y)).collect(),
            color,
            stroke_width: None,
        });
    }
    let chart = (0..360)
        .map(|step| {
            let x = step as f32 * 4.0;
            PathVertex::new(x, 450.0 + (step as f32 * 0.21).sin() * 300.0)
        })
        .collect();
    scene.insert(Primitive::Path {
        vertices: chart,
        color: Rgba::new(0.9, 0.6, 0.2, 1.0),
        stroke_width: Some(2.0),
    });

    let (columns, rows) = (4, 60);
    let (width, height) = (VIEWPORT.0 / columns as f32, VIEWPORT.1 / rows as f32);
    for index in 0..columns * rows {
        let (x, y) = (
            (index % columns) as f32 * width,
            (index / columns) as f32 * height,
        );
        let check = [(2.0, 8.0), (6.0, 12.0), (13.0, 3.0)];
        scene.insert(Primitive::Path {
            vertices: check
                .iter()
                .map(|&(dx, dy)| PathVertex::new(x + dx, y + dy))
                .collect(),
            color: Rgba::new(0.3, 0.8, 0.4, 1.0),
            stroke_width: Some(2.0),
        });
        let star = (0..10)
            .map(|point| {
                let angle = point as f32 * std::f32::consts::PI / 5.0;
                let radius = if point % 2 == 0 { 7.0 } else { 3.0 };
                PathVertex::new(
                    x + 24.0 + angle.sin() * radius,
                    y + 7.5 - angle.cos() * radius,
                )
            })
            .collect();
        scene.insert(Primitive::Path {
            vertices: star,
            color: Rgba::new(0.95, 0.8, 0.2, 1.0),
            stroke_width: None,
        });
        scene.insert(label(
            Bounds::from_xywh(x + 36.0, y, width - 40.0, height),
            format!("item {index}"),
            10.0,
            Rgba::new(0.85, 0.87, 0.9, 1.0),
        ));
    }
    scene
}

fn checksum(pixels: &[u8]) -> u64 {
    pixels.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let viewport = Size::new(VIEWPORT.0, VIEWPORT.1);
    let scenes = [
        ("list", list_scene()?),
        ("text_grid", text_grid_scene()),
        ("cards", cards_scene()),
        ("icons_paths", icons_paths_scene()),
    ];
    let mut renderer = MetalRenderer::new()?;
    // Rendering a capture waits for it, and so for every frame queued before it.
    let empty = Scene::new();
    let drain = |renderer: &mut MetalRenderer| {
        renderer
            .capture_frame_pixels(&empty, Size::new(1.0, 1.0))
            .map(|_| ())
    };

    println!("{{");
    for (position, (name, scene)) in scenes.iter().enumerate() {
        let pixels = renderer.capture_frame_pixels_at_scale(scene, viewport, SCALE_FACTOR)?;
        let stats = renderer.last_frame_stats();
        for _ in 0..WARMUP {
            renderer.render_offscreen(scene, viewport, SCALE_FACTOR)?;
        }
        let (mut ms, mut counts) = (Vec::new(), Vec::new());
        for _ in 0..SAMPLES {
            drain(&mut renderer)?;
            let (i0, t0) = (instructions(), Instant::now());
            renderer.render_offscreen(scene, viewport, SCALE_FACTOR)?;
            ms.push(t0.elapsed().as_secs_f64() * 1e3);
            counts.push(instructions().saturating_sub(i0) as f64 / 1e6);
        }
        let mut gpu_ms = Vec::new();
        for _ in 0..SAMPLES / 3 {
            let gpu = renderer.render_offscreen_gpu_time(scene, viewport, SCALE_FACTOR)?;
            gpu_ms.push(gpu.as_secs_f64() * 1e3);
        }
        println!(
            "  \"{name}\": {{\"primitives\": {}, \"draw_calls\": {}, \"instances\": {}, \"instructions_m\": {:.2}, \"ms\": {:.3}, \"gpu_ms\": {:.3}, \"pixel_checksum\": \"{:016x}\"}}{}",
            scene.len(),
            stats.draw_calls,
            stats.instances,
            median(counts),
            median(ms),
            median(gpu_ms),
            checksum(&pixels),
            if position + 1 == scenes.len() {
                ""
            } else {
                ","
            }
        );
    }
    println!("}}");
    Ok(())
}
