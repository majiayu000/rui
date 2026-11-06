//! Incremental frames must draw exactly what a full frame draws.
//!
//! Each frame rebuilds a randomly edited tree. The incremental side keeps the
//! unchanged parts of the previous tree (`rui::elements::element::reconcile`), their layout
//! nodes and their paint; the full side lays out and paints a fresh tree from
//! scratch. Their scenes must match primitive for primitive.

#[path = "support/random_tree.rs"]
mod random_tree;

use random_tree::{Incremental, Node, Rng};
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::{Div, canvas, div, span, styled_text, text};
use rui::renderer::text::TextMeasureCache;
use rui::renderer::{Primitive, Scene};
use rui::{Bounds, Color, ElementId, Size};
use std::cell::Cell;
use std::rc::Rc;
use taffy::prelude::TaffyTree;

fn full_frame(mut root: Div, measurer: &mut TextMeasureCache, viewport: Size) -> Vec<String> {
    let mut taffy = TaffyTree::<ElementId>::new();
    let bounds = match FramePipeline::layout_root_with_text_measurer(
        &mut root, &mut taffy, measurer, viewport,
    ) {
        Ok(bounds) => bounds,
        Err(err) => panic!("fresh layout failed: {err}"),
    };
    let mut scene = Scene::new();
    FramePipeline::paint_root(&mut root, &taffy, &mut scene, bounds);
    describe(scene.primitives(), viewport)
}

/// One line per primitive, so a mismatch names the first one that differs.
///
/// Only what can be seen in `viewport` counts: the incremental runner does not
/// paint unchanged subtrees outside it.
fn describe(primitives: &[Primitive], viewport: Size) -> Vec<String> {
    let viewport = Bounds::from_xywh(0.0, 0.0, viewport.width, viewport.height);
    primitives
        .iter()
        .filter(|primitive| primitive.may_touch(&viewport))
        .map(|primitive| format!("{primitive:?}"))
        .collect()
}

fn assert_same(kept: &[String], full: &[String], context: &str) {
    if let Some(index) = (0..kept.len().max(full.len())).find(|&i| kept.get(i) != full.get(i)) {
        panic!(
            "{context}: primitive {index} of {} (full frame has {}) differs\n  kept: {:?}\n  full: {:?}",
            kept.len(),
            full.len(),
            kept.get(index),
            full.get(index)
        );
    }
}

#[test]
fn incremental_frames_match_full_frames_over_random_edits() {
    let mut measurer = TextMeasureCache::new();
    let mut replayed = 0;
    for seed in 1..=32u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
        let mut model = Node::random(&mut rng, 4);
        let mut incremental = Incremental::new();
        incremental.rebuild(model.build());
        let mut viewport = Size::new(640.0, 480.0);
        for frame in 0..40 {
            match rng.below(5) {
                // A frame without a rebuild, as after an event that changed nothing.
                0 => {}
                // A resize: everything that moves replays at its new origin.
                1 => {
                    viewport =
                        Size::new(200.0 + rng.below(600) as f32, 200.0 + rng.below(400) as f32);
                    incremental.rebuild(model.build());
                }
                _ => {
                    for _ in 0..1 + rng.below(3) {
                        let at = rng.below(model.count() as u64) as usize;
                        model.nth(at).edit(&mut rng);
                    }
                    incremental.rebuild(model.build());
                }
            }
            incremental.frame(&mut measurer, viewport);
            let kept = describe(incremental.scene.primitives(), viewport);
            let full = full_frame(model.build(), &mut measurer, viewport);
            assert_same(&kept, &full, &format!("seed {seed}, frame {frame}"));
            replayed += incremental.scene.replayed_len();
        }
    }
    assert!(
        replayed > 0,
        "no frame replayed anything: the oracle checked nothing"
    );
}

/// A list where one row changes per frame, the case incremental frames are for.
#[test]
fn one_changed_row_replays_the_others_moved_or_not() {
    let mut measurer = TextMeasureCache::new();
    let list = |edited: usize, round: usize| {
        div().w(320.0).flex_col().children((0..40).map(move |row| {
            let label = if row == edited {
                format!("edited {round}")
            } else {
                format!("row {row}")
            };
            // The edited row grows, which moves every row below it.
            let height = if row == edited {
                20.0 + round as f32
            } else {
                20.0
            };
            div().h(height).bg(Color::hex(0x334455)).child(text(label))
        }))
    };
    let viewport = Size::new(320.0, 2_000.0);
    let mut incremental = Incremental::new();
    for round in 0..12 {
        incremental.rebuild(list(round % 5, round));
        incremental.frame(&mut measurer, viewport);
        let kept = describe(incremental.scene.primitives(), viewport);
        let full = full_frame(list(round % 5, round), &mut measurer, viewport);
        assert_same(&kept, &full, &format!("round {round}"));
        if round > 0 {
            // 40 rows of a quad and a text; only the edited row repaints.
            assert!(
                incremental.scene.replayed_len() >= 76,
                "round {round} replayed {} primitives",
                incremental.scene.replayed_len()
            );
        }
    }
}

