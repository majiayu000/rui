//! Paragraphs made of differently styled spans.

use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::LayoutNodes;
use rui::elements::text::FontWeight;
use rui::elements::{Div, div, shared_styled_text, span, span_range, styled_text};
use rui::renderer::text::TextMeasureCache;
use rui::renderer::{Primitive, Scene};
use rui::{Bounds, Color, ElementId, Size};
use taffy::prelude::TaffyTree;

/// One drawn piece of text.
#[derive(Debug, Clone, PartialEq)]
struct Piece {
    bounds: Bounds,
    content: String,
    color: Color,
    font_size: f32,
    font_weight: u16,
}

fn paint(root: &mut Div, taffy: &TaffyTree<ElementId>, bounds: Bounds) -> Vec<Piece> {
    let mut scene = Scene::new();
    FramePipeline::paint_root(root, taffy, &mut scene, bounds);
    scene
        .primitives()
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Text {
                bounds,
                content,
                color,
                font_size,
                font_weight,
                ..
            } => Some(Piece {
                bounds: *bounds,
                content: content.to_string(),
                color: Color::Rgba(*color),
                font_size: *font_size,
                font_weight: *font_weight,
            }),
            _ => None,
        })
        .collect()
}

fn lay_out(root: &mut Div, viewport: Size) -> (Bounds, Vec<Piece>) {
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut measurer = TextMeasureCache::new();
    let bounds =
        FramePipeline::layout_root_with_text_measurer(root, &mut taffy, &mut measurer, viewport)
            .expect("layout");
    (bounds, paint(root, &taffy, bounds))
}

fn rgba(color: Color) -> Color {
    Color::Rgba(color.to_rgba())
}

#[test]
fn spans_inherit_the_parent_style_and_override_what_they_set() {
    let mut root = div()
        .text_color(Color::RED)
        .text_size(16.0)
        .child(styled_text([
            span("plain "),
            span("bold ").bold(),
            span("blue ").color(Color::BLUE),
            span("big").size(24.0),
        ]));
    let (_, pieces) = lay_out(&mut root, Size::new(800.0, 200.0));

    let styles: Vec<_> = pieces
        .iter()
        .map(|piece| {
            (
                piece.content.as_str(),
                piece.color,
                piece.font_size,
                piece.font_weight,
            )
        })
        .collect();
    assert_eq!(
        styles,
        [
            ("plain ", rgba(Color::RED), 16.0, 400),
            ("bold ", rgba(Color::RED), 16.0, 700),
            ("blue ", rgba(Color::BLUE), 16.0, 400),
            ("big", rgba(Color::RED), 24.0, 400),
        ]
    );
    // One line: pieces left to right, every one on the same baseline row.
    assert!(
        pieces
            .windows(2)
            .all(|pair| pair[0].bounds.x() < pair[1].bounds.x())
    );
    assert_eq!(
        pieces[3].bounds.y(),
        0.0,
        "the tallest span starts the line"
    );
    assert!(
        pieces[0].bounds.y() > 0.0,
        "smaller spans move down to its baseline"
    );
}

#[test]
fn a_styled_paragraph_wraps_across_spans_and_reports_its_height() {
    let words = "wraps across spans like one text ".repeat(6);
    let mut root = div().flex_col().w(180.0).child(styled_text([
        span(words.clone()),
        span(words.clone()).bold().color(Color::BLUE),
        span(words.clone()).size(20.0),
    ]));
    let (bounds, pieces) = lay_out(&mut root, Size::new(640.0, 900.0));

    let mut rows: Vec<f32> = pieces.iter().map(|piece| piece.bounds.y()).collect();
    rows.dedup();
    assert!(rows.len() > 6, "expected many lines, got {pieces:?}");
    for piece in &pieces {
        assert!(
            piece.bounds.x() + piece.bounds.width() <= 180.0 + 2.0,
            "piece runs past the container: {piece:?}"
        );
    }
    let last = pieces.last().expect("pieces");
    assert!(bounds.height() >= last.bounds.y() + last.bounds.height() - 1.0);
    // Pieces of one line are contiguous text; lines are separated by a break.
    let mut drawn = String::new();
    let mut row = None;
    for piece in &pieces {
        let top = piece.bounds.y() + piece.bounds.height();
        if row.is_some_and(|row: f32| (row - top).abs() > 6.0) {
            drawn.push(' ');
        }
        row = Some(top);
        drawn.push_str(&piece.content);
    }
    let expected = format!("{words}{words}{words}");
    assert_eq!(
        drawn.split_whitespace().collect::<Vec<_>>(),
        expected.split_whitespace().collect::<Vec<_>>()
    );
}

