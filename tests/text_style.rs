//! Text inherits its style from parents and wraps to its container.

use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::LayoutNodes;
use rui::elements::text::FontWeight;
use rui::elements::{Div, div, text};
use rui::renderer::text::{TextMeasureCache, TextRequest};
use rui::renderer::{Primitive, Scene};
use rui::{Bounds, Color, ElementId, Size};
use taffy::prelude::TaffyTree;

const PARAGRAPH: &str = "The quick brown fox jumps over the lazy dog while the five boxing \
                         wizards jump quickly and a wizard's job is to vex chumps quickly in fog.";

/// One drawn text line.
#[derive(Debug, Clone, PartialEq)]
struct Line {
    bounds: Bounds,
    content: String,
    color: Color,
    font_size: f32,
    font_weight: u16,
}

fn lay_out(root: &mut Div, viewport: Size) -> (Bounds, Vec<Line>) {
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut measurer = TextMeasureCache::new();
    let bounds =
        FramePipeline::layout_root_with_text_measurer(root, &mut taffy, &mut measurer, viewport)
            .expect("layout");
    (bounds, paint(root, &taffy, bounds))
}

fn paint(root: &mut Div, taffy: &TaffyTree<ElementId>, bounds: Bounds) -> Vec<Line> {
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
            } => Some(Line {
                bounds: *bounds,
                content: content.clone(),
                color: Color::Rgba(*color),
                font_size: *font_size,
                font_weight: *font_weight,
            }),
            _ => None,
        })
        .collect()
}

fn line_width(content: &str) -> f32 {
    TextMeasureCache::new()
        .measure_single_line(TextRequest::new(content, 14.0, 400, None, 1.4))
        .expect("measure")
        .size
        .width
}

/// Height of one 14px line; layout sizes are rounded to whole pixels.
const LINE_BOX: f32 = 14.0 * 1.4;

#[test]
fn parent_text_style_reaches_descendant_text() {
    let mut root = div()
        .flex_col()
        .text_color(Color::RED)
        .text_size(20.0)
        .font_weight(FontWeight::Bold)
        .child(text("inherits"))
        .child(text("own color").color(Color::BLUE))
        .child(div().text_size(10.0).child(text("nested")));
    let (_, lines) = lay_out(&mut root, Size::new(400.0, 300.0));

    let red = Color::Rgba(Color::RED.to_rgba());
    let blue = Color::Rgba(Color::BLUE.to_rgba());
    assert_eq!(lines.len(), 3);
    assert_eq!(
        (lines[0].color, lines[0].font_size, lines[0].font_weight),
        (red, 20.0, 700)
    );
    assert_eq!(
        (lines[1].color, lines[1].font_size, lines[1].font_weight),
        (blue, 20.0, 700)
    );
    assert_eq!(
        (lines[2].color, lines[2].font_size, lines[2].font_weight),
        (red, 10.0, 700)
    );
    // The inherited size is what the text was measured at.
    assert_eq!(lines[0].bounds.height(), 20.0 * 1.4);
    assert_eq!(lines[2].bounds.height(), 10.0 * 1.4);
}

#[test]
fn inherited_style_does_not_leak_to_siblings() {
    let mut root = div()
        .flex_col()
        .child(div().text_color(Color::RED).child(text("inside")))
        .child(text("outside"));
    let (_, lines) = lay_out(&mut root, Size::new(400.0, 300.0));

    assert_eq!(lines[1].color, Color::Rgba(Color::BLACK.to_rgba()));
    assert_eq!(lines[1].font_size, 14.0);
}

#[test]
fn long_paragraph_wraps_to_container_width_and_reports_its_height() {
    let mut root = div().flex_col().w(200.0).child(text(PARAGRAPH).wrap());
    let (bounds, lines) = lay_out(&mut root, Size::new(640.0, 480.0));

    assert!(lines.len() > 2, "expected several lines, got {lines:?}");
    for (index, line) in lines.iter().enumerate() {
        assert!(
            line_width(&line.content) <= 200.0 + 1.0,
            "line {index} is wider than its container: {line:?}"
        );
        assert_eq!(line.bounds.y(), index as f32 * LINE_BOX);
        assert_eq!(line.bounds.width(), 200.0);
    }
    assert_eq!(bounds.height(), (lines.len() as f32 * LINE_BOX).round());
    let words: Vec<&str> = lines
        .iter()
        .flat_map(|line| line.content.split_whitespace())
        .collect();
    assert_eq!(words, PARAGRAPH.split_whitespace().collect::<Vec<_>>());
}