#[test]
fn a_changed_inherited_text_style_repaints_kept_text() {
    let mut measurer = TextMeasureCache::new();
    let tree = |color: u32| div().text_color(Color::hex(color)).child(text("same text"));
    let viewport = Size::new(320.0, 240.0);
    let mut incremental = Incremental::new();
    for color in [0x112233, 0x445566, 0x445566, 0x778899] {
        incremental.rebuild(tree(color));
        incremental.frame(&mut measurer, viewport);
        let kept = describe(incremental.scene.primitives(), viewport);
        let full = full_frame(tree(color), &mut measurer, viewport);
        assert_same(&kept, &full, &format!("color {color:06x}"));
    }
}

#[test]
fn a_changed_inherited_text_style_repaints_kept_styled_text() {
    let mut measurer = TextMeasureCache::new();
    let tree = |color: u32, size: f32| {
        div()
            .w(120.0)
            .text_color(Color::hex(color))
            .text_size(size)
            .child(styled_text([
                span("same styled "),
                span("text that wraps").bold(),
            ]))
    };
    let viewport = Size::new(320.0, 240.0);
    let mut incremental = Incremental::new();
    for (color, size) in [
        (0x112233, 14.0),
        (0x112233, 14.0),
        (0x445566, 14.0),
        (0x445566, 18.0),
        (0x445566, 18.0),
    ] {
        incremental.rebuild(tree(color, size));
        incremental.frame(&mut measurer, viewport);
        let kept = describe(incremental.scene.primitives(), viewport);
        let full = full_frame(tree(color, size), &mut measurer, viewport);
        assert_same(&kept, &full, &format!("color {color:06x}, size {size}"));
    }
}

#[test]
fn elements_that_read_shared_state_paint_every_frame() {
    let mut measurer = TextMeasureCache::new();
    let shade = Rc::new(Cell::new(0.0f32));
    let tree = |shade: &Rc<Cell<f32>>| {
        let shade = Rc::clone(shade);
        div().child(
            div().w(40.0).h(40.0).child(
                canvas(move |bounds: Bounds, painter| {
                    painter.fill(bounds, Color::rgb(shade.get(), 0.0, 0.0));
                })
                .w(10.0)
                .h(10.0),
            ),
        )
    };
    let viewport = Size::new(320.0, 240.0);
    let mut incremental = Incremental::new();
    incremental.rebuild(tree(&shade));
    for step in 0..4 {
        shade.set(step as f32 / 4.0);
        // No rebuild: the canvas must still draw its new shade.
        incremental.frame(&mut measurer, viewport);
        let kept = describe(incremental.scene.primitives(), viewport);
        let full = full_frame(tree(&shade), &mut measurer, viewport);
        assert_same(&kept, &full, &format!("step {step}"));
    }
    assert!(matches!(
        incremental.scene.primitives().last(),
        Some(Primitive::PopClip)
    ));
}

/// A column far taller than the window, pushed down and back up by a spacer, so
/// rows leave the view and come back.
fn tall_column(spacer: f32, edited: usize, round: usize) -> Div {
    div()
        .w(320.0)
        .flex_col()
        .child(div().h(spacer))
        .children((0..200).map(move |row| {
            let label = if row == edited {
                format!("edited {round}")
            } else {
                format!("row {row}")
            };
            div().h(20.0).bg(Color::hex(0x334455)).child(text(label))
        }))
}

#[test]
fn unchanged_rows_out_of_view_are_not_painted() {
    let mut measurer = TextMeasureCache::new();
    let viewport = Size::new(320.0, 200.0);
    let mut incremental = Incremental::new();
    let frames = [
        (0.0, 3),
        (0.0, 3),
        (60.0, 5),
        (400.0, 5),
        (0.0, 150),
        (2_000.0, 1),
        (20.0, 1),
    ];
    for (round, (spacer, edited)) in frames.into_iter().enumerate() {
        incremental.rebuild(tall_column(spacer, edited, round));
        incremental.frame(&mut measurer, viewport);
        let kept = describe(incremental.scene.primitives(), viewport);
        let full = full_frame(tall_column(spacer, edited, round), &mut measurer, viewport);
        assert_same(&kept, &full, &format!("round {round}"));
        if round > 0 {
            // A 200 px window shows at most 12 of the 20 px rows, two primitives each.
            assert!(
                incremental.scene.len() < 60,
                "round {round} painted {} primitives",
                incremental.scene.len()
            );
        }
    }
}
