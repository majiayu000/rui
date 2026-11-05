//! Incremental frames must draw exactly what a full frame draws.
//!
//! Each frame rebuilds a randomly edited tree. The incremental side keeps the
//! unchanged parts of the previous tree (`Element::reconcile`), their layout
//! nodes and their paint; the full side lays out and paints a fresh tree from
//! scratch. Their scenes must match primitive for primitive.

use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::LayoutNodes;
use rui::elements::{Div, Element, canvas, div, text};
use rui::renderer::text::TextMeasureCache;
use rui::renderer::{Primitive, Scene};
use rui::{Bounds, Color, ElementId, Size};
use std::cell::Cell;
use std::rc::Rc;
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

const WORDS: [&str; 6] = [
    "a",
    "row",
    "longer label",
    "x y z",
    "",
    "a paragraph long enough to wrap onto several lines in a narrow box",
];

/// The data a frame's tree is built from; edits change it between frames.
#[derive(Clone)]
enum Node {
    Box {
        color: u32,
        row: bool,
        gap: f32,
        width: Option<f32>,
        height: Option<f32>,
        text_color: Option<u32>,
        children: Vec<Node>,
    },
    Label {
        word: usize,
        size: f32,
        wrap: bool,
    },
}

impl Node {
    fn random(rng: &mut Rng, depth: u32) -> Node {
        if depth == 0 || rng.below(4) == 0 {
            return Node::Label {
                word: rng.below(WORDS.len() as u64) as usize,
                size: 10.0 + rng.below(8) as f32,
                wrap: rng.below(3) == 0,
            };
        }
        Node::Box {
            color: rng.below(0xff_ffff) as u32,
            row: rng.below(2) == 0,
            gap: rng.below(6) as f32,
            width: (rng.below(3) == 0).then(|| 20.0 + rng.below(200) as f32),
            height: (rng.below(3) == 0).then(|| 10.0 + rng.below(60) as f32),
            text_color: (rng.below(4) == 0).then(|| rng.below(0xff_ffff) as u32),
            children: (0..rng.below(5))
                .map(|_| Node::random(rng, depth - 1))
                .collect(),
        }
    }

    fn build(&self) -> Div {
        match self {
            Node::Box {
                color,
                row,
                gap,
                width,
                height,
                text_color,
                children,
            } => {
                let mut node = div().bg(Color::hex(*color)).gap(*gap);
                node = if *row {
                    node.flex_row()
                } else {
                    node.flex_col()
                };
                if let Some(width) = width {
                    node = node.w(*width);
                }
                if let Some(height) = height {
                    node = node.h(*height);
                }
                if let Some(text_color) = text_color {
                    node = node.text_color(Color::hex(*text_color));
                }
                node.children(children.iter().map(Node::build))
            }
            Node::Label { word, size, wrap } => {
                let label = text(WORDS[*word]).size(*size);
                // Text sizes are fractional, so siblings after it move by
                // fractions of a pixel when it changes.
                div()
                    .flex_row()
                    .child(if *wrap { label.wrap() } else { label })
            }
        }
    }

    /// Every node, depth first, for picking one to edit.
    fn count(&self) -> usize {
        match self {
            Node::Box { children, .. } => 1 + children.iter().map(Node::count).sum::<usize>(),
            Node::Label { .. } => 1,
        }
    }

    fn nth(&mut self, n: usize) -> &mut Node {
        if n == 0 {
            return self;
        }
        let mut n = n - 1;
        let Node::Box { children, .. } = self else {
            unreachable!("labels have no descendants")
        };
        for child in children {
            let count = child.count();
            if n < count {
                return child.nth(n);
            }
            n -= count;
        }
        unreachable!("n is below the node count")
    }

    fn edit(&mut self, rng: &mut Rng) {
        match self {
            Node::Box {
                color,
                height,
                text_color,
                children,
                ..
            } => match rng.below(6) {
                0 => *color = rng.below(0xff_ffff) as u32,
                1 => *height = (rng.below(2) == 0).then(|| 10.0 + rng.below(60) as f32),
                2 => *text_color = (rng.below(2) == 0).then(|| rng.below(0xff_ffff) as u32),
                3 if !children.is_empty() => {
                    children.remove(rng.below(children.len() as u64) as usize);
                }
                _ => {
                    let at = rng.below(children.len() as u64 + 1) as usize;
                    children.insert(at, Node::random(rng, 2));
                }
            },
            Node::Label { word, size, wrap } => match rng.below(3) {
                0 => *word = rng.below(WORDS.len() as u64) as usize,
                1 => *size = 10.0 + rng.below(8) as f32,
                _ => *wrap = !*wrap,
            },
        }
    }
}

/// Layout and paint state an incremental runner keeps across frames.
struct Incremental {
    root: Div,
    taffy: TaffyTree<ElementId>,
    nodes: LayoutNodes,
    scene: Scene,
}

impl Incremental {
    fn new() -> Self {
        Self {
            root: div(),
            taffy: TaffyTree::new(),
            nodes: LayoutNodes::new(),
            scene: Scene::new(),
        }
    }

    /// Rebuild as the frame pipeline does: keep what reconciles.
    fn rebuild(&mut self, mut next: Div) {
        if !next.reconcile(&mut self.root) {
            self.root = next;
        }
    }

    fn frame(&mut self, measurer: &mut TextMeasureCache, viewport: Size) -> Vec<String> {
        let bounds = match FramePipeline::layout_root_retained(
            &mut self.root,
            &mut self.taffy,
            &mut self.nodes,
            measurer,
            viewport,
        ) {
            Ok(bounds) => bounds,
            Err(err) => panic!("retained layout failed: {err}"),
        };
        FramePipeline::paint_root(&mut self.root, &self.taffy, &mut self.scene, bounds);
        describe(self.scene.primitives())
    }
}

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
    describe(scene.primitives())
}

/// One line per primitive, so a mismatch names the first one that differs.
fn describe(primitives: &[Primitive]) -> Vec<String> {
    primitives
        .iter()
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
            let kept = incremental.frame(&mut measurer, viewport);
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
        let kept = incremental.frame(&mut measurer, viewport);
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
        let kept = incremental.frame(&mut measurer, viewport);
        let full = full_frame(tree(color), &mut measurer, viewport);
        assert_same(&kept, &full, &format!("color {color:06x}"));
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
        let kept = incremental.frame(&mut measurer, viewport);
        let full = full_frame(tree(&shade), &mut measurer, viewport);
        assert_same(&kept, &full, &format!("step {step}"));
    }
    assert!(matches!(
        incremental.scene.primitives().last(),
        Some(Primitive::PopClip)
    ));
}
