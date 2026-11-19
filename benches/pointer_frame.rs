//! Cost of pointer frames through the presenter's input path. Prints JSON.
//!
//! `cargo bench --bench pointer_frame` (`STRESS_ROWS=500` for another length,
//! `POINTER_FIXTURE=buttons` for the fixtures whose name contains it)
//!
//! Every runner dispatches input after layout (`FrameStage::ORDER`), and layout
//! leaves the presenter's scene stale, so each pointer event first paints the
//! whole tree into a scene of its own to find what it hits: the hit pass. Each
//! fixture runs its rounds three ways:
//!
//! - `frame`: through `rui::testing::mount`, one frame per event, Prepare to
//!   Finish, as a runner draws it.
//! - `stages`: the same frames on a `Presenter` driven stage by stage.
//! - `dispatch_after_paint`: the same events dispatched after paint, when the
//!   painted scene is current and no hit pass runs. No runner does this.
//!
//! `hit_pass` is `stages.dispatch` minus `dispatch_after_paint`, and `share` is
//! its part of `frame`. Costs are per event, the median over the rounds after
//! warm-up: retired instructions of the whole process (macOS `ri_instructions`;
//! 0 where the machine does not count them) and heap allocations counted by
//! this binary's allocator. Milliseconds move with load, so the load average is
//! printed beside them.
mod pointer_fixtures;

use pointer_fixtures::{ALL, Fixture, Probe, Seen, Setup, VIEWPORT};
use rui::core::event::Event;
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::{Element, PointerEvent};
use rui::renderer::Scene;
use rui::testing::mount;
use rui::{AppContext, Div, Point, Presenter, Size};
use serde_json::{Value, json};
use std::alloc::{GlobalAlloc, Layout, System};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

const DEFAULT_ROWS: usize = 2_000;
const WARMUP: usize = 3;
const SAMPLES: usize = 11;

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
    fn getloadavg(loads: *mut f64, count: i32) -> i32;
}

/// `ri_instructions` of `rusage_info_v4`, read as words.
fn instructions() -> u64 {
    let mut words = [0u64; 64];
    // SAFETY: flavor 4 writes a rusage_info_v4, far shorter than 64 words.
    let status = unsafe { proc_pid_rusage(std::process::id() as i32, 4, words.as_mut_ptr()) };
    assert_eq!(status, 0, "proc_pid_rusage failed");
    words[31]
}

/// The one-minute load average, as `uptime` prints it.
fn load() -> f64 {
    let mut loads = [0.0f64; 3];
    // SAFETY: writes at most the three averages asked for.
    let read = unsafe { getloadavg(loads.as_mut_ptr(), 3) };
    if read > 0 { loads[0] } else { f64::NAN }
}

fn rows() -> usize {
    std::env::var("STRESS_ROWS")
        .ok()
        .and_then(|rows| rows.parse().ok())
        .unwrap_or(DEFAULT_ROWS)
}

#[derive(Clone, Copy, Default)]
struct Cost {
    instructions: f64,
    allocations: f64,
    ns: f64,
}

impl Cost {
    fn per(self, events: usize) -> Self {
        let events = events as f64;
        Self {
            instructions: self.instructions / events,
            allocations: self.allocations / events,
            ns: self.ns / events,
        }
    }

    fn plus(self, other: Self) -> Self {
        Self {
            instructions: self.instructions + other.instructions,
            allocations: self.allocations + other.allocations,
            ns: self.ns + other.ns,
        }
    }

    fn json(self) -> Value {
        json!({
            "instructions_m": rounded(self.instructions / 1e6, 3),
            "allocations": rounded(self.allocations, 1),
        })
    }

    fn json_with_ms(self, load: f64) -> Value {
        json!({
            "instructions_m": rounded(self.instructions / 1e6, 3),
            "allocations": rounded(self.allocations, 1),
            "ms": rounded(self.ns / 1e6, 2),
            "load_1m": rounded(load, 1),
        })
    }
}

fn rounded(value: f64, digits: i32) -> f64 {
    let scale = 10f64.powi(digits);
    (value * scale).round() / scale
}