#[test]
fn truncated_styled_text_ends_in_an_ellipsis() {
    let mut root = div().w(120.0).child(
        styled_text([
            span("Status: ").semibold(),
            span("every check passed on the first try today").color(Color::GREEN),
        ])
        .truncate(),
    );
    let (bounds, pieces) = lay_out(&mut root, Size::new(640.0, 480.0));

    assert_eq!(
        pieces.last().map(|piece| piece.content.as_str()),
        Some("\u{2026}")
    );
    assert!(
        pieces
            .iter()
            .all(|piece| piece.bounds.y() == pieces[0].bounds.y())
    );
    assert_eq!(bounds.height(), (14.0f32 * 1.4).round());
    assert_eq!(pieces[0].font_weight, FontWeight::Semibold.to_value());
}

#[test]
fn a_retained_layout_rewraps_styled_text_when_its_container_resizes() {
    let words = "resize rewraps the styled paragraph ".repeat(4);
    let build = |width: f32| {
        div()
            .w(width)
            .child(styled_text([span(words.clone()), span("end").bold()]))
    };
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut nodes = LayoutNodes::new();
    let mut measurer = TextMeasureCache::new();
    let mut rows = |width: f32| {
        let mut root = build(width);
        let bounds = FramePipeline::layout_root_retained(
            &mut root,
            &mut taffy,
            &mut nodes,
            &mut measurer,
            Size::new(800.0, 800.0),
        )
        .expect("layout");
        let mut ys: Vec<f32> = paint(&mut root, &taffy, bounds)
            .iter()
            .map(|piece| piece.bounds.y())
            .collect();
        ys.dedup();
        ys.len()
    };
    let wide = rows(600.0);
    let narrow = rows(150.0);
    assert!(narrow > wide, "{narrow} lines at 150px, {wide} at 600px");
    assert_eq!(rows(600.0), wide);
}

#[test]
fn spans_over_one_shared_text_draw_as_spans_with_their_own_text() {
    let line: std::sync::Arc<str> = std::sync::Arc::from("let width = 42;");
    let mut shared = div().text_size(16.0).child(shared_styled_text(
        std::sync::Arc::clone(&line),
        [
            span_range(0..4).color(Color::BLUE),
            span_range(4..9).bold(),
            span_range(9..12),
            span_range(12..15).size(20.0),
        ],
    ));
    let mut owned = div().text_size(16.0).child(styled_text([
        span("let ").color(Color::BLUE),
        span("width").bold(),
        span(" = "),
        span("42;").size(20.0),
    ]));
    let viewport = Size::new(800.0, 200.0);
    assert_eq!(
        lay_out(&mut shared, viewport),
        lay_out(&mut owned, viewport)
    );
}

#[test]
fn appending_a_span_extends_the_text() {
    let mut appended = div().child(styled_text([span("one ")]).span(span("two").bold()));
    let mut listed = div().child(styled_text([span("one "), span("two").bold()]));
    let viewport = Size::new(800.0, 200.0);
    assert_eq!(
        lay_out(&mut appended, viewport),
        lay_out(&mut listed, viewport)
    );
}

#[test]
#[should_panic(expected = "cover 3 of its 5 bytes")]
fn shared_spans_must_cover_the_whole_text() {
    shared_styled_text("hello", [span_range(0..3)]);
}

#[test]
#[should_panic(expected = "does not follow byte 2")]
fn shared_spans_must_follow_each_other() {
    shared_styled_text("hello", [span_range(0..2), span_range(3..5)]);
}

#[test]
#[should_panic(expected = "place it with shared_styled_text")]
fn a_range_span_needs_shared_text() {
    styled_text([span_range(0..2)]);
}
