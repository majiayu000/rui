//! Performance gate: fails when a frame costs more than `benchmarks/perf_budgets.json`
//! allows. Run by CI.
//!
//! `cargo bench --bench perf_gate`
//!
//! Gated everywhere: heap allocations per frame (counted by this binary's
//! allocator), the virtual list's cost at 100 000 rows against 1 000, and resident
//! footprint. Gated where the machine exposes it: retired instructions of the CPU
//! stages (build, layout, paint). Hosted CI VMs may not, and the report says so.
//! Metal work is not gated by instructions: driver threads make it move too much.
//!
//! `PERF_GATE_PIXELS=1` also compares the long list's rendered pixels with the
//! committed checksum. Different GPUs antialias differently, so that is for local
//! runs before and after a renderer change, not for CI.
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::{Element, LayoutNodes};
use rui::elements::{VirtualListHandle, virtual_list};
use rui::renderer::Scene;
use rui::renderer::metal::MetalRenderer;
use rui::renderer::text::TextMeasureCache;
use rui::{Color, Div, ElementId, Size, div, text};
use serde::Deserialize;
use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use taffy::prelude::TaffyTree;

const BUDGETS_JSON: &str = include_str!("../benchmarks/perf_budgets.json");
const VIEWPORT: (f32, f32) = (1440.0, 900.0);
const ROW_HEIGHT: f32 = 28.0;
const EDITED_ROW: usize = 1_000;
const WARMUP: usize = 3;
const SAMPLES: usize = 15;

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
}

/// `rusage_info_v4` read as words: index 9 is `ri_phys_footprint`, 31 `ri_instructions`.
fn rusage() -> [u64; 64] {
    let mut words = [0u64; 64];
    // SAFETY: flavor 4 writes a rusage_info_v4, far shorter than 64 words.
    let status = unsafe { proc_pid_rusage(std::process::id() as i32, 4, words.as_mut_ptr()) };
    assert_eq!(status, 0, "proc_pid_rusage failed");
    words
}

#[derive(Debug, Deserialize)]
struct Budgets {
    /// Where and when the budgets were measured; read by people, not checked.
    #[allow(dead_code)]
    calibration: String,
    /// Most instructions per frame, in millions, by case.
    instructions_m: BTreeMap<String, f64>,
    /// Most heap allocations per frame, by case.
    allocations: BTreeMap<String, f64>,
    /// Most the 100 000-row virtual list may cost relative to 1 000 rows.
    virtual_list_scaling_max: f64,
    footprint_mb_max: f64,
    pixel_checksum: String,
}

