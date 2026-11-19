use std::cell::Cell;
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use gpui::{
    AnyView, AppContext, BenchAppContext, Context, DevicePixels, Entity, Hsla, IntoElement,
    ParentElement, Render, Scene, Size, StyleRefinement, Styled, Window, div, px, rgb,
    uniform_list,
};

use super::terminal::Terminal;
use super::workbench_data::*;
use super::{EDITOR_FONT, bench_view, gpu_time};

struct Sidebar {
    renders: Rc<Cell<usize>>,
}

impl Render for Sidebar {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        uniform_list("workbench-sessions", SESSIONS, |indices, _, _| {
            indices
                .map(|index| {
                    div()
                        .w(px(SIDEBAR_WIDTH))
                        .h(px(ROW_HEIGHT))
                        .text_size(px(FONT_SIZE))
                        .line_height(px(ROW_HEIGHT))
                        .font_family(EDITOR_FONT)
                        .text_color(rgb(TEXT_COLOR))
                        .bg(rgb(SIDEBAR_COLORS[index % SIDEBAR_COLORS.len()]))
                        .child(session_text(index))
                })
                .collect::<Vec<_>>()
        })
        .w(px(SIDEBAR_WIDTH))
        .h(px(HEIGHT))
    }
}

struct Workbench {
    sidebar: Entity<Sidebar>,
    terminal: Entity<Terminal>,
    turn: usize,
}

impl Workbench {
    fn new(cx: &mut Context<Self>, renders: Rc<Cell<usize>>) -> Self {
        Self {
            sidebar: cx.new(|_| Sidebar { renders }),
            terminal: cx.new(|cx| Terminal::with_width(cx, PANE_WIDTH)),
            turn: 0,
        }
    }
}

impl Render for Workbench {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .w(px(WIDTH))
            .h(px(HEIGHT))
            .child(
                AnyView::from(self.sidebar.clone()).cached(
                    StyleRefinement::default()
                        .w(px(SIDEBAR_WIDTH))
                        .h(px(HEIGHT)),
                ),
            )
            .child(self.terminal.clone())
    }
}

fn run(cx: &mut BenchAppContext, case: &'static str, sample_gpu: bool, full: bool) {
    let turns = gpu_time::WARMUP_FRAMES + gpu_time::sample_count();
    let renders = Rc::new(Cell::new(0));
    let mut initial_renders = 0;
    bench_view(
        cx,
        case,
        sample_gpu,
        |cx| Workbench::new(cx, Rc::clone(&renders)),
        |view, _, cx| {
            if sample_gpu && view.turn == 0 {
                initial_renders = renders.get();
                assert!(initial_renders > 0, "the sidebar renders at startup");
            }
            view.turn += 1;
            view.terminal.update(cx, |terminal, cx| {
                terminal.turn = view.turn;
                cx.notify();
            });
            if full {
                view.sidebar.update(cx, |_, cx| cx.notify());
                cx.notify();
            }
        },
    );
    if sample_gpu {
        // Like RUI, read the count after the last frame is dispatched.
        let sidebar_renders = renders.get() - initial_renders;
        assert_eq!(sidebar_renders, if full { turns } else { 0 });
        eprintln!("gpui {case} terminal_turn={turns} sidebar_renders={sidebar_renders}");
    }
}

pub(super) fn workbench_pane(cx: &mut BenchAppContext, sample_gpu: bool) {
    run(cx, "workbench_pane", sample_gpu, false);
}

pub(super) fn workbench_full(cx: &mut BenchAppContext, sample_gpu: bool) {
    run(cx, "workbench_full", sample_gpu, true);
}

/// Verifies the submitted geometry and returns a fingerprint of the terminal glyphs.
pub(super) fn inspect_scene(case: &str, scene: &Scene, size: Size<DevicePixels>) -> u64 {
    let scale = 2.0;
    assert_eq!(
        (size.width.0, size.height.0),
        ((WIDTH * scale) as i32, (HEIGHT * scale) as i32)
    );
    let colors = SIDEBAR_COLORS.map(|color| Hsla::from(rgb(color)));
    let mut visible_rows: Vec<_> = scene
        .quads
        .iter()
        .filter(|quad| {
            quad.background
                .as_solid()
                .is_some_and(|color| colors.contains(&color))
        })
        .map(|quad| quad.bounds)
        .collect();
    visible_rows.sort_by(|a, b| a.origin.y.0.total_cmp(&b.origin.y.0));
    assert_eq!(visible_rows.len(), VISIBLE_ROWS);
    for (row, bounds) in visible_rows.iter().enumerate() {
        assert_eq!(
            (
                bounds.origin.x.0,
                bounds.origin.y.0,
                bounds.size.width.0,
                bounds.size.height.0
            ),
            (
                0.0,
                row as f32 * ROW_HEIGHT * scale,
                SIDEBAR_WIDTH * scale,
                ROW_HEIGHT * scale
            ),
        );
    }
    assert!(scene.quads.iter().any(|quad| {
        quad.background.as_solid() == Some(Hsla::from(rgb(PANE_BG)))
            && quad.bounds.origin.x.0 == SIDEBAR_WIDTH * scale
            && quad.bounds.origin.y.0 == 0.0
            && quad.bounds.size.width.0 == PANE_WIDTH * scale
            && quad.bounds.size.height.0 == HEIGHT * scale
    }));
    let mut rows_with_glyphs = BTreeSet::new();
    let mut fingerprint = std::hash::DefaultHasher::new();
    let sprites = scene
        .monochrome_sprites
        .iter()
        .map(|sprite| (sprite.bounds, sprite.tile))
        .chain(
            scene
                .subpixel_sprites
                .iter()
                .map(|sprite| (sprite.bounds, sprite.tile)),
        );
    for (bounds, tile) in sprites.filter(|(bounds, _)| bounds.origin.x.0 >= SIDEBAR_WIDTH * scale) {
        rows_with_glyphs.insert((bounds.origin.y.0 / (ROW_HEIGHT * scale)) as usize);
        (
            bounds.origin.x.0.to_bits(),
            bounds.origin.y.0.to_bits(),
            tile.bounds.origin.x.0,
            tile.bounds.origin.y.0,
        )
            .hash(&mut fingerprint);
    }
    assert!(rows_with_glyphs.into_iter().eq(0..VISIBLE_ROWS));
    let result = fingerprint.finish();
    eprintln!(
        "gpui {case} sidebar_rows={VISIBLE_ROWS} terminal_rows={VISIBLE_ROWS} viewport_width={WIDTH} viewport_height={HEIGHT} terminal_glyph_fingerprint={result:x}"
    );
    result
}
