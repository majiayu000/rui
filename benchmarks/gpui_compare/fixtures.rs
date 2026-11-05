//! GPUI side of the RUI comparison: the fixtures of `benches/compare_frame.rs` in RUI,
//! drawn by GPUI through its headless Metal renderer.
//!
//! `run.sh` in RUI copies this file into `crates/gpui_platform/benches/rui_compare.rs`
//! of a pinned Zed checkout. Keep every fixture identical to RUI's: same content and
//! sizes, in a 1440x900 window at scale 2. Bench names are RUI's case names.
//!
//! - `list_full`: 2000 rows, the root is notified, so every row is rebuilt, laid out
//!   and painted.
//! - `list_full_one_row`: each row is its own view drawn with `.cached(..)`, and only
//!   one row is notified. This is GPUI's own incremental path, written by hand.
//! - `list_virtual_*`: `uniform_list`, scrolled to the middle.
//! - `text_paragraphs`: 200 wrapped paragraphs; one paragraph's text changes.
//! - `quad_grid`: a 200x60 grid of 7x15 cells painted by one `canvas`; one cell changes.
//! - `resize_*`: the window width alternates between 1440 and 1280 each frame.
//! - `startup`: process start to the first long-list frame submitted.
//!
//! The benches are registered as `#[gpui::bench(fps = 120)]` registers them, but with
//! the headless renderer wrapped by [`CountingRenderer`], which counts draw calls.
//! GPU time is not reported: the headless renderer commits its command buffer
//! inside `render_scene` and does not expose it.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use gpui::{
    AnyView, AppContext as _, BenchAppContext, Bounds, Context, DevicePixels, Entity, IntoElement,
    ParentElement, PlatformAtlas, PlatformHeadlessRenderer, PrimitiveBatch, Render, Scene,
    ScrollStrategy, Styled, UniformListScrollHandle, Window, canvas, div, fill, point, px,
    relative, rgb, size, uniform_list,
};

const LIST_ROWS: usize = 2_000;
const ROW_HEIGHT: f32 = 28.;
const VISIBLE_ROWS: usize = 32;
const RESIZED_WIDTH: f32 = 1280.;
const PARAGRAPHS: usize = 200;
const EDITED_PARAGRAPH: usize = 3;
const WORDS: [&str; 16] = [
    "layout",
    "glyph",
    "frame",
    "render",
    "the",
    "a",
    "metal",
    "scene",
    "of",
    "quad",
    "wrapping",
    "paragraph",
    "to",
    "retained",
    "text",
    "atlas",
];
const GRID: (usize, usize) = (200, 60);
const CELL: (f32, f32) = (7., 15.);

/// RUI's text defaults: 14 px, line height 1.4.
fn row_element(index: usize, selected: bool) -> impl IntoElement {
    div()
        .w_full()
        .h(px(ROW_HEIGHT))
        .flex()
        .flex_row()
        .justify_between()
        .text_size(px(14.))
        .line_height(relative(1.4))
        .bg(rgb(if selected { 0x2d3748 } else { 0x1a202c }))
        .child(format!("task {index}: fix the failing check"))
        .child(format!("{} min", index % 60))
}

struct List {
    selected: usize,
}

/// Times `List` was rendered, so `startup` can show its first frame was drawn.
static LIST_RENDERS: AtomicUsize = AtomicUsize::new(0);

impl Render for List {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        LIST_RENDERS.fetch_add(1, Ordering::Relaxed);
        let selected = self.selected;
        div()
            .size_full()
            .flex()
            .flex_col()
            .children((0..LIST_ROWS).map(|index| row_element(index, index == selected)))
    }
}

struct Row {
    index: usize,
    selected: bool,
}

impl Render for Row {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        row_element(self.index, self.selected)
    }
}

struct CachedList {
    rows: Vec<Entity<Row>>,
    turn: usize,
}

impl Render for CachedList {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .children(self.rows.iter().map(|row| {
                AnyView::from(row.clone())
                    .cached(gpui::StyleRefinement::default().w_full().h(px(ROW_HEIGHT)))
            }))
    }
}

struct VirtualList {
    count: usize,
    selected: usize,
    turn: usize,
    scroll: UniformListScrollHandle,
}

impl VirtualList {
    fn new(count: usize) -> Self {
        let scroll = UniformListScrollHandle::new();
        scroll.scroll_to_item(count / 2, ScrollStrategy::Top);
        Self {
            count,
            selected: count / 2,
            turn: 0,
            scroll,
        }
    }
}

