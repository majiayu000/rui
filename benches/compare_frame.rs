//! RUI side of the GPUI comparison. Prints one line per case.
//!
//! `cargo bench --bench compare_frame [-- <case>...]` runs the named cases, or all of
//! them. `benchmarks/gpui_compare/run.sh` runs each case in its own process, as it does
//! for GPUI, and prints both sides in one table.
//!
//! Every fixture matches `benchmarks/gpui_compare/fixtures.rs`: same content and sizes,
//! a 1440x900 window at scale 2. Each frame changes one thing, then rebuilds, lays out
//! against the kept layout tree, paints what the window shows, as the native runner
//! does, and submits to Metal without waiting, as GPUI's headless renderer does.
//! Instructions are the whole process's retired instructions,
//! as GPUI's `BENCH_MEASUREMENT=instructions` counts; allocations are counted by this
//! binary's allocator, as `gpui::bench_main!` installs one.
//!
//! Draw calls and instances come from `MetalRenderer::last_frame_stats`. GPU time comes
//! from the same sampled frames through `benchmarks/gpui_compare/gpu_time.rs`, which
//! GPUI's side uses too: each committed command buffer's `GPUEndTime - GPUStartTime`.
use rui::core::entity::Entity;
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::{Element, LayoutNodes};
use rui::elements::{VirtualListHandle, shared_styled_text, span_range, virtual_list};
use rui::renderer::metal::MetalRenderer;
use rui::renderer::text::TextMeasureCache;
use rui::renderer::{RendererResourceLimits, Scene};
use rui::{AppContext, Color, ElementId, Size, div, text};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use taffy::prelude::TaffyTree;

const CASES: [&str; 15] = [
    "startup",
    "list_full",
    "list_full_rows",
    "list_virtual_1k",
    "list_virtual_100k",
    "text_paragraphs",
    "quad_grid",
    "quad_grid_cached",
    "resize_full",
    "resize_virtual",
    "list_full_one_row",
    "editor_scroll",
    "editor_edit",
    "paths",
    "paths_icons",
];
mod compare_grid;
mod compare_paths;
use compare_grid::{CELL, ChangedCell, GRID, changed_cell, quad_grid, static_grid};
#[path = "../benchmarks/gpui_compare/gpu_time.rs"]
mod gpu_time;

const WARMUP: usize = 10;
/// Long enough for the last frames' command buffers to complete.
const GPU_SETTLE: std::time::Duration = std::time::Duration::from_millis(100);
const SAMPLES: usize = 60;
const VIEWPORT: (f32, f32) = (1440.0, 900.0);
/// The second width of the resize cases; the height stays.
const RESIZED_WIDTH: f32 = 1280.0;
const SCALE_FACTOR: f32 = 2.0;
const LIST_ROWS: usize = 2_000;
const ROW_HEIGHT: f32 = 28.0;
/// Rows the virtual list shows, so the selection stays on screen.
const VISIBLE_ROWS: usize = 32;
const PARAGRAPHS: usize = 200;
/// The paragraph whose text changes each frame; on screen.
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
const EDITOR_LINES: usize = 10_000;
const EDITOR_LINE_HEIGHT: f32 = 20.0;
const EDITOR_FONT_SIZE: f32 = 13.0;
const EDITOR_FONT: &str = "SF Mono";
const GUTTER_WIDTH: f32 = 64.0;
/// The first line on screen when an editor case starts.
const EDITOR_TOP: usize = 5_000;
/// The cursor's line, counted from the first line on screen.
const CURSOR_ROW: usize = 10;
const KEYWORDS: [&str; 8] = ["fn", "let", "pub", "if", "match", "return", "for", "impl"];
const IDENTS: [&str; 12] = [
    "frame", "layout", "glyph", "scene", "cursor", "buffer", "row", "offset", "width", "atlas",
    "span", "view",
];

struct CountingAllocator;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

// SAFETY: forwards every call to `System` unchanged; only counts allocations.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

unsafe extern "C" {
    fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut u64) -> i32;
    fn mach_absolute_time() -> u64;
    fn mach_timebase_info(info: *mut [u32; 2]) -> i32;
}