/// Counters at the start of what is measured.
struct Meter {
    instructions: u64,
    allocations: u64,
    started: Instant,
}

impl Meter {
    fn start() -> Self {
        Self {
            instructions: instructions(),
            allocations: ALLOCATIONS.load(Ordering::Relaxed),
            started: Instant::now(),
        }
    }

    fn stop(self) -> Cost {
        let ns = self.started.elapsed().as_nanos() as f64;
        Cost {
            instructions: instructions().saturating_sub(self.instructions) as f64,
            allocations: ALLOCATIONS
                .load(Ordering::Relaxed)
                .saturating_sub(self.allocations) as f64,
            ns,
        }
    }
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

/// The median of each counter over `costs`.
fn median_cost(costs: &[Cost]) -> Cost {
    Cost {
        instructions: median(costs.iter().map(|cost| cost.instructions).collect()),
        allocations: median(costs.iter().map(|cost| cost.allocations).collect()),
        ns: median(costs.iter().map(|cost| cost.ns).collect()),
    }
}

/// What a round left, for the fixture to check.
fn seen<'a>(probe: &'a Probe, presenter: &'a Presenter<Div>) -> Seen<'a> {
    Seen {
        probe,
        scene: presenter.scene(),
        cursor: presenter.cursor(),
        root_bounds: presenter.root_bounds(),
    }
}

fn hit_regions(scene: &Scene) -> usize {
    scene
        .layers()
        .iter()
        .map(|layer| layer.hit_regions().len())
        .sum()
}

/// Whole frames through the headless runner, one per event.
struct Frames {
    /// Where the fixture put the pointer in the mounted tree.
    pointer: Point,
    per_event: Cost,
    idle: Cost,
    canvas_calls: f64,
    idle_canvas_calls: f64,
    primitives: usize,
    hit_regions: usize,
}

fn run_frames(fixture: Fixture, rows: usize) -> Frames {
    let probe = Rc::new(Probe::default());
    let tree = Rc::clone(&probe);
    let mut session = mount(VIEWPORT, move |_: &mut AppContext| {
        fixture.build(&tree, rows)
    })
    .expect("mount");
    if let Some(scroll) = fixture.scroll(rows) {
        assert!(
            session.dispatch_scroll_event(&scroll),
            "the scroll view scrolls"
        );
    }
    session.frame().expect("frame");
    let pointer = fixture.pointer(session.scene());
    let (mut costs, mut calls) = (Vec::new(), Vec::new());
    for round in 0..WARMUP + SAMPLES {
        let events = fixture.events(round, pointer);
        let calls_before = probe.canvas_calls.get();
        let meter = Meter::start();
        match fixture.setup(round) {
            Setup::None => {}
            Setup::Swap => {
                probe.swapped.set(!probe.swapped.get());
                session.app_context_mut().request_rebuild();
            }
            Setup::Resize(size) => {
                session.resize(size);
            }
        }
        for event in &events {
            session.dispatch_pointer_event(*event);
        }
        let cost = meter.stop().per(events.len());
        let round_calls = (probe.canvas_calls.get() - calls_before) as f64 / events.len() as f64;
        let seen = Seen {
            probe: &probe,
            scene: session.scene(),
            cursor: session.cursor(),
            root_bounds: session.root_bounds(),
        };
        fixture.check(round, pointer, rows, &seen);
        if round >= WARMUP {
            costs.push(cost);
            calls.push(round_calls);
        }
    }
    // Frames that dispatch nothing, after the last round left its state.
    let (mut idle, mut idle_calls) = (Vec::new(), Vec::new());
    for round in 0..WARMUP + SAMPLES {
        let calls_before = probe.canvas_calls.get();
        let meter = Meter::start();
        session.frame().expect("idle frame");
        let cost = meter.stop();
        if round >= WARMUP {
            idle.push(cost);
            idle_calls.push((probe.canvas_calls.get() - calls_before) as f64);
        }
    }
    Frames {
        pointer,
        per_event: median_cost(&costs),
        idle: median_cost(&idle),
        canvas_calls: median(calls),
        idle_canvas_calls: median(idle_calls),
        primitives: session.scene().len(),
        hit_regions: hit_regions(session.scene()),
    }
}

/// A presenter driven stage by stage, as `FramePipeline::run_frame` drives it.
struct Staged<F> {
    context: AppContext,
    build: F,
    presenter: Presenter<Div>,
    viewport: Size,
}

/// One frame's stages, each measured.
#[derive(Clone, Copy, Default)]
struct Stages {
    rebuild: Cost,
    layout: Cost,
    dispatch: Cost,
    paint: Cost,
}

impl<F: FnMut(&mut AppContext) -> Div> Staged<F> {
    fn new(fixture: Fixture, rows: usize, mut build: F) -> Self {
        let mut context = AppContext::new();
        let root = build(&mut context);
        let mut staged = Self {
            context,
            build,
            presenter: Presenter::with_root(VIEWPORT, root),
            viewport: VIEWPORT,
        };
        staged.draw();
        if let Some(scroll) = fixture.scroll(rows) {
            let (scrolled, _) = staged
                .presenter
                .with_event_context(|root, cx| root.handle_scroll_event(cx, &scroll));
            assert!(scrolled, "the scroll view scrolls");
        }
        staged.draw();
        staged
    }