impl Render for VirtualList {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected;
        uniform_list("rows", self.count, move |range, _, _| {
            range
                .map(|index| row_element(index, index == selected))
                .collect::<Vec<_>>()
        })
        .track_scroll(&self.scroll)
        .size_full()
    }
}

/// Paragraph `index`: 8 to 127 words, the same on both sides.
fn paragraph(index: usize) -> String {
    let words = 8 + (index * 37) % 120;
    (0..words)
        .map(|word| WORDS[(index * 7 + word * 3) % WORDS.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

struct Paragraphs {
    revision: usize,
}

impl Render for Paragraphs {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let revision = self.revision;
        div()
            .w_full()
            .flex()
            .flex_col()
            .p(px(16.))
            .gap(px(8.))
            .text_size(px(14.))
            .line_height(relative(1.4))
            .children((0..PARAGRAPHS).map(move |index| {
                let mut content = paragraph(index);
                if index == EDITED_PARAGRAPH {
                    content.push_str(&format!(" revision {revision}"));
                }
                content
            }))
    }
}

fn cell_color(row: usize, column: usize) -> u32 {
    (((column * 255 / GRID.0) as u32) << 16) | (((row * 255 / GRID.1) as u32) << 8) | 0x60
}

struct Grid {
    changed: usize,
}

impl Render for Grid {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let changed = self.changed;
        canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                for row in 0..GRID.1 {
                    for column in 0..GRID.0 {
                        let cell = Bounds::new(
                            point(
                                bounds.origin.x + px(column as f32 * CELL.0),
                                bounds.origin.y + px(row as f32 * CELL.1),
                            ),
                            size(px(CELL.0), px(CELL.1)),
                        );
                        let color = if row * GRID.0 + column == changed {
                            0xffffff
                        } else {
                            cell_color(row, column)
                        };
                        window.paint_quad(fill(cell, rgb(color)));
                    }
                }
            },
        )
        .w(px(GRID.0 as f32 * CELL.0))
        .h(px(GRID.1 as f32 * CELL.1))
    }
}

unsafe extern "C" {
    fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut u64) -> i32;
    fn mach_absolute_time() -> u64;
    fn mach_timebase_info(info: *mut [u32; 2]) -> i32;
}

/// `rusage_info_v4` read as words, as RUI's `compare_frame` reads it: index 9 is
/// `ri_phys_footprint`, 10 `ri_proc_start_abstime`.
fn rusage() -> [u64; 64] {
    let mut words = [0u64; 64];
    // SAFETY: flavor 4 writes a rusage_info_v4, far shorter than 64 words.
    let status = unsafe { proc_pid_rusage(std::process::id() as i32, 4, words.as_mut_ptr()) };
    assert_eq!(status, 0, "proc_pid_rusage failed");
    words
}

fn footprint_mb() -> f64 {
    rusage()[9] as f64 / 1_048_576.0
}

/// Milliseconds since the kernel started this process.
fn ms_since_process_start() -> f64 {
    let mut timebase = [0u32; 2];
    // SAFETY: writes a mach_timebase_info_data_t, two u32s.
    assert_eq!(unsafe { mach_timebase_info(&mut timebase) }, 0);
    // SAFETY: no preconditions.
    let ticks = unsafe { mach_absolute_time() }.saturating_sub(rusage()[10]);
    ticks as f64 * timebase[0] as f64 / timebase[1] as f64 / 1e6
}

fn viewport() -> gpui::Size<gpui::Pixels> {
    size(px(1440.), px(900.))
}

/// Runs `update` once per frame on a fresh window showing `view`.
fn bench_view<V: Render>(
    cx: &mut BenchAppContext,
    case: &str,
    view: impl FnOnce(&mut Context<V>) -> V,
    update: impl FnMut(&mut V, &mut Window, &mut Context<V>),
) {
    let mut window = cx.add_empty_window();
    window.simulate_resize(viewport());
    let root = window.update(|window, cx| window.replace_root(cx, |_, cx| view(cx)));
    cx.run_until_idle();
    cx.bench_renderer(root, update);
    eprintln!("gpui {case} footprint_mb={:.1}", footprint_mb());
    window.update(|window, _| window.remove_window());
    cx.run_until_idle();
}

/// What the platform's resize callback does, from inside a frame update.
fn alternate_width(turn: &mut usize, window: &mut Window, cx: &mut gpui::App) {
    *turn += 1;
    let width = if *turn % 2 == 0 { 1440. } else { RESIZED_WIDTH };
    window.resize(size(px(width), px(900.)));
    window.bounds_changed(cx);
}

