//! Random trees and edits for the incremental oracles, and a runner that
//! keeps layout and paint across frames as the frame pipeline does.
#![allow(dead_code)]

use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::LayoutNodes;
use rui::elements::{Div, Element, div, text};
use rui::renderer::Scene;
use rui::renderer::text::TextMeasureCache;
use rui::{Color, ElementId, Size};
use taffy::prelude::TaffyTree;

/// xorshift64*: deterministic without a dependency.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    pub fn below(&mut self, n: u64) -> u64 {
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
pub enum Node {
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
    pub fn random(rng: &mut Rng, depth: u32) -> Node {
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

    pub fn build(&self) -> Div {
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
    pub fn count(&self) -> usize {
        match self {
            Node::Box { children, .. } => 1 + children.iter().map(Node::count).sum::<usize>(),
            Node::Label { .. } => 1,
        }
    }

    pub fn nth(&mut self, n: usize) -> &mut Node {
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

    pub fn edit(&mut self, rng: &mut Rng) {
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
pub struct Incremental {
    pub root: Div,
    taffy: TaffyTree<ElementId>,
    nodes: LayoutNodes,
    pub scene: Scene,
}

impl Incremental {
    pub fn new() -> Self {
        Self {
            root: div(),
            taffy: TaffyTree::new(),
            nodes: LayoutNodes::new(),
            scene: Scene::new(),
        }
    }

    /// Rebuild as the frame pipeline does: keep what reconciles.
    pub fn rebuild(&mut self, mut next: Div) {
        if !next.reconcile(&mut self.root) {
            self.root = next;
        }
    }

    /// Lay out and paint the kept tree.
    pub fn frame(&mut self, measurer: &mut TextMeasureCache, viewport: Size) {
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
    }
}
