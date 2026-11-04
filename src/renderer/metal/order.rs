//! Draw order from overlap, as GPUI's `BoundsTree` assigns it: a primitive's
//! order is one more than the highest order of any earlier primitive it
//! overlaps, so primitives that overlap nothing earlier share an order and may
//! be drawn in any sequence among themselves.
//!
//! The index is a uniform grid over the viewport. Each cell lists the bounds
//! that touch it and the highest order among them, so a query skips cells that
//! cannot raise its answer. Cells keep their storage across frames.

/// Logical pixels per grid cell.
const CELL: f32 = 64.0;

/// `[x0, y0, x1, y1]`, half open: a pixel center on `x1` or `y1` is outside.
pub(super) type Rect = [f32; 4];

#[derive(Default)]
struct Cell {
    max_order: u32,
    entries: Vec<u32>,
}

#[derive(Default)]
pub(super) struct OverlapOrder {
    columns: usize,
    rows: usize,
    cells: Vec<Cell>,
    entries: Vec<(Rect, u32)>,
}

impl OverlapOrder {
    /// Empties the index for a frame of `width` by `height` logical pixels.
    pub(super) fn reset(&mut self, width: f32, height: f32) {
        let columns = ((width / CELL).ceil() as usize).max(1);
        let rows = ((height / CELL).ceil() as usize).max(1);
        if columns != self.columns || rows != self.rows {
            self.columns = columns;
            self.rows = rows;
            self.cells.clear();
            self.cells.resize_with(columns * rows, Cell::default);
        } else {
            for cell in &mut self.cells {
                cell.max_order = 0;
                cell.entries.clear();
            }
        }
        self.entries.clear();
    }

    /// The order of a primitive covering `rect`, which must lie within the frame
    /// passed to `reset`. Orders start at 1.
    pub(super) fn insert(&mut self, rect: Rect) -> u32 {
        let (c0, r0, c1, r1) = self.cell_range(rect);
        let mut below = 0;
        for row in r0..=r1 {
            for column in c0..=c1 {
                let cell = &self.cells[row * self.columns + column];
                if cell.max_order <= below {
                    continue;
                }
                for &entry in &cell.entries {
                    let (other, order) = self.entries[entry as usize];
                    if order > below && overlaps(rect, other) {
                        below = order;
                    }
                }
            }
        }
        let order = below + 1;
        let entry = self.entries.len() as u32;
        self.entries.push((rect, order));
        for row in r0..=r1 {
            for column in c0..=c1 {
                let cell = &mut self.cells[row * self.columns + column];
                cell.max_order = cell.max_order.max(order);
                cell.entries.push(entry);
            }
        }
        order
    }

    fn cell_range(&self, rect: Rect) -> (usize, usize, usize, usize) {
        let column = |x: f32| ((x / CELL).floor().max(0.0) as usize).min(self.columns - 1);
        let row = |y: f32| ((y / CELL).floor().max(0.0) as usize).min(self.rows - 1);
        (column(rect[0]), row(rect[1]), column(rect[2]), row(rect[3]))
    }
}

/// Whether two half-open rectangles share any area. Rectangles that only touch
/// share no pixel center, since the rasterizer gives an edge's pixels to one side.
pub(super) fn overlaps(a: Rect, b: Rect) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
        [x, y, x + width, y + height]
    }

    #[test]
    fn rows_that_only_touch_share_an_order() {
        let mut order = OverlapOrder::default();
        order.reset(1440.0, 900.0);
        for row in 0..30 {
            let y = row as f32 * 28.0;
            assert_eq!(
                order.insert(rect(0.0, y, 1440.0, 28.0)),
                1,
                "background {row}"
            );
            assert_eq!(
                order.insert(rect(8.0, y + 6.0, 200.0, 16.0)),
                2,
                "title {row}"
            );
            assert_eq!(
                order.insert(rect(1380.0, y + 6.0, 40.0, 16.0)),
                2,
                "time {row}"
            );
        }
    }

    #[test]
    fn order_rises_with_each_overlap_and_only_then() {
        let mut order = OverlapOrder::default();
        order.reset(400.0, 400.0);
        assert_eq!(order.insert(rect(0.0, 0.0, 100.0, 100.0)), 1);
        assert_eq!(order.insert(rect(50.0, 50.0, 100.0, 100.0)), 2);
        assert_eq!(order.insert(rect(120.0, 120.0, 10.0, 10.0)), 3);
        // Far from all of them, across many cells.
        assert_eq!(order.insert(rect(200.0, 0.0, 200.0, 400.0)), 1);
        // Over the first only: the second's higher order is not reached.
        assert_eq!(order.insert(rect(0.0, 0.0, 40.0, 40.0)), 2);
        assert_eq!(order.insert(rect(0.0, 0.0, 400.0, 400.0)), 4);
    }

    #[test]
    fn bounds_past_the_frame_fall_in_its_edge_cells() {
        let mut order = OverlapOrder::default();
        order.reset(100.0, 100.0);
        assert_eq!(order.insert(rect(-50.0, -50.0, 60.0, 60.0)), 1);
        assert_eq!(order.insert(rect(5.0, 5.0, 2.0, 2.0)), 2);
        assert_eq!(order.insert(rect(90.0, 90.0, 500.0, 500.0)), 1);
    }

    #[test]
    fn reset_forgets_the_previous_frame() {
        let mut order = OverlapOrder::default();
        order.reset(100.0, 100.0);
        order.insert(rect(0.0, 0.0, 100.0, 100.0));
        order.reset(100.0, 100.0);
        assert_eq!(order.insert(rect(0.0, 0.0, 100.0, 100.0)), 1);
        order.reset(300.0, 100.0);
        assert_eq!(order.insert(rect(0.0, 0.0, 300.0, 100.0)), 1);
    }
}
