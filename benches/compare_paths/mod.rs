//! The vector cases of `compare_frame`: `paths` and `paths_icons`, drawn from
//! `benchmarks/paths_scene.rs` as egui's and Vello's harnesses draw them.
//!
//! `paths` is one `canvas` painting `Primitive::Path`s, curves flattened to
//! [`TOLERANCE`], and the circles as round quads. `paths_icons` is 300 `svg`
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
use rui::{Bounds, Color, Size, canvas, div};
use std::rc::Rc;

/// Curves are flattened to within this many logical pixels: a quarter of a
/// device pixel at scale 2.
const TOLERANCE: f32 = 0.125;
const BACKGROUND: u32 = 0x101418;

fn rgba(color: [u8; 4]) -> Rgba {
    Rgba::from_u8(color[0], color[1], color[2], color[3])
}

fn vertices(outline: &Outline, points: &mut Vec<[f32; 2]>, at: [f32; 2]) -> Vec<PathVertex> {
    outline.flatten(TOLERANCE, points);
    points
        .iter()
        .map(|p| PathVertex::new(at[0] + p[0], at[1] + p[1]))
        .collect()
}

fn paint_shape(shape: &Shape, at: [f32; 2], points: &mut Vec<[f32; 2]>, scene: &mut rui::Painter) {
    let primitive = match shape {
        Shape::Fill { outline, color } => Primitive::Path {
            vertices: vertices(outline, points, at),
            color: rgba(*color),
            stroke_width: None,
        },
        Shape::Stroke {
            outline,
            width,
            color,
        } => Primitive::Path {
            vertices: vertices(outline, points, at),
            color: rgba(*color),
            stroke_width: Some(*width),
        },
        Shape::Circle {
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
            background: rgba(*color),
            border_color: Rgba::TRANSPARENT,
            border_widths: Edges::ZERO,
            corner_radii: Corners::all(*radius),
        },
    };
    scene.paint(primitive);
}

/// The `paths` scene at frame `turn`, as one canvas.
pub(super) fn paths(shapes: &Rc<Vec<Shape>>, turn: usize) -> impl Element + use<> {
    let shapes = Rc::clone(shapes);
    let moving = paths_scene::moving(&shapes, turn);
    canvas(move |bounds, painter| {
        let at = [bounds.x(), bounds.y()];
        let mut points = Vec::new();
        for (index, shape) in shapes.iter().enumerate() {
            let shape = if index == paths_scene::MOVING {
                &moving
            } else {
                shape
            };
            paint_shape(shape, at, &mut points, painter);
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

/// Saves frame 0 of `case` when `COMPARE_PNG` names a directory.
pub(super) fn save_png(
    case: &str,
    harness: &mut Harness,
    mut root: impl Element,
    viewport: Size,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(dir) = std::env::var_os("COMPARE_PNG") else {
        return Ok(());
    };
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
    let pixels =
        harness
            .renderer
            .capture_frame_pixels_at_scale(&harness.scene, viewport, SCALE_FACTOR)?;
    let (width, height) = (
        (viewport.width * SCALE_FACTOR) as u32,
        (viewport.height * SCALE_FACTOR) as u32,
    );
    let path = std::path::Path::new(&dir).join(format!("rui_{case}.png"));
    image::save_buffer(&path, &pixels, width, height, image::ColorType::Rgba8)?;
    eprintln!("saved {}", path.display());
    Ok(())
}
