//! A 200x60 character grid with per-cell colors, painted by one canvas.
//!
//! Each cell is a quad and a glyph resolved once by a `GlyphResolver`, so the
//! grid costs 12,000 quads and 12,000 glyphs and shapes nothing. The cell width
//! is the advance of `0`; wide characters are centred on two cells, so they do
//! not push later columns off the grid. Click a cell to change its character.

use rui::elements::element::PointerEventKind;
use rui::prelude::*;
use rui::renderer::text::{FontGlyph, GlyphResolver};
use std::cell::RefCell;
use std::rc::Rc;

const COLUMNS: usize = 200;
const ROWS: usize = 60;
const FONT_SIZE: f32 = 13.0;
const CELL_HEIGHT: f32 = 16.0;
#[cfg(target_os = "macos")]
const FAMILY: Option<&str> = Some("SF Mono");
#[cfg(target_os = "windows")]
const FAMILY: Option<&str> = Some("Consolas");
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const FAMILY: Option<&str> = None;

fn cell_color(row: usize, column: usize) -> Color {
    let hue = column as f32 / COLUMNS as f32 * 360.0;
    let lightness = 0.15 + row as f32 / ROWS as f32 * 0.25;
    Color::hsl(hue, 0.6, lightness)
}

/// The character first drawn in a cell: printable ASCII, and every tenth
/// row a CJK word, two cells a character.
fn initial(row: usize, column: usize) -> (char, bool) {
    if row % 10 == 5 && (20..40).contains(&column) {
        let wide = ['中', '文', '网', '格', '测'];
        return (wide[(column - 20) / 2 % wide.len()], column.is_multiple_of(2));
    }
    (char::from(b'!' + ((row * 7 + column) % 94) as u8), true)
}

/// A cell's glyph, or `None` for a cell a wide glyph on its left covers.
struct Grid {
    resolver: GlyphResolver,
    cells: Vec<Option<FontGlyph>>,
}

impl Grid {
    fn new() -> Self {
        let mut resolver = GlyphResolver::new(FAMILY, 400);
        let cells = (0..ROWS)
            .flat_map(|row| (0..COLUMNS).map(move |column| (row, column)))
            .map(|(row, column)| {
                let (ch, starts) = initial(row, column);
                starts.then(|| resolver.resolve(ch)).flatten()
            })
            .collect();
        Self { resolver, cells }
    }

    fn cell_width(&mut self) -> f32 {
        self.resolver
            .resolve('0')
            .map_or(8.0, |zero| zero.advance(FONT_SIZE))
    }
}

struct GridView {
    grid: Rc<RefCell<Grid>>,
    cell_width: f32,
    /// Baseline from a cell's top: the line centred in the cell, as a text
    /// line is centred in its box.
    baseline: f32,
}

impl View for GridView {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let (painted, clicked) = (Rc::clone(&self.grid), Rc::clone(&self.grid));
        let (cell_width, baseline) = (self.cell_width, self.baseline);
        let notifier = cx.notifier();
        div().bg(Color::hex(0x101014)).p(16.0).child(
            canvas(move |bounds, painter| {
                let grid = painted.borrow();
                for row in 0..ROWS {
                    let y = bounds.y() + row as f32 * CELL_HEIGHT;
                    for column in 0..COLUMNS {
                        let x = bounds.x() + column as f32 * cell_width;
                        let cell = Bounds::from_xywh(x, y, cell_width, CELL_HEIGHT);
                        painter.fill(cell, cell_color(row, column));
                        let Some(glyph) = grid.cells[row * COLUMNS + column] else {
                            continue;
                        };
                        let advance = glyph.advance(FONT_SIZE);
                        let span = if advance > cell_width * 1.5 { 2.0 } else { 1.0 };
                        let centred = (span * cell_width - advance) * 0.5;
                        painter.glyph(
                            glyph,
                            Point::new(x + centred.max(0.0), y + baseline),
                            FONT_SIZE,
                            Color::WHITE,
                        );
                    }
                }
            })
            .w(COLUMNS as f32 * cell_width)
            .h(ROWS as f32 * CELL_HEIGHT)
            .on_pointer(move |event| {
                if event.kind != PointerEventKind::Up {
                    return;
                }
                let row = (event.position.y / CELL_HEIGHT) as usize;
                let column = (event.position.x / cell_width) as usize;
                if row < ROWS && column < COLUMNS {
                    let mut grid = clicked.borrow_mut();
                    let glyph = grid.resolver.resolve('#');
                    grid.cells[row * COLUMNS + column] = glyph;
                    notifier.notify();
                }
            }),
        )
    }
}

fn main() {
    let mut grid = Grid::new();
    let cell_width = grid.cell_width();
    let (ascent, descent) = grid
        .resolver
        .ascent_descent(FONT_SIZE)
        .unwrap_or((FONT_SIZE * 0.8, -FONT_SIZE * 0.2));
    let options = WindowOptions::default().title("RUI Canvas Grid").size(
        COLUMNS as f32 * cell_width + 32.0,
        ROWS as f32 * CELL_HEIGHT + 32.0,
    );
    App::new().run_view_with_options(
        options,
        GridView {
            grid: Rc::new(RefCell::new(grid)),
            cell_width,
            baseline: (CELL_HEIGHT - (ascent - descent)) * 0.5 + ascent,
        },
    );
}