#[test]
fn wrapped_text_in_a_row_takes_only_the_width_it_needs() {
    let mut root = div().flex_row().w(400.0).child(text("short").wrap());
    let (_, lines) = lay_out(&mut root, Size::new(640.0, 480.0));

    assert_eq!(lines.len(), 1);
    assert!((lines[0].bounds.width() - line_width("short")).abs() <= 1.0);
}

#[test]
fn max_lines_truncates_with_an_ellipsis() {
    let mut root = div()
        .flex_col()
        .w(200.0)
        .child(text(PARAGRAPH).max_lines(2));
    let (bounds, lines) = lay_out(&mut root, Size::new(640.0, 480.0));

    assert_eq!(lines.len(), 2);
    assert!(lines[1].content.ends_with('\u{2026}'), "{lines:?}");
    assert!(line_width(&lines[1].content) <= 200.0 + 1.0);
    assert_eq!(bounds.height(), (2.0 * LINE_BOX).round());
}

#[test]
fn truncate_keeps_one_line_that_fits() {
    let mut root = div().flex_col().w(120.0).child(text(PARAGRAPH).truncate());
    let (bounds, lines) = lay_out(&mut root, Size::new(640.0, 480.0));

    assert_eq!(lines.len(), 1);
    assert!(lines[0].content.ends_with('\u{2026}'));
    assert!(line_width(&lines[0].content) <= 120.0 + 1.0);
    assert_eq!(bounds.height(), LINE_BOX.round());
}

#[test]
fn text_that_fits_is_not_truncated() {
    let mut root = div().flex_col().w(400.0).child(text("fits").max_lines(1));
    let (_, lines) = lay_out(&mut root, Size::new(640.0, 480.0));

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].content, "fits");
}

#[test]
fn newlines_start_new_lines() {
    let mut root = div().flex_col().w(400.0).child(text("one\n\nthree").wrap());
    let (bounds, lines) = lay_out(&mut root, Size::new(640.0, 480.0));

    let contents: Vec<&str> = lines.iter().map(|line| line.content.as_str()).collect();
    assert_eq!(contents, ["one", "", "three"]);
    assert_eq!(bounds.height(), (3.0 * LINE_BOX).round());
}

#[test]
fn resizing_the_container_rewraps_in_a_retained_layout() {
    let build = |width: f32| div().flex_col().w(width).child(text(PARAGRAPH).wrap());
    let viewport = Size::new(640.0, 480.0);
    let mut measurer = TextMeasureCache::new();
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut nodes = LayoutNodes::new();

    let mut frames = Vec::new();
    for width in [320.0, 160.0, 320.0] {
        let mut root = build(width);
        let bounds = FramePipeline::layout_root_retained(
            &mut root,
            &mut taffy,
            &mut nodes,
            &mut measurer,
            viewport,
        )
        .expect("retained layout");
        let lines = paint(&mut root, &taffy, bounds);
        assert_eq!(bounds.height(), (lines.len() as f32 * LINE_BOX).round());
        assert_eq!(
            (bounds, lines.clone()),
            lay_out(&mut build(width), viewport),
            "width {width}"
        );
        frames.push(lines.len());
    }
    assert!(
        frames[1] > frames[0],
        "narrower container must add lines: {frames:?}"
    );
    assert_eq!(frames[2], frames[0]);
}

#[test]
fn retained_layout_rewraps_changed_text_with_an_unchanged_style() {
    let viewport = Size::new(640.0, 480.0);
    let mut measurer = TextMeasureCache::new();
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut nodes = LayoutNodes::new();

    let mut heights = Vec::new();
    for content in ["short", PARAGRAPH, "short"] {
        let mut root = div().flex_col().w(200.0).child(text(content).wrap());
        let bounds = FramePipeline::layout_root_retained(
            &mut root,
            &mut taffy,
            &mut nodes,
            &mut measurer,
            viewport,
        )
        .expect("retained layout");
        heights.push(bounds.height());
    }
    assert_eq!(heights[0], LINE_BOX.round());
    assert!(heights[1] > 2.0 * LINE_BOX, "{heights:?}");
    assert_eq!(heights[2], LINE_BOX.round());
}