fn startup(cx: &mut BenchAppContext) {
    static MEASURED: AtomicBool = AtomicBool::new(false);
    let to_fixture = ms_since_process_start();
    let t0 = Instant::now();
    let mut window = cx.add_empty_window();
    window.simulate_resize(viewport());
    let root = window.update(|window, cx| window.replace_root(cx, |_, _| List { selected: 0 }));
    cx.run_until_idle();
    // `bench_renderer` draws the pending first frame before its first update.
    cx.bench_renderer(root, |_, _, _| {
        if !MEASURED.swap(true, Ordering::Relaxed) {
            assert!(LIST_RENDERS.load(Ordering::Relaxed) > 0, "first frame not drawn yet");
            eprintln!(
                "gpui startup process_to_fixture_ms={to_fixture:.1} first_frame_ms={:.1} process_to_first_frame_ms={:.1} footprint_mb={:.1}",
                t0.elapsed().as_secs_f64() * 1e3,
                ms_since_process_start(),
                footprint_mb()
            );
        }
    });
    window.update(|window, _| window.remove_window());
    cx.run_until_idle();
}

fn list_full(cx: &mut BenchAppContext) {
    bench_view(
        cx,
        "list_full",
        |_| List { selected: 0 },
        |list, _, cx| {
            list.selected = (list.selected + 1) % LIST_ROWS;
            cx.notify();
        },
    );
}

fn list_full_one_row(cx: &mut BenchAppContext) {
    bench_view(
        cx,
        "list_full_one_row",
        |cx| CachedList {
            rows: (0..LIST_ROWS)
                .map(|index| {
                    cx.new(|_| Row {
                        index,
                        selected: index == 0,
                    })
                })
                .collect(),
            turn: 0,
        },
        |list, _, cx| {
            let row = list.rows[list.turn % list.rows.len()].clone();
            list.turn += 1;
            row.update(cx, |row, cx| {
                row.selected = !row.selected;
                cx.notify();
            });
        },
    );
}

fn virtual_list(cx: &mut BenchAppContext, case: &str, count: usize) {
    bench_view(
        cx,
        case,
        |_| VirtualList::new(count),
        |list, _, cx| {
            list.turn += 1;
            list.selected = list.count / 2 + list.turn % VISIBLE_ROWS;
            cx.notify();
        },
    );
}

fn list_virtual_1k(cx: &mut BenchAppContext) {
    virtual_list(cx, "list_virtual_1k", 1_000);
}

fn list_virtual_100k(cx: &mut BenchAppContext) {
    virtual_list(cx, "list_virtual_100k", 100_000);
}

fn text_paragraphs(cx: &mut BenchAppContext) {
    bench_view(
        cx,
        "text_paragraphs",
        |_| Paragraphs { revision: 0 },
        |page, _, cx| {
            page.revision += 1;
            cx.notify();
        },
    );
}

fn quad_grid(cx: &mut BenchAppContext) {
    bench_view(
        cx,
        "quad_grid",
        |_| Grid { changed: 0 },
        |grid, _, cx| {
            grid.changed = (grid.changed + 1) % (GRID.0 * GRID.1);
            cx.notify();
        },
    );
}

fn resize_full(cx: &mut BenchAppContext) {
    let mut turn = 0;
    bench_view(
        cx,
        "resize_full",
        |_| List { selected: 0 },
        |_, window, cx| alternate_width(&mut turn, window, cx),
    );
}

fn resize_virtual(cx: &mut BenchAppContext) {
    let mut turn = 0;
    bench_view(
        cx,
        "resize_virtual",
        |_| VirtualList::new(1_000),
        |_, window, cx| alternate_width(&mut turn, window, cx),
    );
}

/// Batches and instances of each scene rendered since the last [`take_draws`].
static SCENES: std::sync::Mutex<Vec<(u64, u64)>> = std::sync::Mutex::new(Vec::new());
/// Scenes with at least one glyph since the last check in [`counted`].
static FRAMES_WITH_GLYPHS: AtomicUsize = AtomicUsize::new(0);

/// GPUI's headless Metal renderer, counting what each scene asks of the GPU. Its Metal
/// renderer issues one instanced draw per `Scene::batches()` entry; paths and surfaces
/// would add more, but no fixture draws them.
struct CountingRenderer(Box<dyn PlatformHeadlessRenderer>);

