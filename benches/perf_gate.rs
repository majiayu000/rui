//! Performance gate: fails when a frame costs more than `benchmarks/perf_budgets/`
//! allows: `<case>.json` per case, `shared.json` for the rest. Run by CI.
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
mod perf_budgets;
mod perf_icons;

use perf_budgets::{CaseBudget, load_budgets};
use perf_icons::IconGrid;
use rui::core::entity::Entity;
use rui::core::event::MouseButton;
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::{Element, LayoutNodes, PointerEvent, PointerEventKind, reconcile};
use rui::elements::view;
use rui::elements::{VirtualListHandle, span, styled_text, virtual_list};
use rui::renderer::Scene;
use rui::renderer::metal::{MetalRenderer, RetainedTarget};
use rui::renderer::text::TextMeasureCache;
use rui::{
    AppContext, Color, Div, ElementId, Point, Presenter, Size, View, ViewContext, div, text,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use taffy::prelude::TaffyTree;

const VIEWPORT: (f32, f32) = (1440.0, 900.0);
const ROW_HEIGHT: f32 = 28.0;
const EDITED_ROW: usize = 1_000;
/// A row in view at the top of the list, for the damage case.
const VISIBLE_EDITED_ROW: usize = 10;
const CODE_LINE_HEIGHT: f32 = 20.0;
const WARMUP: usize = 3;
/// Rows of `views.churn`, and how many of them are replaced per frame.
const CHURN_ROWS: usize = 200;
const CHURN_PER_FRAME: usize = 20;
/// Frames of `views.churn` the heap must stay flat over.
const CHURN_FRAMES: usize = 300;
const SAMPLES: usize = 15;

struct CountingAllocator;

static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);
/// Bytes allocated and not yet freed.
static LIVE_BYTES: AtomicI64 = AtomicI64::new(0);

// SAFETY: forwards every call to `System` unchanged; only counts allocations.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        LIVE_BYTES.fetch_add(layout.size() as i64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size() as i64, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        LIVE_BYTES.fetch_add(new_size as i64 - layout.size() as i64, Ordering::Relaxed);
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

/// A row of the long list as a view, showing its own label entity.
/// A row of the list written with `AppContext::rows`.
struct RowItem {
    index: usize,
    label: String,
}

struct ListRow {
    index: usize,
    label: Entity<String>,
}

impl View for ListRow {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Div {
        let label = cx.app().get(self.label).map(|label| label.clone());
        row(self.index, usize::MAX).child(text(label.expect("row label")))
    }
}

/// Line `index` of a code view: its number, then colored tokens as styled runs.
fn code_line(index: usize) -> Div {
    div()
        .w_full()
        .h(CODE_LINE_HEIGHT)
        .flex_row()
        .child(div().w(64.0).child(text((index + 1).to_string())))
        .child(styled_text([
            span("let ").color(Color::hex(0xc678dd)),
            span(format!("value_{} ", index % 97)).color(Color::hex(0xe5c07b)),
            span("= ").color(Color::hex(0xabb2bf)),
            span((index * 31 % 1000).to_string()).color(Color::hex(0xd19a66)),
            span(";").color(Color::hex(0xabb2bf)),
        ]))
}

/// Row views with their label entities, oldest first.
type ChurnRows = Rc<RefCell<VecDeque<(Entity<ListRow>, Entity<String>)>>>;

/// A list of row views whose oldest rows are replaced by new ones each frame.
struct Churn {
    rows: ChurnRows,
    made: usize,
    root: Div,
    surfaces: Surfaces,
}

impl Churn {
    fn new(context: &mut AppContext) -> Self {
        let mut churn = Self {
            rows: Rc::default(),
            made: 0,
            root: div(),
            surfaces: Surfaces::new(),
        };
        for _ in 0..CHURN_ROWS {
            churn.push_row(context);
        }
        churn
    }

    fn push_row(&mut self, context: &mut AppContext) {
        // Labels repeat, so the text caches stay warm and the same size.
        let label = context.create(format!("row {}", self.made % CHURN_ROWS));
        let row = context.new_view(ListRow {
            index: self.made,
            label,
        });
        self.rows.borrow_mut().push_back((row, label));
        self.made += 1;
    }

    fn frame(&mut self, context: &mut AppContext, measurer: &mut TextMeasureCache) {
        for _ in 0..CHURN_PER_FRAME {
            let (_, label) = self.rows.borrow_mut().pop_front().expect("a row");
            // The view goes once no tree places it; its label is ours to release.
            context.release(label);
            self.push_row(context);
        }
        context.request_rebuild();
        let rows = Rc::clone(&self.rows);
        let mut build = move |_: &mut AppContext| {
            div()
                .w_full()
                .flex_col()
                .children(rows.borrow().iter().map(|(row, _)| view(*row)))
        };
        FramePipeline::build_frame_retained(
            context,
            &mut self.root,
            &mut build,
            &mut self.surfaces.taffy,
            &mut self.surfaces.nodes,
            &mut self.surfaces.scene,
            measurer,
            Size::new(VIEWPORT.0, VIEWPORT.1),
        )
        .expect("churn frame");
    }
}