/// `rusage_info_v4` read as words: index 9 is `ri_phys_footprint`, 10
/// `ri_proc_start_abstime`, 31 `ri_instructions`.
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

fn row(index: usize, selected: usize) -> rui::Div {
    row_with(index, index == selected)
}

fn row_with(index: usize, selected: bool) -> rui::Div {
    let background = if selected {
        Color::hex(0x2d3748)
    } else {
        Color::hex(0x1a202c)
    };
    div()
        .w_full()
        .h(ROW_HEIGHT)
        .flex_row()
        .justify_between()
        .bg(background)
        .child(text(format!("task {index}: fix the failing check")))
        .child(text(format!("{} min", index % 60)))
}

/// A row of `list_full_one_row`, kept in an entity.
struct ListRow {
    index: usize,
    selected: bool,
}

fn list(selected: usize) -> impl Element {
    div()
        .w_full()
        .flex_col()
        .children((0..LIST_ROWS).map(move |index| row(index, selected)))
}

fn virtual_rows(handle: &VirtualListHandle, count: usize, selected: usize) -> impl Element {
    virtual_list(handle, count, ROW_HEIGHT, move |index| row(index, selected))
        .w_full()
        .h_full()
}

/// Paragraph `index`: 8 to 127 words, the same on both sides.
fn paragraph(index: usize) -> String {
    let words = 8 + (index * 37) % 120;
    (0..words)
        .map(|word| WORDS[(index * 7 + word * 3) % WORDS.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

fn paragraphs(revision: usize) -> impl Element {
    div()
        .w_full()
        .flex_col()
        .p(16.0)
        .gap(8.0)
        .children((0..PARAGRAPHS).map(move |index| {
            let mut content = paragraph(index);
            if index == EDITED_PARAGRAPH {
                content.push_str(&format!(" revision {revision}"));
            }
            text(content).size(14.0).line_height(1.4).wrap()
        }))
}

#[derive(Clone, Copy)]
enum Token {
    Plain,
    Keyword,
    Ident,
    Number,
    Str,
    Comment,
}

impl Token {
    fn color(self) -> u32 {
        match self {
            Token::Plain => 0xabb2bf,
            Token::Keyword => 0xc678dd,
            Token::Ident => 0xe5c07b,
            Token::Number => 0xd19a66,
            Token::Str => 0x98c379,
            Token::Comment => 0x7f848e,
        }
    }
}

/// Line `index` of the code view as colored tokens, the same on both sides:
/// a comment every ninth line, otherwise a statement of eleven tokens.
fn code_line(index: usize) -> Vec<(String, Token)> {
    let indent = " ".repeat(index % 4 * 4);
    let ident = |n: usize| IDENTS[(index * 7 + n * 5) % IDENTS.len()];
    if index.is_multiple_of(9) {
        let words = (0..4).map(ident).collect::<Vec<_>>().join(" ");
        return vec![(format!("{indent}// {words}"), Token::Comment)];
    }
    vec![
        (indent, Token::Plain),
        (KEYWORDS[index % KEYWORDS.len()].to_string(), Token::Keyword),
        (" ".to_string(), Token::Plain),
        (ident(0).to_string(), Token::Ident),
        (" = ".to_string(), Token::Plain),
        (ident(1).to_string(), Token::Ident),
        ("(".to_string(), Token::Plain),
        ((index * 31 % 1000).to_string(), Token::Number),
        (", ".to_string(), Token::Plain),
        (format!("\"{}\"", ident(2)), Token::Str),
        (");".to_string(), Token::Plain),
    ]
}

/// The one-character edit of frame `turn`: the line's first identifier, or its
/// comment, ends in a letter that changes every frame.
fn edit_line(line: &mut [(String, Token)], turn: usize) {
    let letter = char::from(b'a' + (turn % 26) as u8);
    let token = match line.get_mut(3) {
        Some(token) => token,
        None => &mut line[0],
    };
    token.0.pop();
    token.0.push(letter);
}

/// A line as an editor keeps it: one string and its tokens as byte ranges.
struct CodeLine {
    text: Arc<str>,
    tokens: Vec<(Range<usize>, Token)>,
}

impl CodeLine {
    fn new(tokens: &[(String, Token)]) -> Self {
        let mut text = String::new();
        let tokens = tokens
            .iter()
            .map(|(content, token)| {
                let start = text.len();
                text.push_str(content);
                (start..text.len(), *token)
            })
            .collect();
        Self {
            text: text.into(),
            tokens,
        }
    }
}

/// The document as tokens, which edits change, and as the lines drawn from them.
struct Document {
    tokens: Vec<Vec<(String, Token)>>,
    lines: Vec<CodeLine>,
}

type SharedDocument = Rc<RefCell<Document>>;

/// A 10 000-line code view: a gutter of line numbers, per-token colors as styled
/// runs in a monospace font, and the cursor's line highlighted.
fn code_view(handle: &VirtualListHandle, document: &SharedDocument, cursor: usize) -> impl Element {
    let document = Rc::clone(document);
    let lines = virtual_list(handle, EDITOR_LINES, EDITOR_LINE_HEIGHT, move |index| {
        let line = &document.borrow().lines[index];
        let spans = line
            .tokens
            .iter()
            .map(|(bytes, token)| span_range(bytes.clone()).color(Color::hex(token.color())));
        div()
            .w_full()
            .h(EDITOR_LINE_HEIGHT)
            .flex_row()
            .bg(Color::hex(if index == cursor {
                0x2c313c
            } else {
                0x21252b
            }))
            .child(
                div()
                    .w(GUTTER_WIDTH)
                    .pr(12.0)
                    .flex_row()
                    .justify_end()
                    .child(text((index + 1).to_string()).color(Color::hex(0x636d83))),
            )
            .child(shared_styled_text(Arc::clone(&line.text), spans))
    })
    .w_full()
    .h_full();
    div()
        .w_full()
        .h_full()
        .font_family(EDITOR_FONT)
        .text_size(EDITOR_FONT_SIZE)
        .line_height(1.5)
        .child(lines)
}

/// The first line on screen at frame `turn` of `editor_scroll`, wrapping before the end.
fn scrolled_top(turn: usize) -> usize {
    EDITOR_TOP + turn % (EDITOR_LINES - EDITOR_TOP - 100)
}

fn document() -> SharedDocument {
    let tokens: Vec<_> = (0..EDITOR_LINES).map(code_line).collect();
    let lines = tokens.iter().map(|line| CodeLine::new(line)).collect();
    Rc::new(RefCell::new(Document { tokens, lines }))
}

/// What one runtime keeps across frames: layout tree, measure cache, scene, renderer.
struct Harness {
    taffy: TaffyTree<ElementId>,
    nodes: LayoutNodes,
    scene: Scene,
    measurer: TextMeasureCache,
    renderer: MetalRenderer,
}

impl Harness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            taffy: TaffyTree::new(),
            nodes: LayoutNodes::new(),
            scene: Scene::new(),
            measurer: TextMeasureCache::new(),
            renderer: MetalRenderer::new()?,
        })
    }

    fn frame(
        &mut self,
        mut root: impl Element,
        viewport: Size,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // The native runner drains a pool per event; without one, Metal's autoreleased
        // objects pile up for the whole run.
        objc2::rc::autoreleasepool(|_| self.draw(&mut root, viewport))
    }

    /// A frame of `context`'s views: rebuilds only what was notified.
    fn app_frame<E: Element>(
        &mut self,
        context: &mut AppContext,
        root: &mut E,
        build: &mut impl FnMut(&mut AppContext) -> E,
        viewport: Size,
    ) -> Result<(), Box<dyn std::error::Error>> {
        objc2::rc::autoreleasepool(|_| {
            FramePipeline::build_frame_retained(
                context,
                root,
                build,
                &mut self.taffy,
                &mut self.nodes,
                &mut self.scene,
                &mut self.measurer,
                viewport,
            )?;
            self.renderer
                .render_offscreen(&self.scene, viewport, SCALE_FACTOR)?;
            Ok(())
        })
    }

    fn draw(
        &mut self,
        root: &mut impl Element,
        viewport: Size,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bounds = FramePipeline::layout_root_retained(
            root,
            &mut self.taffy,
            &mut self.nodes,
            &mut self.measurer,
            viewport,
        )?;
        FramePipeline::paint_root_in_viewport(root, &self.taffy, &mut self.scene, bounds, viewport);
        self.renderer
            .render_offscreen(&self.scene, viewport, SCALE_FACTOR)?;
        Ok(())
    }
}

