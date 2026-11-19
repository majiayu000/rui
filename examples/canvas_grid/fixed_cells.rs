//! A few decoded terminal cells at explicit columns, drawn with public APIs.

use rui::elements::Painter;
use rui::elements::text::TextAlign;
use rui::prelude::*;
use rui::renderer::Primitive;
use rui::renderer::text::{BaseDirection, FontGlyph, GlyphResolver};

pub(crate) const FONT_SIZE: f32 = 13.0;
pub(crate) const CELL_HEIGHT: f32 = 16.0;
pub(crate) const FIXTURE_HEIGHT: f32 = 4.0 * CELL_HEIGHT;
#[cfg(target_os = "macos")]
pub(crate) const FAMILY: Option<&str> = Some("SF Mono");
#[cfg(target_os = "windows")]
pub(crate) const FAMILY: Option<&str> = Some("Consolas");
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) const FAMILY: Option<&str> = None;

pub(crate) fn cell_metrics(resolver: &mut GlyphResolver) -> (f32, f32) {
    let cell_width = resolver
        .resolve('0')
        .map_or(8.0, |zero| zero.advance(FONT_SIZE));
    let (ascent, descent) = resolver
        .ascent_descent(FONT_SIZE)
        .unwrap_or((FONT_SIZE * 0.8, -FONT_SIZE * 0.2));
    let baseline = (CELL_HEIGHT - (ascent - descent)) * 0.5 + ascent;
    (cell_width, baseline)
}

pub(crate) fn paint(
    bounds: Bounds,
    painter: &mut Painter<'_>,
    cell_width: f32,
    baseline: f32,
    sentinel: Option<FontGlyph>,
) {
    // (Starting column, occupied columns, decoded grapheme). Positions come
    // from the cell stream, independently of what the fonts advance by.
    for (column, span, cluster) in [(0, 2, "中"), (4, 1, "e\u{0301}"), (7, 2, "👩‍💻")] {
        let x = bounds.x() + column as f32 * cell_width;
        let cell = Bounds::from_xywh(x, bounds.y(), span as f32 * cell_width, CELL_HEIGHT);
        painter.clip(cell, |painter| {
            painter.paint(Primitive::Text {
                bounds: cell,
                content: cluster.into(),
                color: Color::WHITE.to_rgba(),
                font_size: FONT_SIZE,
                font_weight: 400,
                font_family: FAMILY.map(Into::into),
                font_features: Default::default(),
                line_height: 1.0,
                // The text renderer centers the primary face's ascent/descent,
                // matching the baseline used to place the cached glyphs.
                align: TextAlign::Center,
                direction: BaseDirection::Auto,
            });
        });
        if let Some(sentinel) = sentinel {
            painter.glyph(
                sentinel,
                Point::new(cell.max_x(), bounds.y() + baseline),
                FONT_SIZE,
                Color::WHITE,
            );
        }
    }

    // The cells spell ┌──┐ / │██│ / └──┘. Draw each continuous stroke or
    // fill as one rectangle, so fractional cell widths leave no joins.
    let row_y = bounds.y() + CELL_HEIGHT;
    let left = bounds.x() + cell_width * 0.5;
    let top = row_y + CELL_HEIGHT * 0.5;
    let bottom = top + 2.0 * CELL_HEIGHT;
    let box_width = 3.0 * cell_width;
    for y in [top, bottom] {
        painter.fill(
            Bounds::from_xywh(left - 0.5, y - 0.5, box_width + 1.0, 1.0),
            Color::WHITE,
        );
    }
    for x in [left, left + box_width] {
        painter.fill(
            Bounds::from_xywh(x - 0.5, top, 1.0, bottom - top),
            Color::WHITE,
        );
    }
    painter.fill(
        Bounds::from_xywh(
            bounds.x() + cell_width,
            row_y + CELL_HEIGHT,
            2.0 * cell_width,
            CELL_HEIGHT,
        ),
        Color::WHITE,
    );
}
