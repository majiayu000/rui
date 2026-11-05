//! RUI side of the GPUI comparison. Prints one line per case.
//!
//! `cargo bench --bench compare_frame [-- <case>...]` runs the named cases, or all of
//! them. `benchmarks/gpui_compare/run.sh` runs each case in its own process, as it does
//! for GPUI, and prints both sides in one table.
//!
//! Every fixture matches `benchmarks/gpui_compare/fixtures.rs`: same content and sizes,
//! a 1440x900 window at scale 2. Each frame changes one thing, then rebuilds, lays out
//! against the kept layout tree, paints and submits to Metal without waiting, as GPUI's
//! headless renderer does. Instructions are the whole process's retired instructions,
//! as GPUI's `BENCH_MEASUREMENT=instructions` counts; allocations are counted by this
//! binary's allocator, as `gpui::bench_main!` installs one.
//!
//! Draw calls and instances come from `MetalRenderer::last_frame_stats`. GPU time is
//! read in a separate pass after the CPU samples, whose frames wait for the command
//! buffer to finish and report `GPUEndTime - GPUStartTime`, so waiting never shows in
//! the CPU numbers.
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::{Element, LayoutNodes};
use rui::elements::{VirtualListHandle, canvas, virtual_list};
use rui::renderer::metal::MetalRenderer;
use rui::renderer::text::TextMeasureCache;
use rui::renderer::{RendererResourceLimits, Scene};
use rui::{Bounds, Color, ElementId, Size, div, text};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use taffy::prelude::TaffyTree;

const CASES: [&str; 9] = [
    "startup",
    "list_full",
    "list_virtual_1k",
    "list_virtual_100k",
    "text_paragraphs",
    "quad_grid",
    "resize_full",
    "resize_virtual",
    "list_full_one_row",
];
const WARMUP: usize = 10;
const SAMPLES: usize = 60;
/// Frames of the separate pass that waits for the GPU to read its time.
const GPU_SAMPLES: usize = 30;
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
const GRID: (usize, usize) = (200, 60);
const CELL: (f32, f32) = (7.0, 15.0);

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
    let background = if index == selected {
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

fn cell_color(row: usize, column: usize) -> u32 {
    (((column * 255 / GRID.0) as u32) << 16) | (((row * 255 / GRID.1) as u32) << 8) | 0x60
}

/// A 200x60 grid of 7x15 cells painted by one canvas; cell `changed` is white.
fn quad_grid(changed: usize) -> impl Element {
    canvas(move |bounds, painter| {
        for row in 0..GRID.1 {
            for column in 0..GRID.0 {
                let cell = Bounds::from_xywh(
                    bounds.x() + column as f32 * CELL.0,
                    bounds.y() + row as f32 * CELL.1,
                    CELL.0,
                    CELL.1,
                );
                let color = if row * GRID.0 + column == changed {
                    0xffffff
                } else {
                    cell_color(row, column)
                };
                painter.fill(cell, Color::hex(color));
            }
        }
    })
    .w(GRID.0 as f32 * CELL.0)
    .h(GRID.1 as f32 * CELL.1)
}

/// What one runtime keeps across frames: layout tree, measure cache, scene, renderer.
struct Harness {
    taffy: TaffyTree<ElementId>,
    nodes: LayoutNodes,
    scene: Scene,
    measurer: TextMeasureCache,
    renderer: MetalRenderer,
    /// Wait for each frame's GPU work and keep its GPU time in `gpu_ms`.
    wait_for_gpu: bool,
    gpu_ms: f64,
}

impl Harness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            taffy: TaffyTree::new(),
            nodes: LayoutNodes::new(),
            scene: Scene::new(),
            measurer: TextMeasureCache::new(),
            renderer: MetalRenderer::new()?,
            wait_for_gpu: false,
            gpu_ms: 0.0,
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
        FramePipeline::paint_root(root, &self.taffy, &mut self.scene, bounds);
        if self.wait_for_gpu {
            let gpu =
                self.renderer
                    .render_offscreen_gpu_time(&self.scene, viewport, SCALE_FACTOR)?;
            self.gpu_ms = gpu.as_secs_f64() * 1e3;
        } else {
            self.renderer
                .render_offscreen(&self.scene, viewport, SCALE_FACTOR)?;
        }
        Ok(())
    }
}

fn percentile(values: &mut [f64], fraction: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len() - 1) as f64 * fraction).round() as usize]
}

/// Runs `frame(harness, turn)` after warm-up and prints the per-frame costs. CPU
/// costs come from frames that do not wait; GPU time from a later pass that does.
fn measure(
    case: &str,
    harness: &mut Harness,
    mut frame: impl FnMut(&mut Harness, usize) -> Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    for turn in 0..WARMUP {
        frame(harness, turn)?;
    }
    let (mut ms, mut instructions, mut allocations) = (Vec::new(), Vec::new(), Vec::new());
    let (mut draw_calls, mut instances) = (Vec::new(), Vec::new());
    for turn in WARMUP..WARMUP + SAMPLES {
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
    harness.wait_for_gpu = true;
    let mut gpu_ms = Vec::new();
    for turn in WARMUP + SAMPLES..WARMUP + SAMPLES + GPU_SAMPLES {
        frame(harness, turn)?;
        gpu_ms.push(harness.gpu_ms);
    }
    harness.wait_for_gpu = false;
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
        "list_full" | "list_full_one_row" => {
            // RUI has no per-view cache like GPUI's `.cached()`: one changed row still
            // rebuilds the list, so both cases are this one.
            let mut harness = Harness::new()?;
            measure(case, &mut harness, |harness, turn| {
                harness.frame(list(turn % LIST_ROWS), viewport)
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
