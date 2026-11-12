//! The vector cases, `paths` and `paths_icons`: the scenes of
//! `benchmarks/paths_scene.rs` painted with `egui::Painter`. Curves are flattened
//! by `epaint`'s `CubicBezierShape::flatten` and filled as `PathShape`s, strokes
//! are `PathShape::line`s, circles `circle_filled`. epaint fills only convex
//! polygons, so the stars and pentagrams are filled as it fills them.

#[path = "../../paths_scene.rs"]
mod paths_scene;

use egui::epaint::{CubicBezierShape, PathShape, PathStroke};
use egui::{Color32, Pos2, Shape, Stroke, Vec2};
use paths_scene::{Outline, Segment};

/// Curves are flattened to within this many points, as on RUI's side.
const TOLERANCE: f32 = 0.125;
pub const BACKGROUND: u32 = 0x101418;

fn color(rgba: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(rgba[0], rgba[1], rgba[2], rgba[3])
}

fn points(outline: &Outline, scale: f32, at: Vec2) -> Vec<Pos2> {
    let pos = |p: [f32; 2]| Pos2::new(p[0] * scale, p[1] * scale) + at;
    let mut points = vec![pos(outline.start)];
    let mut from = points[0];
    for segment in &outline.segments {
        match *segment {
            Segment::Line(p) => {
                from = pos(p);
                points.push(from);
            }
            Segment::Cubic(a, b, p) => {
                let curve = CubicBezierShape::from_points_stroke(
                    [from, pos(a), pos(b), pos(p)],
                    false,
                    Color32::TRANSPARENT,
                    Stroke::NONE,
                );
                points.extend(curve.flatten(Some(TOLERANCE)).into_iter().skip(1));
                from = pos(p);
            }
        }
    }
    if outline.closed && points.len() > 1 && points[0] == points[points.len() - 1] {
        points.pop();
    }
    points
}

/// `shape`, scaled by `scale` and moved by `at`, as egui shapes.
fn shape(shape: &paths_scene::Shape, scale: f32, at: Vec2) -> Shape {
    match shape {
        paths_scene::Shape::Fill { outline, color: c } => Shape::Path(PathShape {
            points: points(outline, scale, at),
            closed: true,
            fill: color(*c),
            stroke: PathStroke::NONE,
        }),
        paths_scene::Shape::Stroke {
            outline,
            width,
            color: c,
        } => Shape::Path(PathShape::line(
            points(outline, scale, at),
            Stroke::new(width * scale, color(*c)),
        )),
        paths_scene::Shape::Circle {
            center,
            radius,
            color: c,
        } => Shape::circle_filled(
            Pos2::new(center[0] * scale, center[1] * scale) + at,
            radius * scale,
            color(*c),
        ),
    }
}

pub struct Paths(Vec<paths_scene::Shape>);

impl Paths {
    pub fn new() -> Self {
        Self(paths_scene::paths())
    }

    pub fn paint(&self, ui: &mut egui::Ui, turn: usize) {
        let moving = paths_scene::moving(&self.0, turn);
        let painter = ui.painter();
        painter.rect_filled(ui.max_rect().expand(16.0), 0.0, crate::hex(BACKGROUND));
        painter.extend(self.0.iter().enumerate().map(|(index, kept)| {
            let kept = if index == paths_scene::MOVING {
                &moving
            } else {
                kept
            };
            shape(kept, 1.0, Vec2::ZERO)
        }));
    }
}

pub struct Icons(Vec<Vec<paths_scene::Shape>>);

impl Icons {
    pub fn new() -> Self {
        Self(paths_scene::icon_designs())
    }

    pub fn paint(&self, ui: &mut egui::Ui, turn: usize) {
        let painter = ui.painter();
        painter.rect_filled(ui.max_rect().expand(16.0), 0.0, crate::hex(BACKGROUND));
        let scale = paths_scene::ICON_SIZE / paths_scene::ICON_BOX;
        let count = paths_scene::ICON_GRID.0 * paths_scene::ICON_GRID.1;
        for index in 0..count {
            let (design, [x, y]) = paths_scene::icon_place(index, turn);
            let at = Vec2::new(x, y);
            painter.extend(self.0[design].iter().map(|kept| shape(kept, scale, at)));
        }
    }
}

/// Saves the last frame as `<dir>/egui_<case>.png` when `COMPARE_PNG` names a
/// directory. The target is BGRA; the PNG is RGBA.
pub fn save_png(harness: &crate::Harness, case: &str) -> Result<(), Box<dyn std::error::Error>> {
    use egui_wgpu::wgpu;
    let Some(dir) = std::env::var_os("COMPARE_PNG") else {
        return Ok(());
    };
    let size = harness.texture.size();
    let row = size.width * 4;
    let buffer = harness.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("egui_compare readback"),
        size: u64::from(row * size.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = harness.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        harness.texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: None,
            },
        },
        size,
    );
    harness.queue.submit([encoder.finish()]);
    buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    harness.device.poll(wgpu::PollType::wait_indefinitely())?;
    let mut pixels = buffer.slice(..).get_mapped_range()?.to_vec();
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let path = std::path::Path::new(&dir).join(format!("egui_{case}.png"));
    image::save_buffer(
        &path,
        &pixels,
        size.width,
        size.height,
        image::ColorType::Rgba8,
    )?;
    eprintln!("saved {}", path.display());
    Ok(())
}
