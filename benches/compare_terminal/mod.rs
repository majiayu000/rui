//! The `terminal_rows` case of `compare_frame`: a terminal's screen as one canvas with
//! one text primitive per row, as a terminal pane draws it. 45 rows of 20 px in SF Mono
//! 13 px. Each frame rows 0..20 get new text (a frame counter and 60 hex characters),
//! as a program printing over the screen does, and the other 25 keep theirs.
//! `benchmarks/gpui_compare/terminal.rs` is the same screen on GPUI.

use rui::elements::canvas;
use rui::elements::element::Element;
use rui::elements::text::TextAlign;
use rui::renderer::Primitive;
use rui::renderer::text::BaseDirection;
use rui::{Bounds, Color, SharedStr};
use std::sync::Arc;

const ROWS: usize = 45;
const CHANGED_ROWS: usize = 20;
const LINE_HEIGHT: f32 = 20.0;
const FONT_SIZE: f32 = 13.0;
const FONT: &str = "SF Mono";
const WIDTH: f32 = 1440.0;
const HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn row_text(row: usize, turn: usize) -> String {
    if row < CHANGED_ROWS {
        format!("{turn:08} row {row:02} {}", &HEX[..60])
    } else {
        format!("static   row {row:02} {}", &HEX[..60])
    }
}

/// The screen at frame `turn`.
pub(super) fn terminal_rows(turn: usize) -> impl Element {
    let rows: Vec<SharedStr> = (0..ROWS).map(|row| row_text(row, turn).into()).collect();
    let family: Arc<str> = Arc::from(FONT);
    canvas(move |bounds, painter| {
        painter.fill(bounds, Color::hex(0x1e1e1e));
        for (index, text) in rows.iter().enumerate() {
            painter.paint(Primitive::Text {
                bounds: Bounds::from_xywh(
                    bounds.x(),
                    bounds.y() + index as f32 * LINE_HEIGHT,
                    bounds.width(),
                    LINE_HEIGHT,
                ),
                content: text.clone(),
                color: Color::hex(0xd4d4d4).to_rgba(),
                font_size: FONT_SIZE,
                font_weight: 400,
                font_family: Some(family.clone()),
                font_features: Default::default(),
                line_height: LINE_HEIGHT,
                align: TextAlign::Left,
                direction: BaseDirection::Auto,
            });
        }
    })
    .w(WIDTH)
    .h(ROWS as f32 * LINE_HEIGHT)
}