impl CountingRenderer {
    fn count(scene: &Scene) {
        let (mut batches, mut instances) = (0, 0);
        for batch in scene.batches() {
            batches += 1;
            instances += match batch {
                PrimitiveBatch::Shadows(range)
                | PrimitiveBatch::Quads(range)
                | PrimitiveBatch::Paths(range)
                | PrimitiveBatch::Underlines(range)
                | PrimitiveBatch::Surfaces(range)
                | PrimitiveBatch::MonochromeSprites { range, .. }
                | PrimitiveBatch::SubpixelSprites { range, .. }
                | PrimitiveBatch::PolychromeSprites { range, .. } => range.len() as u64,
            };
        }
        SCENES.lock().unwrap().push((batches, instances));
        if !scene.monochrome_sprites.is_empty() || !scene.subpixel_sprites.is_empty() {
            FRAMES_WITH_GLYPHS.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl PlatformHeadlessRenderer for CountingRenderer {
    fn render_scene_to_image(
        &mut self,
        scene: &Scene,
        size: gpui::Size<DevicePixels>,
    ) -> anyhow::Result<image::RgbaImage> {
        Self::count(scene);
        self.0.render_scene_to_image(scene, size)
    }

    fn render_scene(
        &mut self,
        scene: &Scene,
        size: gpui::Size<DevicePixels>,
    ) -> anyhow::Result<()> {
        Self::count(scene);
        self.0.render_scene(scene, size)
    }

    fn sprite_atlas(&self) -> std::sync::Arc<dyn PlatformAtlas> {
        self.0.sprite_atlas()
    }
}

/// Median draw calls and instances per scene since the last call. The median skips
/// the empty first scene of each new window.
fn take_draws() -> (u64, u64) {
    let mut scenes = std::mem::take(&mut *SCENES.lock().unwrap());
    if scenes.is_empty() {
        return (0, 0);
    }
    let middle = scenes.len() / 2;
    let batches = *scenes.select_nth_unstable_by_key(middle, |scene| scene.0).1;
    let instances = *scenes.select_nth_unstable_by_key(middle, |scene| scene.1).1;
    (batches.0, instances.1)
}

/// What `#[gpui::bench(fps = 120)]` generates for `body`, with [`CountingRenderer`].
fn counted(
    criterion: &mut criterion::Criterion<gpui::BenchMeasurement>,
    name: &'static str,
    body: fn(&mut BenchAppContext),
) {
    let report = gpui::BenchReport::with_fps(120);
    take_draws();
    FRAMES_WITH_GLYPHS.store(0, Ordering::Relaxed);
    criterion.bench_function(name, {
        let report = report.clone();
        move |bencher| {
            let mut cx = BenchAppContext::new_with_platform_and_report(
                gpui::bench_platform(
                    Some(Box::new(|| {
                        Ok(gpui_platform::current_headless_renderer()?.map(|renderer| {
                            Box::new(CountingRenderer(renderer))
                                as Box<dyn PlatformHeadlessRenderer>
                        }))
                    })),
                    gpui_platform::bench_text_system(),
                ),
                Some(name),
                bencher,
                report.clone(),
            );
            body(&mut cx);
            cx.teardown();
        }
    });
    report.print(name);
    // Criterion skips benches its filter does not select; they render nothing.
    if SCENES.lock().unwrap().is_empty() {
        return;
    }
    let (draw_calls, instances) = take_draws();
    // Every fixture but the grid draws text, which must reach the scene as glyphs.
    // Built without `font-kit`, GPUI uses `NoopTextSystem` and draws none.
    if name != "quad_grid" && FRAMES_WITH_GLYPHS.load(Ordering::Relaxed) == 0 {
        panic!("{name}: GPUI drew no glyphs; build gpui_platform with the font-kit feature");
    }
    eprintln!("gpui {name} draw_calls={draw_calls} instances={instances}");
}

macro_rules! counted_benches {
    ($($target:ident => $body:ident),* $(,)?) => {
        $(
            fn $target(criterion: &mut criterion::Criterion<gpui::BenchMeasurement>) {
                counted(criterion, stringify!($body), $body);
            }
        )*
    };
}

counted_benches!(
    bench_startup => startup,
    bench_list_full => list_full,
    bench_list_full_one_row => list_full_one_row,
    bench_list_virtual_1k => list_virtual_1k,
    bench_list_virtual_100k => list_virtual_100k,
    bench_text_paragraphs => text_paragraphs,
    bench_quad_grid => quad_grid,
    bench_resize_full => resize_full,
    bench_resize_virtual => resize_virtual,
);

gpui::bench_group!(
    name = benches;
    config = criterion::Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(20);
    targets = bench_startup, bench_list_full, bench_list_full_one_row, bench_list_virtual_1k,
        bench_list_virtual_100k, bench_text_paragraphs, bench_quad_grid, bench_resize_full,
        bench_resize_virtual
);
gpui::bench_main!(benches);
