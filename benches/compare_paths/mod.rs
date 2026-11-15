//! The vector cases of `compare_frame`: `paths` and `paths_icons`, drawn from
//! `benchmarks/paths_scene.rs` as egui's and Vello's harnesses draw them.
//!
//! `paths` is one `canvas` painting `Primitive::Path`s, curves flattened to
//! [`TOLERANCE`] once and the vertices kept, and the circles as round quads. `paths_icons` is 300 `svg`
//! elements of 12 designs. `COMPARE_PNG=<dir>` also saves frame 0 of each case as
//! `<dir>/rui_<case>.png`, to compare with the other sides.

#[path = "../../benchmarks/paths_scene.rs"]
mod paths_scene;

use super::{Harness, SCALE_FACTOR};
use paths_scene::{Outline, Shape};
use rui::core::color::Rgba;
use rui::core::frame_pipeline::FramePipeline;
use rui::core::geometry::Edges;
use rui::core::style::Corners;
use rui::elements::element::Element;
use rui::elements::svg;
use rui::renderer::{PathVertex, Primitive};
use rui::{Bounds, Color, Point, Size, canvas, div};
use std::rc::Rc;
use std::sync::Arc;

/// Curves are flattened to within this many logical pixels: a quarter of a
/// device pixel at scale 2.
const TOLERANCE: f32 = 0.125;
const BACKGROUND: u32 = 0x101418;
/// How far a channel of a frame drawn from kept paths may be from the first.
const MOST_KEPT_DIFFERENCE: u8 = 4;

fn rgba(color: [u8; 4]) -> Rgba {
    Rgba::from_u8(color[0], color[1], color[2], color[3])
}

/// A shape ready to paint: a path's vertices flattened once and kept, as an app
/// keeps the geometry it draws, or a circle.
pub(super) enum Kept {
    Path {
        vertices: Arc<[PathVertex]>,
        color: Rgba,
        stroke_width: Option<f32>,
    },
    Circle {
        center: [f32; 2],
        radius: f32,
        color: Rgba,
    },
}

impl Kept {
    fn new(shape: &Shape, points: &mut Vec<[f32; 2]>) -> Self {
        let mut flatten = |outline: &Outline| -> Arc<[PathVertex]> {
            outline.flatten(TOLERANCE, points);
            points.iter().map(|p| PathVertex::new(p[0], p[1])).collect()
        };
        match shape {
            Shape::Fill { outline, color } => Kept::Path {
                vertices: flatten(outline),
                color: rgba(*color),
                stroke_width: None,
            },
            Shape::Stroke {
                outline,
                width,
                color,
            } => Kept::Path {
                vertices: flatten(outline),
                color: rgba(*color),
                stroke_width: Some(*width),
            },
            Shape::Circle {
                center,
                radius,
                color,
            } => Kept::Circle {
                center: *center,
                radius: *radius,
                color: rgba(*color),
            },
        }
    }

    /// The shape painted with its origin at `at`.
    fn paint(&self, at: [f32; 2], painter: &mut rui::Painter) {
        let primitive = match self {
            Kept::Path {
                vertices,
                color,
                stroke_width,
            } => Primitive::Path {
                vertices: Arc::clone(vertices),
                offset: Point::new(at[0], at[1]),
                color: *color,
                stroke_width: *stroke_width,
            },
            Kept::Circle {
                center,
                radius,
                color,
            } => Primitive::Quad {
                bounds: Bounds::from_xywh(
                    at[0] + center[0] - radius,
                    at[1] + center[1] - radius,
                    radius * 2.0,
                    radius * 2.0,
                ),
                background: *color,
                border_color: Rgba::TRANSPARENT,
                border_widths: Edges::ZERO,
                corner_radii: Corners::all(*radius),
            },
        };
        painter.paint(primitive);
    }
}

/// The `paths` scene at frame `turn`, as one canvas painting every shape each
/// frame from the kept geometry; only the turning shape is flattened again.
pub(super) fn paths(
    shapes: &Rc<Vec<Shape>>,
    kept: &Rc<Vec<Kept>>,
    turn: usize,
) -> impl Element + use<> {
    let kept = Rc::clone(kept);
    let moving = Kept::new(&paths_scene::moving(shapes, turn), &mut Vec::new());
    canvas(move |bounds, painter| {
        let at = [bounds.x(), bounds.y()];
        for (index, shape) in kept.iter().enumerate() {
            let shape = if index == paths_scene::MOVING {
                &moving
            } else {
                shape
            };
            shape.paint(at, painter);
        }
    })
    .w_full()
    .h_full()
}

