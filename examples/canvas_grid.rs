//! A 200x60 character grid with per-cell colors, painted by one canvas.
//!
//! Each cell is a quad and each row one text run, so the grid costs 12,000 quads
//! and 60 text primitives. Click a cell to print its row and column.

use rui::elements::element::PointerEventKind;
use rui::elements::text::TextAlign;
use rui::prelude::*;
use rui::renderer::Primitive;
use rui::renderer::text::BaseDirection;

const COLUMNS: usize = 200;
const ROWS: usize = 60;
const CELL_WIDTH: f32 = 8.0;
const CELL_HEIGHT: f32 = 16.0;

fn cell_color(row: usize, column: usize) -> Color {
    let hue = column as f32 / COLUMNS as f32 * 360.0;
    let lightness = 0.15 + row as f32 / ROWS as f32 * 0.25;
    Color::hsl(hue, 0.6, lightness)
}

fn row_text(row: usize) -> String {
    (0..COLUMNS)
        .map(|column| char::from(b'!' + ((row * 7 + column) % 94) as u8))
        .collect()
}

fn main() {
    let rows: Vec<String> = (0..ROWS).map(row_text).collect();
    let options = WindowOptions::default().title("RUI Canvas Grid").size(
        COLUMNS as f32 * CELL_WIDTH + 32.0,
        ROWS as f32 * CELL_HEIGHT + 32.0,
    );
    App::new().run_with_options(options, move |_cx| {
        let rows = rows.clone();
        div().bg(Color::hex(0x101014)).p(16.0).child(
            canvas(move |bounds, painter| {
                for (row, content) in rows.iter().enumerate() {
                    let y = bounds.y() + row as f32 * CELL_HEIGHT;
                    for column in 0..COLUMNS {
                        let x = bounds.x() + column as f32 * CELL_WIDTH;
                        let cell = Bounds::from_xywh(x, y, CELL_WIDTH, CELL_HEIGHT);
                        painter.fill(cell, cell_color(row, column));
                    }
                    painter.paint(Primitive::Text {
                        bounds: Bounds::from_xywh(bounds.x(), y, bounds.width(), CELL_HEIGHT),
                        content: content.clone().into(),
                        color: Color::WHITE.to_rgba(),
                        font_size: 13.0,
                        font_weight: 400,
                        font_family: Some("SF Mono".into()),
                        line_height: 1.0,
                        align: TextAlign::Left,
                        direction: BaseDirection::Auto,
                    });
                }
            })
            .w(COLUMNS as f32 * CELL_WIDTH)
            .h(ROWS as f32 * CELL_HEIGHT)
            .on_pointer(|event| {
                if event.kind == PointerEventKind::Up {
                    let row = (event.position.y / CELL_HEIGHT) as usize;
                    let column = (event.position.x / CELL_WIDTH) as usize;
                    if row < ROWS && column < COLUMNS {
                        println!("cell row {row}, column {column}");
                    }
                }
            }),
        )
    });
}