fn percentile(values: &mut [f64], fraction: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len() - 1) as f64 * fraction).round() as usize]
}

/// Runs `frame(harness, turn)` after warm-up and prints the per-frame costs. No frame
/// waits for the GPU; GPU time is read from each frame's command buffer once done.
fn measure(
    case: &str,
    harness: &mut Harness,
    mut frame: impl FnMut(&mut Harness, usize) -> Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // `COMPARE_SAMPLES` measures more frames, to profile the steady state.
    let samples = std::env::var("COMPARE_SAMPLES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(SAMPLES);
    gpu_time::install();
    for turn in 0..WARMUP {
        frame(harness, turn)?;
    }
    // Let the warm-up frames' completed handlers run.
    std::thread::sleep(GPU_SETTLE);
    gpu_time::take();
    let (mut ms, mut instructions, mut allocations) = (Vec::new(), Vec::new(), Vec::new());
    let (mut draw_calls, mut instances) = (Vec::new(), Vec::new());
    for turn in WARMUP..WARMUP + samples {
        let (i0, a0) = (rusage()[31], ALLOCATIONS.load(Ordering::Relaxed));
        let t0 = Instant::now();
        frame(harness, turn)?;
        ms.push(t0.elapsed().as_secs_f64() * 1e3);
        instructions.push(rusage()[31].saturating_sub(i0) as f64 / 1e6);
        allocations.push(ALLOCATIONS.load(Ordering::Relaxed).saturating_sub(a0) as f64);
        let stats = harness.renderer.last_frame_stats();
        draw_calls.push(f64::from(stats.draw_calls));
        instances.push(f64::from(stats.instances));
    }
    let footprint = footprint_mb();
    std::thread::sleep(GPU_SETTLE);
    let mut gpu_ms = gpu_time::take();
    if gpu_ms.len() != samples {
        return Err(format!(
            "{case}: {} command buffers completed for {samples} frames; GPU time per frame needs one each",
            gpu_ms.len()
        )
        .into());
    }
    println!(
        "rui {case} instructions_m={:.2} ms_p50={:.3} ms_p95={:.3} allocations={:.0} footprint_mb={footprint:.1} draw_calls={:.0} instances={:.0} gpu_ms_p50={:.3} gpu_ms_p95={:.3}",
        percentile(&mut instructions, 0.5),
        percentile(&mut ms, 0.5),
        percentile(&mut ms, 0.95),
        percentile(&mut allocations, 0.5),
        percentile(&mut draw_calls, 0.5),
        percentile(&mut instances, 0.5),
        percentile(&mut gpu_ms, 0.5),
        percentile(&mut gpu_ms, 0.95),
    );
    Ok(())
}

fn run(case: &str) -> Result<(), Box<dyn std::error::Error>> {
    let viewport = Size::new(VIEWPORT.0, VIEWPORT.1);
    let resized = |turn: usize| {
        if turn.is_multiple_of(2) {
            viewport
        } else {
            Size::new(RESIZED_WIDTH, VIEWPORT.1)
        }
    };
    match case {
        // From before the renderer exists to the first long-list frame submitted.
        // As the native runner starts: the renderer is created on another thread
        // while the first frame is built, laid out and painted, and the first
        // present waits for it. `renderer_wait_ms` is what that wait still took.
        "startup" => {
            let to_fixture = ms_since_process_start();
            let t0 = Instant::now();
            let pending = MetalRenderer::spawn(RendererResourceLimits::default());
            let (mut taffy, mut nodes, mut scene) =
                (TaffyTree::new(), LayoutNodes::new(), Scene::new());
            let mut measurer = TextMeasureCache::new();
            let mut root = list(0);
            let bounds = FramePipeline::layout_root_retained(
                &mut root,
                &mut taffy,
                &mut nodes,
                &mut measurer,
                viewport,
            )?;
            FramePipeline::paint_root(&mut root, &taffy, &mut scene, bounds);
            let laid_out = Instant::now();
            let mut renderer = pending.wait()?;
            let renderer_wait = laid_out.elapsed().as_secs_f64() * 1e3;
            objc2::rc::autoreleasepool(|_| {
                renderer.render_offscreen(&scene, viewport, SCALE_FACTOR)
            })?;
            let first_frame = t0.elapsed().as_secs_f64() * 1e3;
            println!(
                "rui startup process_to_fixture_ms={to_fixture:.1} first_frame_ms={first_frame:.1} process_to_first_frame_ms={:.1} footprint_mb={:.1} renderer_wait_ms={renderer_wait:.1}",
                ms_since_process_start(),
                footprint_mb()
            );
        }
        "list_full" => {
            let mut harness = Harness::new()?;
            measure(case, &mut harness, |harness, turn| {
                harness.frame(list(turn % LIST_ROWS), viewport)
            })?;
        }
        // Each row is a view of its own entity, as GPUI's `.cached()` rows are, and
        // only the row that toggles is notified; the list around it is not rebuilt.
        "list_full_one_row" => {
            let mut harness = Harness::new()?;
            let mut context = AppContext::new();
            let rows: Vec<Entity<ListRow>> = (0..LIST_ROWS)
                .map(|index| {
                    context.create(ListRow {
                        index,
                        selected: index == 0,
                    })
                })
                .collect();
            let items = rows.clone();
            let mut build = move |cx: &mut AppContext| {
                div()
                    .w_full()
                    .flex_col()
                    .children(cx.rows(&items, |row: &ListRow, _| row_with(row.index, row.selected)))
            };
            let mut root = div();
            measure(case, &mut harness, |harness, turn| {
                let toggled = rows[turn % rows.len()];
                if let Some(mut row) = context.get_mut(toggled) {
                    row.selected = !row.selected;
                }
                context.notify(toggled.id());
                harness.app_frame(&mut context, &mut root, &mut build, viewport)
            })?;
        }
        // `list_full` with each row a view of its own entity: the selection moves
        // as in `list_full`, and only the two rows it leaves and enters are
        // notified and rendered again.
        "list_full_rows" => {
            let mut harness = Harness::new()?;
            let mut context = AppContext::new();
            let rows: Vec<Entity<ListRow>> = (0..LIST_ROWS)
                .map(|index| {
                    context.create(ListRow {
                        index,
                        selected: index == 0,
                    })
                })
                .collect();
            let items = rows.clone();
            let mut build = move |cx: &mut AppContext| {
                div()
                    .w_full()
                    .flex_col()
                    .children(cx.rows(&items, |row: &ListRow, _| row_with(row.index, row.selected)))
            };
            let mut root = div();
            measure(case, &mut harness, |harness, turn| {
                let selected = turn % LIST_ROWS;
                let previous = (turn + LIST_ROWS - 1) % LIST_ROWS;
                for (index, on) in [(previous, false), (selected, true)] {
                    if let Some(mut row) = context.get_mut(rows[index]) {
                        row.selected = on;
                    }
                    context.notify(rows[index].id());
                }
                harness.app_frame(&mut context, &mut root, &mut build, viewport)
            })?;
        }
        "list_virtual_1k" | "list_virtual_100k" => {
            let count = if case == "list_virtual_1k" {
                1_000
            } else {
                100_000
            };
            let mut harness = Harness::new()?;
            let handle = VirtualListHandle::new();
            handle.scroll_to(count as f32 / 2.0 * ROW_HEIGHT);
            measure(case, &mut harness, |harness, turn| {
                let selected = count / 2 + turn % VISIBLE_ROWS;
                harness.frame(virtual_rows(&handle, count, selected), viewport)
            })?;
        }
        "text_paragraphs" => {
            let mut harness = Harness::new()?;
            measure(case, &mut harness, |harness, turn| {
                harness.frame(paragraphs(turn), viewport)
            })?;
        }
        "quad_grid" => {
            let mut harness = Harness::new()?;
            measure(case, &mut harness, |harness, turn| {
                harness.frame(quad_grid(turn % (GRID.0 * GRID.1)), viewport)
            })?;
        }
        // `quad_grid` as a static grid in a view that is never notified, so its
        // paint is replayed, under the changed cell in a view of its own.
        "quad_grid_cached" => {
            let mut harness = Harness::new()?;
            let mut context = AppContext::new();
            let grid = [context.create(())];
            let cell = [context.create(ChangedCell(0))];
            let mut build = move |cx: &mut AppContext| {
                div()
                    .w(GRID.0 as f32 * CELL.0)
                    .h(GRID.1 as f32 * CELL.1)
                    .children(cx.rows(&grid, |_: &(), _| static_grid()))
                    .children(cx.rows(&cell, |cell: &ChangedCell, _| changed_cell(cell.0)))
            };
            let mut root = div();
            measure(case, &mut harness, |harness, turn| {
                if let Some(mut changed) = context.get_mut(cell[0]) {
                    changed.0 = turn % (GRID.0 * GRID.1);
                }
                context.notify(cell[0].id());
                harness.app_frame(&mut context, &mut root, &mut build, viewport)
            })?;
        }
        "resize_full" => {
            let mut harness = Harness::new()?;
            measure(case, &mut harness, |harness, turn| {
                harness.frame(list(0), resized(turn))
            })?;
        }
        "resize_virtual" => {
            let mut harness = Harness::new()?;
            let handle = VirtualListHandle::new();
            handle.scroll_to(500.0 * ROW_HEIGHT);
            measure(case, &mut harness, |harness, turn| {
                harness.frame(virtual_rows(&handle, 1_000, 500), resized(turn))
            })?;
        }
        // The view scrolls down one line per frame; the cursor moves with it.
        "editor_scroll" => {
            let mut harness = Harness::new()?;
            let handle = VirtualListHandle::new();
            let document = document();
            measure(case, &mut harness, |harness, turn| {
                let top = scrolled_top(turn);
                handle.scroll_to(top as f32 * EDITOR_LINE_HEIGHT);
                harness.frame(code_view(&handle, &document, top + CURSOR_ROW), viewport)
            })?;
        }
        // One character of the cursor's line changes each frame.
        "editor_edit" => {
            let mut harness = Harness::new()?;
            let handle = VirtualListHandle::new();
            handle.scroll_to(EDITOR_TOP as f32 * EDITOR_LINE_HEIGHT);
            let document = document();
            let cursor = EDITOR_TOP + CURSOR_ROW;
            measure(case, &mut harness, |harness, turn| {
                let mut edited = document.borrow_mut();
                edit_line(&mut edited.tokens[cursor], turn);
                edited.lines[cursor] = CodeLine::new(&edited.tokens[cursor]);
                drop(edited);
                harness.frame(code_view(&handle, &document, cursor), viewport)
            })?;
        }
        // 2 000 fills, 500 strokes, 200 circles; one fill turns each frame.
        "paths" => {
            let mut harness = Harness::new()?;
            let shapes = compare_paths::scene();
            let kept = compare_paths::kept(&shapes);
            let root = |turn| compare_paths::backdrop(compare_paths::paths(&shapes, &kept, turn));
            compare_paths::save_png(case, &mut harness, || root(0), viewport)?;
            measure(case, &mut harness, |harness, turn| {
                harness.frame(root(turn), viewport)
            })?;
        }
        // 300 SVG icons of 20 paths; one moves each frame.
        "paths_icons" => {
            let mut harness = Harness::new()?;
            let markup = compare_paths::icon_markup();
            let root = |turn| compare_paths::backdrop(compare_paths::icons(&markup, turn));
            compare_paths::save_png(case, &mut harness, || root(0), viewport)?;
            measure(case, &mut harness, |harness, turn| {
                harness.frame(root(turn), viewport)
            })?;
        }
        _ => return Err(format!("unknown case {case}; cases: {}", CASES.join(" ")).into()),
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `cargo bench` passes `--bench`; `--list` lists the cases; anything else names one.
    if std::env::args().any(|arg| arg == "--list") {
        println!("{}", CASES.join("\n"));
        return Ok(());
    }
    let named: Vec<String> = std::env::args()
        .skip(1)
        .filter(|arg| !arg.starts_with("--"))
        .collect();
    if named.is_empty() {
        CASES.iter().try_for_each(|case| run(case))
    } else {
        named.iter().try_for_each(|case| run(case))
    }
}
