//! A retained layout pass must place everything exactly where a fresh pass does,
//! however the tree changes between frames.

use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::LayoutNodes;
use rui::elements::{Div, div, text};
use rui::renderer::Scene;
use rui::renderer::text::TextMeasureCache;
use rui::{Color, ElementId, Size};
use taffy::prelude::TaffyTree;

/// xorshift64*: deterministic without a dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn tree(rng: &mut Rng, depth: u32) -> Div {
    let mut node = div().bg(Color::hex(rng.below(0xff_ffff) as u32));
    node = match rng.below(3) {
        0 => node.flex_row().gap(rng.below(8) as f32),
        1 => node.flex_col().p(rng.below(6) as f32),
        _ => node.flex_row().justify_between().items_center(),
    };
    if rng.below(3) == 0 {
        node = node.w(20.0 + rng.below(200) as f32);
    }
    if rng.below(3) == 0 {
        node = node.h(10.0 + rng.below(60) as f32);
    }
    if rng.below(4) == 0 {
        node = node.flex_grow(1.0);
    }
    let children = if depth == 0 { 0 } else { rng.below(5) };
    for _ in 0..children {
        node = if rng.below(3) == 0 {
            let words = ["a", "row", "longer label", "x y z", ""];
            node.child(
                text(words[rng.below(words.len() as u64) as usize])
                    .size(10.0 + rng.below(8) as f32),
            )
        } else {
            node.child(tree(rng, depth - 1))
        };
    }
    node
}

fn scene_of(root: &mut Div, taffy: &TaffyTree<ElementId>, bounds: rui::Bounds) -> String {
    let mut scene = Scene::new();
    FramePipeline::paint_root(root, taffy, &mut scene, bounds);
    format!("{:?}", scene.primitives())
}

#[test]
fn retained_layout_matches_a_fresh_layout_across_changing_trees() {
    let viewport = Size::new(640.0, 480.0);
    let mut measurer = TextMeasureCache::new();
    for seed in 1..=24u64 {
        let mut kept_taffy = TaffyTree::<ElementId>::new();
        let mut nodes = LayoutNodes::new();
        let mut frame_rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        for frame in 0..30 {
            // Mostly small edits of one tree, sometimes a different shape.
            let tree_seed = if frame_rng.below(4) == 0 {
                frame_rng.next() | 1
            } else {
                seed * 1_000 + frame_rng.below(3)
            };
            let mut retained = tree(&mut Rng(tree_seed), 4);
            let mut fresh = tree(&mut Rng(tree_seed), 4);

            let retained_bounds = FramePipeline::layout_root_retained(
                &mut retained,
                &mut kept_taffy,
                &mut nodes,
                &mut measurer,
                viewport,
            )
            .expect("retained layout");
            let mut fresh_taffy = TaffyTree::<ElementId>::new();
            let fresh_bounds = FramePipeline::layout_root_with_text_measurer(
                &mut fresh,
                &mut fresh_taffy,
                &mut measurer,
                viewport,
            )
            .expect("fresh layout");

            assert_eq!(retained_bounds, fresh_bounds, "seed {seed} frame {frame}");
            assert_eq!(
                scene_of(&mut retained, &kept_taffy, retained_bounds),
                scene_of(&mut fresh, &fresh_taffy, fresh_bounds),
                "seed {seed} frame {frame}"
            );
            assert_eq!(
                kept_taffy.total_node_count(),
                nodes.len(),
                "seed {seed} frame {frame}: nodes from earlier frames must be removed"
            );
        }
    }
}

#[test]
fn retained_pass_recovers_when_the_tree_was_cleared_elsewhere() {
    let viewport = Size::new(320.0, 240.0);
    let mut measurer = TextMeasureCache::new();
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut nodes = LayoutNodes::new();
    let mut first = tree(&mut Rng(7), 3);
    FramePipeline::layout_root_retained(
        &mut first,
        &mut taffy,
        &mut nodes,
        &mut measurer,
        viewport,
    )
    .expect("first layout");

    // A one-off layout over the same tree clears it; the kept ids are now stale.
    let mut other = tree(&mut Rng(8), 3);
    FramePipeline::layout_root_with_text_measurer(&mut other, &mut taffy, &mut measurer, viewport)
        .expect("one-off layout");

    let mut again = tree(&mut Rng(7), 3);
    let bounds = FramePipeline::layout_root_retained(
        &mut again,
        &mut taffy,
        &mut nodes,
        &mut measurer,
        viewport,
    )
    .expect("retained layout after clear");
    let mut fresh = tree(&mut Rng(7), 3);
    let mut fresh_taffy = TaffyTree::<ElementId>::new();
    let fresh_bounds = FramePipeline::layout_root_with_text_measurer(
        &mut fresh,
        &mut fresh_taffy,
        &mut measurer,
        viewport,
    )
    .expect("fresh layout");
    assert_eq!(
        scene_of(&mut again, &taffy, bounds),
        scene_of(&mut fresh, &fresh_taffy, fresh_bounds)
    );
}
