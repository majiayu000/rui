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

use crate::workbench_data::{
    FONT_SIZE, PANE_BG, ROW_HEIGHT, TEXT_COLOR, VISIBLE_ROWS, terminal_text,
};

const FONT: &str = "SF Mono";

/// The screen at frame `turn`.
pub(super) fn terminal_rows(turn: usize, width: f32) -> impl Element {
    let rows: Vec<SharedStr> = (0..VISIBLE_ROWS)
        .map(|row| terminal_text(row, turn).into())
        .collect();
    // `RUI_TERMINAL_FONT` names another family, such as a static TrueType one.
    let family: Arc<str> = Arc::from(
        std::env::var("RUI_TERMINAL_FONT")
            .as_deref()
            .unwrap_or(FONT),
    );
    canvas(move |bounds, painter| {
        painter.fill(bounds, Color::hex(PANE_BG));
        for (index, text) in rows.iter().enumerate() {
            painter.paint(Primitive::Text {
                bounds: Bounds::from_xywh(
                    bounds.x(),
                    bounds.y() + index as f32 * ROW_HEIGHT,
                    bounds.width(),
                    ROW_HEIGHT,
                ),
                content: text.clone(),
                color: Color::hex(TEXT_COLOR).to_rgba(),
                font_size: FONT_SIZE,
                font_weight: 400,
                font_family: Some(family.clone()),
                font_features: Default::default(),
                line_height: ROW_HEIGHT,
                align: TextAlign::Left,
                direction: BaseDirection::Auto,
            });
        }
    })
    .w(width)
    .h(VISIBLE_ROWS as f32 * ROW_HEIGHT)
}
