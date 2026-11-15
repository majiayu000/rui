//! The grid cases of `compare_frame`: `quad_grid`, one canvas painted again each
//! frame, and `quad_grid_cached`, the same grid in a canvas kept by its cache key
//! under the changed cell.

use rui::elements::canvas;
use rui::elements::element::Element;
use rui::{Bounds, Color, div};

pub(super) const GRID: (usize, usize) = (200, 60);
pub(super) const CELL: (f32, f32) = (7.0, 15.0);

fn cell_color(row: usize, column: usize) -> u32 {
    (((column * 255 / GRID.0) as u32) << 16) | (((row * 255 / GRID.1) as u32) << 8) | 0x60
}

/// A 200x60 grid of 7x15 cells painted by one canvas; cell `changed` is white.
pub(super) fn quad_grid(changed: usize) -> impl Element {
    canvas(move |bounds, painter| {
        for row in 0..GRID.1 {
            for column in 0..GRID.0 {
                let cell = Bounds::from_xywh(
                    bounds.x() + column as f32 * CELL.0,
                    bounds.y() + row as f32 * CELL.1,
                    CELL.0,
                    CELL.1,
                );
                let color = if row * GRID.0 + column == changed {
                    0xffffff
                } else {
                    cell_color(row, column)
                };
                painter.fill(cell, Color::hex(color));
            }
        }
    })
    .w(GRID.0 as f32 * CELL.0)
    .h(GRID.1 as f32 * CELL.1)
}

/// The grid of `quad_grid` with no cell changed, for a view that never renders again,
/// kept by its canvas as iced's `canvas::Cache` keeps it.
pub(super) fn static_grid() -> impl Element {
    canvas(move |bounds, painter| {
        for row in 0..GRID.1 {
            for column in 0..GRID.0 {
                let cell = Bounds::from_xywh(
                    bounds.x() + column as f32 * CELL.0,
                    bounds.y() + row as f32 * CELL.1,
                    CELL.0,
                    CELL.1,
                );
                painter.fill(cell, Color::hex(cell_color(row, column)));
            }
        }
    })
    .cache_key(())
    .w(GRID.0 as f32 * CELL.0)
    .h(GRID.1 as f32 * CELL.1)
}

/// Cell `changed` of the grid, white, placed absolutely over the grid.
pub(super) fn changed_cell(changed: usize) -> impl Element {
    div()
        .absolute()
        .pl((changed % GRID.0) as f32 * CELL.0)
        .pt((changed / GRID.0) as f32 * CELL.1)
        .child(div().w(CELL.0).h(CELL.1).bg(Color::hex(0xffffff)))
}

/// Which cell of `quad_grid_cached` is white, kept in an entity.
pub(super) struct ChangedCell(pub(super) usize);