#[derive(Clone, Copy)]
struct Cost {
    instructions_m: f64,
    allocations: f64,
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

/// Median cost of `frame(round)` over the samples, after warm-up.
fn measure(mut frame: impl FnMut(usize)) -> Cost {
    for round in 0..WARMUP {
        frame(round);
    }
    let (mut instructions, mut allocations) = (Vec::new(), Vec::new());
    for round in WARMUP..WARMUP + SAMPLES {
        let (i0, a0) = (rusage()[31], ALLOCATIONS.load(Ordering::Relaxed));
        frame(round);
        instructions.push(rusage()[31].saturating_sub(i0) as f64 / 1e6);
        allocations.push(ALLOCATIONS.load(Ordering::Relaxed).saturating_sub(a0) as f64);
    }
    Cost {
        instructions_m: median(instructions),
        allocations: median(allocations),
    }
}

fn row(index: usize, selected: usize) -> Div {
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

fn list(rows: usize, selected: usize) -> Div {
    div()
        .w_full()
        .flex_col()
        .children((0..rows).map(move |index| row(index, selected)))
}

/// The list with one row's label changed: the edit `round` made to row `EDITED_ROW`.
fn list_with_edit(rows: usize, round: usize) -> Div {
    div()
        .w_full()
        .flex_col()
        .children((0..rows).map(move |index| {
            if index == EDITED_ROW {
                row(index, usize::MAX).child(text(format!("edit {round}")))
            } else {
                row(index, usize::MAX).child(text("edit"))
            }
        }))
}

/// Paragraph `index` of the wrapped text case: 8 to 127 words.
fn paragraph(index: usize) -> String {
    const WORDS: [&str; 12] = [
        "layout",
        "glyph",
        "shaping",
        "frame",
        "wrapped",
        "paragraph",
        "text",
        "cache",
        "the",
        "of",
        "a",
        "width",
    ];
    (0..8 + (index * 37) % 120)
        .map(|word| WORDS[(index * 7 + word * 3) % WORDS.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

/// 200 wrapped paragraphs; paragraph 3 gets a new suffix each round.
fn paragraphs(round: usize) -> Div {
    div()
        .w_full()
        .flex_col()
        .p(16.0)
        .gap(8.0)
        .children((0..200).map(move |index| {
            let mut content = paragraph(index);
            if index == 3 {
                content.push_str(&format!(" revision {round}"));
            }
            text(content).size(14.0).line_height(1.4).wrap()
        }))
}

/// Layout and paint state a runner keeps across frames.
struct Surfaces {
    taffy: TaffyTree<ElementId>,
    nodes: LayoutNodes,
    scene: Scene,
}

impl Surfaces {
    fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            nodes: LayoutNodes::new(),
            scene: Scene::new(),
        }
    }

    fn frame(&mut self, root: &mut impl Element, measurer: &mut TextMeasureCache) {
        let viewport = Size::new(VIEWPORT.0, VIEWPORT.1);
        let bounds = FramePipeline::layout_root_retained(
            root,
            &mut self.taffy,
            &mut self.nodes,
            measurer,
            viewport,
        )
        .expect("layout");
        FramePipeline::paint_root(root, &self.taffy, &mut self.scene, bounds);
    }
}

fn pixel_checksum(scene: &Scene) -> Result<String, String> {
    let mut renderer = MetalRenderer::new().map_err(|err| err.to_string())?;
    let pixels = renderer
        .capture_frame_pixels(scene, Size::new(VIEWPORT.0, VIEWPORT.1))
        .map_err(|err| err.to_string())?;
    let hash = pixels.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    Ok(format!("{hash:016x}"))
}

fn main() -> ExitCode {
    let budgets: Budgets = match serde_json::from_str(BUDGETS_JSON) {
        Ok(budgets) => budgets,
        Err(err) => {
            eprintln!("perf_gate: benchmarks/perf_budgets.json is invalid: {err}");
            return ExitCode::FAILURE;
        }
    };
    let mut measurer = TextMeasureCache::new();
    let mut costs: BTreeMap<&str, Cost> = BTreeMap::new();

    // A 2000-row column; each frame selects another row, as a click would.
    let mut surfaces = Surfaces::new();
    costs.insert(
        "list.frame_cpu",
        measure(|round| surfaces.frame(&mut list(2_000, round), &mut measurer)),
    );
    let mut rounds: Vec<Div> = (0..WARMUP + SAMPLES)
        .map(|round| list(2_000, round))
        .collect();
    let mut layout_surfaces = Surfaces::new();
    costs.insert(
        "list.layout_retained",
        measure(|round| {
            let viewport = Size::new(VIEWPORT.0, VIEWPORT.1);
            FramePipeline::layout_root_retained(
                &mut rounds[round],
                &mut layout_surfaces.taffy,
                &mut layout_surfaces.nodes,
                &mut measurer,
                viewport,
            )
            .expect("layout");
        }),
    );
    drop(rounds);

    // Wrapped paragraphs, one of which changes each frame.
    let mut surfaces = Surfaces::new();
    costs.insert(
        "text.paragraphs",
        measure(|round| surfaces.frame(&mut paragraphs(round), &mut measurer)),
    );

    for (name, count) in [
        ("virtual_list.1k", 1_000usize),
        ("virtual_list.100k", 100_000),
    ] {
        let handle = VirtualListHandle::new();
        handle.scroll_to(count as f32 * ROW_HEIGHT / 2.0);
        let mut surfaces = Surfaces::new();
        costs.insert(
            name,
            measure(|round| {
                let selected = count / 2 + round;
                let mut root = virtual_list(&handle, count, ROW_HEIGHT, move |index| {
                    row(index, selected)
                })
                .w_full()
                .h_full();
                surfaces.frame(&mut root, &mut measurer);
            }),
        );
    }

    let mut failures = Vec::new();
    // Footprint with the long list drawn once, including the renderer when present.
    let footprint_note = match MetalRenderer::new() {
        Ok(mut renderer) => {
            let viewport = Size::new(VIEWPORT.0, VIEWPORT.1);
            match renderer.render_offscreen(&surfaces.scene, viewport, 2.0) {
                Ok(()) => "with the Metal renderer",
                Err(err) => {
                    failures.push(format!("rendering the long list failed: {err}"));
                    "render failed"
                }
            }
        }
        Err(_) => "no Metal device: CPU stages only",
    };
    let footprint = rusage()[9] as f64 / 1_048_576.0;
    println!(
        "footprint: {footprint:.1} MB (max {:.1}, {footprint_note})",
        budgets.footprint_mb_max
    );
    if footprint > budgets.footprint_mb_max {
        failures.push(format!(
            "footprint {footprint:.1} MB, max {:.1} MB",
            budgets.footprint_mb_max
        ));
    }

    // One row's text changes per frame. #173's target is under 10% of a full
    // frame: the same list laid out from an empty layout tree, with warm caches.
    // Measured after the footprint so it stays that of the long list drawn once.
    let mut full_surfaces = Surfaces::new();
    costs.insert(
        "list.full_frame",
        measure(|round| {
            full_surfaces.taffy.clear();
            full_surfaces.nodes = LayoutNodes::new();
            full_surfaces.frame(&mut list_with_edit(2_000, round), &mut measurer);
        }),
    );
    drop(full_surfaces);
    let mut edit_surfaces = Surfaces::new();
    costs.insert(
        "list.one_row_changed",
        measure(|round| edit_surfaces.frame(&mut list_with_edit(2_000, round), &mut measurer)),
    );
    drop(edit_surfaces);

    let counters = costs.values().any(|cost| cost.instructions_m > 0.0);
    println!(
        "{:<24} {:>14} {:>10} {:>14} {:>10}",
        "case", "instructions", "budget", "allocations", "budget"
    );
    for (name, cost) in &costs {
        let instruction_budget = budgets.instructions_m.get(*name).copied();
        let allocation_budget = budgets.allocations.get(*name).copied();
        let (Some(instruction_budget), Some(allocation_budget)) =
            (instruction_budget, allocation_budget)
        else {
            failures.push(format!("{name}: no budget in benchmarks/perf_budgets.json"));
            continue;
        };
        println!(
            "{name:<24} {:>13.1}M {:>9.1}M {:>14.0} {:>10.0}",
            cost.instructions_m, instruction_budget, cost.allocations, allocation_budget
        );
        if counters && cost.instructions_m > instruction_budget {
            failures.push(format!(
                "{name}: {:.1} M instructions per frame, budget {instruction_budget:.1} M",
                cost.instructions_m
            ));
        }
        if cost.allocations > allocation_budget {
            failures.push(format!(
                "{name}: {:.0} allocations per frame, budget {allocation_budget:.0}",
                cost.allocations
            ));
        }
    }
    if !counters {
        println!(
            "instruction counter unavailable on this machine: instruction budgets not checked"
        );
    }

    let (full, changed) = (costs["list.full_frame"], costs["list.one_row_changed"]);
    println!(
        "list one row changed / full frame: {:.2} by instructions, {:.2} by allocations (#173 target 0.10)",
        changed.instructions_m / full.instructions_m,
        changed.allocations / full.allocations
    );

    let (small, large) = (costs["virtual_list.1k"], costs["virtual_list.100k"]);
    let scaling = if counters {
        large.instructions_m / small.instructions_m
    } else {
        large.allocations / small.allocations
    };
    println!(
        "virtual_list 100k / 1k: {scaling:.2} (max {:.2}, by {})",
        budgets.virtual_list_scaling_max,
        if counters {
            "instructions"
        } else {
            "allocations"
        }
    );
    if scaling > budgets.virtual_list_scaling_max {
        failures.push(format!(
            "virtual_list: 100 000 rows cost {scaling:.2}x 1 000 rows, max {:.2}x",
            budgets.virtual_list_scaling_max
        ));
    }

    if std::env::var("PERF_GATE_PIXELS").as_deref() == Ok("1") {
        let mut surfaces = Surfaces::new();
        surfaces.frame(&mut list(2_000, 0), &mut measurer);
        match pixel_checksum(&surfaces.scene) {
            Ok(checksum) => {
                println!(
                    "pixel checksum: {checksum} (expected {})",
                    budgets.pixel_checksum
                );
                if checksum != budgets.pixel_checksum {
                    failures.push(format!(
                        "pixel checksum {checksum}, expected {}: the long list renders differently",
                        budgets.pixel_checksum
                    ));
                }
            }
            Err(err) => failures.push(format!(
                "PERF_GATE_PIXELS=1 but pixels could not be read: {err}"
            )),
        }
    }

    if failures.is_empty() {
        println!("perf_gate: within budget");
        ExitCode::SUCCESS
    } else {
        for failure in &failures {
            eprintln!("perf_gate: over budget: {failure}");
        }
        ExitCode::FAILURE
    }
}