    /// A frame with nothing dispatched.
    fn draw(&mut self) {
        FramePipeline::prepare_frame(&mut self.context);
        self.presenter
            .rebuild_if_needed(&mut self.context, &mut self.build);
        self.presenter.layout(self.viewport).expect("layout");
        self.presenter.paint();
        FramePipeline::finish_frame(&mut self.context);
    }

    fn setup(&mut self, setup: Setup, probe: &Probe) {
        match setup {
            Setup::None => {}
            Setup::Swap => {
                probe.swapped.set(!probe.swapped.get());
                self.context.request_rebuild();
            }
            Setup::Resize(size) => {
                self.viewport = size;
                self.context.request_rebuild();
                self.presenter.handle_window_event(&Event::WindowResize {
                    width: size.width,
                    height: size.height,
                });
            }
        }
    }

    /// A frame in the runners' order: layout, then `event`, then paint.
    fn frame(&mut self, event: &PointerEvent) -> Stages {
        let meter = Meter::start();
        FramePipeline::prepare_frame(&mut self.context);
        self.presenter
            .rebuild_if_needed(&mut self.context, &mut self.build);
        let rebuild = meter.stop();
        let meter = Meter::start();
        self.presenter.layout(self.viewport).expect("layout");
        let layout = meter.stop();
        let meter = Meter::start();
        self.presenter.dispatch_pointer_event(event);
        let dispatch = meter.stop();
        let meter = Meter::start();
        self.presenter.paint();
        let paint = meter.stop();
        FramePipeline::finish_frame(&mut self.context);
        Stages {
            rebuild,
            layout,
            dispatch,
            paint,
        }
    }