/// The icons scene at frame `turn`: 15 rows of 20 cells, each an `svg`.
pub(super) fn icons(markup: &Rc<Vec<String>>, turn: usize) -> impl Element + use<> {
    let (columns, rows) = paths_scene::ICON_GRID;
    let (cell_w, cell_h) = paths_scene::ICON_CELL;
    let size = paths_scene::ICON_SIZE;
    div().w_full().flex_col().children((0..rows).map(|row| {
        div()
            .flex_row()
            .h(cell_h)
            .children((0..columns).map(|column| {
                let index = row * columns + column;
                let (design, [x, y]) = paths_scene::icon_place(index, turn);
                // The cell's padding places the icon where `icon_place` says.
                div()
                    .w(cell_w)
                    .h(cell_h)
                    .pl(x - column as f32 * cell_w)
                    .pt(y - row as f32 * cell_h)
                    .child(svg(&markup[design]).size(Size::new(size, size)))
            }))
    }))
}

pub(super) fn scene() -> Rc<Vec<Shape>> {
    Rc::new(paths_scene::paths())
}

/// Every shape of `shapes` flattened once.
pub(super) fn kept(shapes: &[Shape]) -> Rc<Vec<Kept>> {
    let mut points = Vec::new();
    Rc::new(
        shapes
            .iter()
            .map(|shape| Kept::new(shape, &mut points))
            .collect(),
    )
}

pub(super) fn icon_markup() -> Rc<Vec<String>> {
    Rc::new(
        paths_scene::icon_designs()
            .iter()
            .map(|design| paths_scene::icon_svg(design))
            .collect(),
    )
}

/// The background every side clears to.
pub(super) fn backdrop(content: impl Element + 'static) -> impl Element {
    div()
        .w_full()
        .h_full()
        .bg(Color::hex(BACKGROUND))
        .child(content)
}

/// Saves frame 0 of `case` when `COMPARE_PNG` names a directory. Frame 0 is
/// drawn three times, the later ones from what the renderer kept, and they
/// must match the first to within [`MOST_KEPT_DIFFERENCE`].
pub(super) fn save_png<E: Element>(
    case: &str,
    harness: &mut Harness,
    root: impl Fn() -> E,
    viewport: Size,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(dir) = std::env::var_os("COMPARE_PNG") else {
        return Ok(());
    };
    let mut first: Option<Vec<u8>> = None;
    for _ in 0..3 {
        let mut root = root();
        let bounds = FramePipeline::layout_root_retained(
            &mut root,
            &mut harness.taffy,
            &mut harness.nodes,
            &mut harness.measurer,
            viewport,
        )?;
        FramePipeline::paint_root_in_viewport(
            &mut root,
            &harness.taffy,
            &mut harness.scene,
            bounds,
            viewport,
        );
        let pixels = harness.renderer.capture_frame_pixels_at_scale(
            &harness.scene,
            viewport,
            SCALE_FACTOR,
        )?;
        // Small paths drawn again come from the atlas, rasterized on the CPU to
        // 8-bit coverage, so they may differ from the first frame's analytic
        // coverage by a step or two.
        let Some(first) = &first else {
            first = Some(pixels);
            continue;
        };
        let most = first
            .iter()
            .zip(&pixels)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        if most > MOST_KEPT_DIFFERENCE {
            let message = format!("{case}: a frame drawn again differs from the first by {most}");
            return Err(message.into());
        }
    }
    let pixels = first.expect("drawn");
    let (width, height) = (
        (viewport.width * SCALE_FACTOR) as u32,
        (viewport.height * SCALE_FACTOR) as u32,
    );
    let path = std::path::Path::new(&dir).join(format!("rui_{case}.png"));
    image::save_buffer(&path, &pixels, width, height, image::ColorType::Rgba8)?;
    eprintln!("saved {}", path.display());
    Ok(())
}