fn list(rows: usize, selected: usize) -> Div {
    div()
        .w_full()
        .flex_col()
        .children((0..rows).map(move |index| row(index, selected)))
}

/// The list with one row's label changed: the edit `round` made to row `edited`.
fn list_with_edit(rows: usize, edited: usize, round: usize) -> Div {
    div()
        .w_full()
        .flex_col()
        .children((0..rows).map(move |index| {
            if index == edited {
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
        FramePipeline::paint_root_in_viewport(root, &self.taffy, &mut self.scene, bounds, viewport);
    }
}

/// A runner's state across frames that rebuild the tree, as the frame pipeline
/// does: the parts of the new tree that match the kept one are not taken.
struct Rebuilt {
    root: Div,
    surfaces: Surfaces,
}

impl Rebuilt {
    fn new() -> Self {
        Self {
            root: div(),
            surfaces: Surfaces::new(),
        }
    }

    fn frame(&mut self, mut next: Div, measurer: &mut TextMeasureCache) {
        if !reconcile(&mut next, &mut self.root) {
            self.root = next;
        }
        self.surfaces.frame(&mut self.root, measurer);
    }
}

/// What a retained window draws again per frame: median over the samples.
struct DamageCost {
    /// Share of the window's pixels drawn again.
    fraction: f64,
    damaged_ms: f64,
    full_ms: f64,
}

/// Frames of `build(round)` through a [`RetainedTarget`], each also drawn in full.
fn damage_cost(
    renderer: &mut MetalRenderer,
    rebuilt: &mut Rebuilt,
    measurer: &mut TextMeasureCache,
    build: impl Fn(usize) -> Div,
) -> Result<DamageCost, String> {
    let viewport = Size::new(VIEWPORT.0, VIEWPORT.1);
    let total = f64::from(VIEWPORT.0) * f64::from(VIEWPORT.1) * 4.0;
    let mut target = RetainedTarget::new();
    let (mut fractions, mut damaged_ms, mut full_ms) = (Vec::new(), Vec::new(), Vec::new());
    for round in 0..WARMUP + SAMPLES {
        rebuilt.frame(build(round), measurer);
        let scene = &rebuilt.surfaces.scene;
        let damaged = renderer
            .render_retained_offscreen_gpu_time(scene, &mut target, viewport, 2.0)
            .map_err(|err| err.to_string())?;
        let redrawn = renderer.last_frame_stats().redrawn_pixels;
        let full = renderer
            .render_offscreen_gpu_time(scene, viewport, 2.0)
            .map_err(|err| err.to_string())?;
        if round >= WARMUP {
            fractions.push(redrawn as f64 / total);
            damaged_ms.push(damaged.as_secs_f64() * 1e3);
            full_ms.push(full.as_secs_f64() * 1e3);
        }
    }
    Ok(DamageCost {
        fraction: median(fractions),
        damaged_ms: median(damaged_ms),
        full_ms: median(full_ms),
    })
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
    let budgets = match load_budgets() {
        Ok(budgets) => budgets,
        Err(err) => {
            eprintln!("perf_gate: invalid budget: {err}");
            return ExitCode::FAILURE;
        }
    };
    let mut measurer = TextMeasureCache::new();
    let mut costs: BTreeMap<&str, Cost> = BTreeMap::new();

    // A 2000-row column; each frame selects another row, as a click would.
    let mut rebuilt = Rebuilt::new();
    costs.insert(
        "list.frame_cpu",
        measure(|round| rebuilt.frame(list(2_000, round), &mut measurer)),
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
    let mut rebuilt_text = Rebuilt::new();
    costs.insert(
        "text.paragraphs",
        measure(|round| rebuilt_text.frame(paragraphs(round), &mut measurer)),
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

    // A 10 000-line code view of styled runs, scrolled one line per frame.
    let handle = VirtualListHandle::new();
    let mut surfaces = Surfaces::new();
    costs.insert(
        "virtual_list.code_scroll",
        measure(|round| {
            handle.scroll_to((5_000 + round) as f32 * CODE_LINE_HEIGHT);
            let mut root = virtual_list(&handle, 10_000, CODE_LINE_HEIGHT, code_line)
                .w_full()
                .h_full();
            surfaces.frame(&mut root, &mut measurer);
        }),
    );

    let mut failures = Vec::new();
    // RUI's own memory, before a renderer and its driver allocations exist.
    let cpu_footprint = rusage()[9] as f64 / 1_048_576.0;
    println!(
        "footprint before the renderer: {cpu_footprint:.1} MB (max {:.1})",
        budgets.footprint_cpu_mb_max
    );
    if cpu_footprint > budgets.footprint_cpu_mb_max {
        failures.push(format!(
            "footprint before the renderer {cpu_footprint:.1} MB, max {:.1} MB",
            budgets.footprint_cpu_mb_max
        ));
    }
    // Footprint with the long list drawn once, including the renderer when present.
    let footprint_note = match MetalRenderer::new() {
        Ok(mut renderer) => {
            let viewport = Size::new(VIEWPORT.0, VIEWPORT.1);
            match renderer.render_offscreen(&rebuilt.surfaces.scene, viewport, 2.0) {
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

    // A pointer move over the long list, whose rows have no handlers.
    let viewport = Size::new(VIEWPORT.0, VIEWPORT.1);
    let mut presenter = Presenter::with_root(viewport, list(2_000, 0));
    for _ in 0..2 {
        presenter.layout(viewport).expect("layout");
        presenter.paint();
    }
    costs.insert(
        "list.pointer_move",
        measure(|round| {
            presenter.dispatch_pointer_event(&PointerEvent {
                kind: PointerEventKind::Move,
                position: Point::new(200.0, 10.0 + round as f32 * 13.0),
                button: None::<MouseButton>,
            });
        }),
    );
    drop(presenter);

    // One row's text changes per frame. #173's target is under 10% of a full
    // frame: the same list laid out from an empty layout tree, with warm caches.
    // Measured after the footprint so it stays that of the long list drawn once.
    let mut full_surfaces = Surfaces::new();
    costs.insert(
        "list.full_frame",
        measure(|round| {
            full_surfaces.taffy.clear();
            full_surfaces.nodes = LayoutNodes::new();
            full_surfaces.frame(&mut list_with_edit(2_000, EDITED_ROW, round), &mut measurer);
        }),
    );
    drop(full_surfaces);
    let mut edited = Rebuilt::new();
    costs.insert(
        "list.one_row_changed",
        measure(|round| edited.frame(list_with_edit(2_000, EDITED_ROW, round), &mut measurer)),
    );

    // The same list with its rows as views: only the edited row renders again,
    // and the list around it is not rebuilt.
    let mut context = AppContext::new();
    let labels: Vec<Entity<String>> = (0..2_000)
        .map(|_| context.create(String::from("edit")))
        .collect();
    let rows: Vec<Entity<ListRow>> = labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            context.new_view(ListRow {
                index,
                label: *label,
            })
        })
        .collect();
    let mut build_list = move |_: &mut AppContext| {
        div()
            .w_full()
            .flex_col()
            .children(rows.iter().map(|row| view(*row)))
    };
    let mut viewed = Surfaces::new();
    let mut viewed_root = div();
    let mut view_frame =
        |context: &mut AppContext, root: &mut Div, measurer: &mut TextMeasureCache| {
            FramePipeline::build_frame_retained(
                context,
                root,
                &mut build_list,
                &mut viewed.taffy,
                &mut viewed.nodes,
                &mut viewed.scene,
                measurer,
                Size::new(VIEWPORT.0, VIEWPORT.1),
            )
            .expect("view frame");
        };
    view_frame(&mut context, &mut viewed_root, &mut measurer);
    let edited_label = labels[EDITED_ROW];
    costs.insert(
        "list.one_row_changed_views",
        measure(|round| {
            *context.get_mut(edited_label).expect("edited label") = format!("edit {round}");
            context.notify(edited_label.id());
            view_frame(&mut context, &mut viewed_root, &mut measurer);
        }),
    );
    drop(viewed_root);

    // The same list written with `cx.rows`: a view per entity without a view type.
    let items: Vec<Entity<RowItem>> = (0..2_000)
        .map(|index| {
            context.create(RowItem {
                index,
                label: String::from("edit"),
            })
        })
        .collect();
    let edited_item = items[EDITED_ROW];
    let mut build_rows = move |cx: &mut AppContext| {
        div()
            .w_full()
            .flex_col()
            .children(cx.rows(&items, |item: &RowItem, _| {
                row(item.index, usize::MAX).child(text(item.label.clone()))
            }))
    };
    let mut rows_surfaces = Surfaces::new();
    let mut rows_root = div();
    let mut rows_frame =
        |context: &mut AppContext, root: &mut Div, measurer: &mut TextMeasureCache| {
            FramePipeline::build_frame_retained(
                context,
                root,
                &mut build_rows,
                &mut rows_surfaces.taffy,
                &mut rows_surfaces.nodes,
                &mut rows_surfaces.scene,
                measurer,
                Size::new(VIEWPORT.0, VIEWPORT.1),
            )
            .expect("rows frame");
        };
    rows_frame(&mut context, &mut rows_root, &mut measurer);
    costs.insert(
        "list.one_row_changed_rows",
        measure(|round| {
            context.get_mut(edited_item).expect("edited item").label = format!("edit {round}");
            context.notify(edited_item.id());
            rows_frame(&mut context, &mut rows_root, &mut measurer);
        }),
    );
    drop(rows_root);

    // Row views made and dropped every frame: the oldest rows leave the list
    // and new ones join it. The rows that stay move up and render again none.
    let mut churn = Churn::new(&mut context);
    costs.insert(
        "views.churn",
        measure(|_| churn.frame(&mut context, &mut measurer)),
    );
    // Caches and arenas reach their size over the first frames.
    for _ in 0..CHURN_FRAMES {
        churn.frame(&mut context, &mut measurer);
    }
    let before = LIVE_BYTES.load(Ordering::Relaxed);
    for _ in 0..CHURN_FRAMES {
        churn.frame(&mut context, &mut measurer);
    }
    let growth_kb = (LIVE_BYTES.load(Ordering::Relaxed) - before) as f64 / 1024.0;
    drop(churn);

    let mut icons = IconGrid::new(Size::new(VIEWPORT.0, VIEWPORT.1), &mut measurer);
    costs.insert(
        "icons.grid_rebuilt",
        measure(|round| icons.frame(round, &mut measurer)),
    );
    drop(icons);

    // #175: how much of the window one change in view draws again, and the GPU
    // time of that frame against a full redraw.
    let damage = match MetalRenderer::new() {
        Ok(mut renderer) => {
            let list = damage_cost(&mut renderer, &mut edited, &mut measurer, |round| {
                list_with_edit(2_000, VISIBLE_EDITED_ROW, round)
            });
            let text = damage_cost(
                &mut renderer,
                &mut Rebuilt::new(),
                &mut measurer,
                paragraphs,
            );
            Some((list, text))
        }
        Err(_) => None,
    };
    drop(edited);

    println!(
        "views.churn heap growth over {CHURN_FRAMES} frames: {growth_kb:.1} KB (max {:.1})",
        budgets.churn_growth_kb_max
    );
    if growth_kb > budgets.churn_growth_kb_max {
        failures.push(format!(
            "views.churn: the heap grew {growth_kb:.1} KB over {CHURN_FRAMES} frames, max {:.1} KB",
            budgets.churn_growth_kb_max
        ));
    }

    let counters = costs.values().any(|cost| cost.instructions_m > 0.0);
    println!(
        "{:<24} {:>14} {:>10} {:>14} {:>10}",
        "case", "instructions", "budget", "allocations", "budget"
    );
    for (name, cost) in &costs {
        let Some(&CaseBudget {
            instructions_m: instruction_budget,
            allocations: allocation_budget,
        }) = budgets.cases.get(*name)
        else {
            failures.push(format!(
                "{name}: no budget, add benchmarks/perf_budgets/{name}.json"
            ));
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

    match damage {
        Some((list, text)) => {
            for (name, cost) in [("list, one row", &list), ("text, one paragraph", &text)] {
                match cost {
                    Ok(cost) => println!(
                        "damage, {name} changed: {:.3} of the window, GPU {:.2} ms against {:.2} ms for a full redraw",
                        cost.fraction, cost.damaged_ms, cost.full_ms
                    ),
                    Err(err) => failures.push(format!("damage, {name} changed: {err}")),
                }
            }
            if let Ok(list) = list {
                println!(
                    "one changed row damage max {:.3}",
                    budgets.one_row_damage_max
                );
                if list.fraction > budgets.one_row_damage_max {
                    failures.push(format!(
                        "one changed row draws {:.3} of the window again, max {:.3}",
                        list.fraction, budgets.one_row_damage_max
                    ));
                }
            }
        }
        None => println!("no Metal device: damage not measured"),
    }

    let full = costs["list.full_frame"];
    for (name, case) in [
        ("rebuilt list", "list.one_row_changed"),
        ("rows as views", "list.one_row_changed_views"),
        ("cx.rows", "list.one_row_changed_rows"),
    ] {
        let changed = costs[case];
        let by_instructions = if counters {
            format!("{:.3}", changed.instructions_m / full.instructions_m)
        } else {
            "unavailable".to_string()
        };
        println!(
            "one row changed, {name} / full frame: {by_instructions} by instructions, {:.3} by allocations (#173 target 0.10)",
            changed.allocations / full.allocations
        );
    }

    let rows = costs["list.one_row_changed_rows"];
    let rows_fraction = if counters {
        rows.instructions_m / full.instructions_m
    } else {
        rows.allocations / full.allocations
    };
    if rows_fraction > budgets.one_row_changed_rows_max {
        failures.push(format!(
            "list.one_row_changed_rows costs {rows_fraction:.3} of a full frame, max {:.2}",
            budgets.one_row_changed_rows_max
        ));
    }

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