    /// The same frame with `event` dispatched after paint; the cost of that dispatch.
    fn dispatch_after_paint(&mut self, event: &PointerEvent) -> Cost {
        FramePipeline::prepare_frame(&mut self.context);
        self.presenter
            .rebuild_if_needed(&mut self.context, &mut self.build);
        self.presenter.layout(self.viewport).expect("layout");
        self.presenter.paint();
        let meter = Meter::start();
        self.presenter.dispatch_pointer_event(event);
        let dispatch = meter.stop();
        self.presenter.paint();
        FramePipeline::finish_frame(&mut self.context);
        dispatch
    }
}

/// Median stage costs per event over the rounds, in the runners' order
/// (`after_paint` false) or with each event dispatched after paint.
fn run_stages(fixture: Fixture, rows: usize, pointer: Point, after_paint: bool) -> Stages {
    let probe = Rc::new(Probe::default());
    let tree = Rc::clone(&probe);
    let mut staged = Staged::new(fixture, rows, move |_: &mut AppContext| {
        fixture.build(&tree, rows)
    });
    let mut rounds = Vec::new();
    for round in 0..WARMUP + SAMPLES {
        staged.setup(fixture.setup(round), &probe);
        let events = fixture.events(round, pointer);
        let mut total = Stages::default();
        for event in &events {
            if after_paint {
                total.dispatch = total.dispatch.plus(staged.dispatch_after_paint(event));
            } else {
                let stages = staged.frame(event);
                total = Stages {
                    rebuild: total.rebuild.plus(stages.rebuild),
                    layout: total.layout.plus(stages.layout),
                    dispatch: total.dispatch.plus(stages.dispatch),
                    paint: total.paint.plus(stages.paint),
                };
            }
        }
        fixture.check(round, pointer, rows, &seen(&probe, &staged.presenter));
        if round >= WARMUP {
            rounds.push(Stages {
                rebuild: total.rebuild.per(events.len()),
                layout: total.layout.per(events.len()),
                dispatch: total.dispatch.per(events.len()),
                paint: total.paint.per(events.len()),
            });
        }
    }
    let stage =
        |pick: fn(&Stages) -> Cost| median_cost(&rounds.iter().map(pick).collect::<Vec<_>>());
    Stages {
        rebuild: stage(|stages| stages.rebuild),
        layout: stage(|stages| stages.layout),
        dispatch: stage(|stages| stages.dispatch),
        paint: stage(|stages| stages.paint),
    }
}

/// `numerator / denominator`, or null when nothing was counted.
fn share(numerator: f64, denominator: f64) -> Value {
    if denominator > 0.0 {
        json!(rounded(numerator / denominator, 3))
    } else {
        Value::Null
    }
}

fn report(fixture: Fixture, rows: usize) -> Value {
    let frames = run_frames(fixture, rows);
    let load = load();
    let pointer = frames.pointer;
    let stages = run_stages(fixture, rows, pointer, false);
    let after_paint = run_stages(fixture, rows, pointer, true).dispatch;
    let hit_pass = Cost {
        instructions: stages.dispatch.instructions - after_paint.instructions,
        allocations: stages.dispatch.allocations - after_paint.allocations,
        ns: 0.0,
    };
    let canvas_calls = match fixture {
        Fixture::CaptureDrag | Fixture::Grid { .. } => {
            json!({"pointer_frame": frames.canvas_calls, "idle_frame": frames.idle_canvas_calls})
        }
        _ => Value::Null,
    };
    json!({
        "name": fixture.name(),
        "events_per_round": fixture.events(0, pointer).len(),
        "frame": frames.per_event.json_with_ms(load),
        "idle_frame": frames.idle.json_with_ms(load),
        "painted_scene": {"primitives": frames.primitives, "hit_regions": frames.hit_regions},
        "stages": {
            "rebuild": stages.rebuild.json(),
            "layout": stages.layout.json(),
            "dispatch": stages.dispatch.json(),
            "paint": stages.paint.json(),
        },
        "dispatch_after_paint": after_paint.json(),
        "hit_pass": {
            "instructions_m": rounded(hit_pass.instructions / 1e6, 3),
            "allocations": rounded(hit_pass.allocations, 1),
            "share": share(hit_pass.instructions, frames.per_event.instructions),
            "allocation_share": share(hit_pass.allocations, frames.per_event.allocations),
        },
        "canvas_calls_per_event": canvas_calls,
    })
}

fn main() {
    let rows = rows();
    let filter = std::env::var("POINTER_FIXTURE").unwrap_or_default();
    let load_start = load();
    let fixtures: Vec<Value> = ALL
        .into_iter()
        .filter(|fixture| fixture.name().contains(filter.as_str()))
        .map(|fixture| report(fixture, rows))
        .collect();
    let output = json!({
        "rows": rows,
        "viewport": [VIEWPORT.width, VIEWPORT.height],
        "instruction_counter": instructions() > 0,
        "load_1m": {"start": rounded(load_start, 1), "end": rounded(load(), 1)},
        "fixtures": fixtures,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&output).expect("JSON output")
    );
}
