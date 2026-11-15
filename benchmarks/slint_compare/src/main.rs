//! Slint side of the comparison. Prints one line per case, in the format of RUI's
//! `benches/compare_frame.rs`, whose fixtures these match: same content and sizes, a
//! 1440x900 window at scale 2, 14 px Arial text. The UI is `ui/fixtures.slint`.
//!
//! Two of Slint's renderers, chosen with `--renderer`:
//! - `software` (default): `MinimalSoftwareWindow` with a reused buffer, so Slint
//!   redraws only what changed into a 2880x1800 RGBA buffer this binary owns. CPU only.
//! - `femtovg`: `FemtoVGWGPURenderer` drawing into an offscreen wgpu texture on Metal
//!   and submitting without waiting for the GPU. It redraws the whole window each frame.
//!
//! Each frame changes one thing through the component's properties or models, then
//! updates timers and renders. `cargo run --release -- [--renderer R] <case>...` runs
//! the named cases, `--list` lists them; `SLINT_COMPARE_DUMP=out.ppm` saves the software
//! renderer's last frame. Instructions are the whole process's retired
//! instructions; allocations are counted by this binary's allocator; the FemtoVG GPU
//! time comes from the Metal hook RUI uses, `benchmarks/gpui_compare/gpu_time.rs`,
//! summed over the command buffers wgpu commits for a frame.

use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, PlatformError, WindowAdapter, WindowEvent};
use slint::wgpu_30::wgpu;
use slint::{ComponentHandle, Model, PhysicalSize, VecModel};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

#[path = "../../gpui_compare/gpu_time.rs"]
mod gpu_time;

slint::include_modules!();

const CASES: [&str; 6] = [
    "startup",
    "list_full",
    "list_virtual_1k",
    "list_virtual_100k",
    "text_paragraphs",
    "quad_grid",
];
const WARMUP: usize = 10;
const GPU_SETTLE: std::time::Duration = std::time::Duration::from_millis(100);
const SAMPLES: usize = 60;
const SCALE_FACTOR: f32 = 2.0;
const PIXELS: (u32, u32) = (2880, 1800);
const LIST_ROWS: usize = 2_000;
const ROW_HEIGHT: f32 = 28.0;
const VISIBLE_ROWS: usize = 32;
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
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

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

fn percentile(values: &mut [f64], fraction: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len() - 1) as f64 * fraction).round() as usize]
}

/// Paragraph `index`: 8 to 127 words, the same on both sides.
fn paragraph(index: usize) -> String {
    let words = 8 + (index * 37) % 120;
    (0..words)
        .map(|word| WORDS[(index * 7 + word * 3) % WORDS.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

fn cell_color(row: usize, column: usize) -> slint::Color {
    slint::Color::from_rgb_u8(
        (column * 255 / GRID.0) as u8,
        (row * 255 / GRID.1) as u8,
        0x60,
    )
}

#[derive(Clone, Copy, PartialEq)]
enum Renderer {
    Software,
    FemtoVG,
}

impl Renderer {
    fn label(self) -> &'static str {
        match self {
            Renderer::Software => "slint-software",
            Renderer::FemtoVG => "slint-femtovg-wgpu",
        }
    }
}

thread_local! {
    /// The window the next component gets; the platform hands it out.
    static NEXT_WINDOW: RefCell<Option<Rc<dyn WindowAdapter>>> = const { RefCell::new(None) };
}

struct BenchPlatform {
    start: Instant,
}

impl Platform for BenchPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        NEXT_WINDOW
            .with(|next| next.borrow_mut().take())
            .ok_or_else(|| PlatformError::Other("no window prepared".into()))
    }

    fn duration_since_start(&self) -> std::time::Duration {
        self.start.elapsed()
    }
}

/// A window that draws through `FemtoVGWGPURenderer` into a texture.
struct WgpuWindow {
    window: slint::Window,
    renderer: slint::platform::femtovg_renderer::FemtoVGWGPURenderer,
    size: Cell<PhysicalSize>,
}

impl WindowAdapter for WgpuWindow {
    fn window(&self) -> &slint::Window {
        &self.window
    }

    fn renderer(&self) -> &dyn slint::platform::Renderer {
        &self.renderer
    }

    fn size(&self) -> PhysicalSize {
        self.size.get()
    }

    fn set_size(&self, size: slint::WindowSize) {
        let scale = self.window.scale_factor();
        self.size.set(size.to_physical(scale));
        self.window.dispatch_event(WindowEvent::Resized {
            size: size.to_logical(scale),
        });
    }
}

struct Gpu {
    device: wgpu::Device,
    window: Rc<WgpuWindow>,
    target: wgpu::Texture,
}

/// What one runtime keeps across frames: the window, and its buffer or GPU target.
enum Target {
    Software {
        window: Rc<MinimalSoftwareWindow>,
        buffer: Vec<PremultipliedRgbaColor>,
    },
    Gpu(Box<Gpu>),
}

struct Harness {
    target: Target,
    /// Physical pixels the last frame redrew; the software renderer reports it.
    redrawn: u64,
}

impl Harness {
    /// Prepares the window that the next component created will get.
    fn new(renderer: Renderer) -> Result<Self, Box<dyn std::error::Error>> {
        let target = match renderer {
            Renderer::Software => {
                let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
                let buffer =
                    vec![PremultipliedRgbaColor::default(); (PIXELS.0 * PIXELS.1) as usize];
                NEXT_WINDOW.with(|next| *next.borrow_mut() = Some(window.clone()));
                Target::Software { window, buffer }
            }
            Renderer::FemtoVG => {
                let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                    backends: wgpu::Backends::METAL,
                    ..wgpu::InstanceDescriptor::new_without_display_handle()
                });
                let adapter =
                    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::HighPerformance,
                        ..Default::default()
                    }))?;
                let (device, queue) =
                    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
                let renderer = slint::platform::femtovg_renderer::FemtoVGWGPURenderer::new(
                    instance,
                    device.clone(),
                    queue,
                )?;
                let window = Rc::new_cyclic(|weak: &std::rc::Weak<WgpuWindow>| WgpuWindow {
                    window: slint::Window::new(weak.clone()),
                    renderer,
                    size: Cell::new(PhysicalSize::default()),
                });
                let target = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("slint_compare target"),
                    size: wgpu::Extent3d {
                        width: PIXELS.0,
                        height: PIXELS.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                NEXT_WINDOW.with(|next| *next.borrow_mut() = Some(window.clone()));
                Target::Gpu(Box::new(Gpu {
                    device,
                    window,
                    target,
                }))
            }
        };
        Ok(Self { target, redrawn: 0 })
    }

    fn window(&self) -> &slint::Window {
        match &self.target {
            Target::Software { window, .. } => window.window(),
            Target::Gpu(gpu) => gpu.window.window(),
        }
    }

    /// Shows `component` in this harness's window at 1440x900, scale 2.
    fn show(&self, component: &impl ComponentHandle) -> Result<(), Box<dyn std::error::Error>> {
        let window = self.window();
        window.dispatch_event(WindowEvent::ScaleFactorChanged {
            scale_factor: SCALE_FACTOR,
        });
        window.set_size(PhysicalSize::new(PIXELS.0, PIXELS.1));
        component.show()?;
        Ok(())
    }

    /// Writes the software renderer's buffer as a PPM image, to check what was drawn.
    fn dump(&self, path: std::ffi::OsString) -> std::io::Result<()> {
        let Target::Software { buffer, .. } = &self.target else {
            return Ok(());
        };
        let mut image = format!("P6 {} {} 255\n", PIXELS.0, PIXELS.1).into_bytes();
        for pixel in buffer {
            image.extend([pixel.red, pixel.green, pixel.blue]);
        }
        std::fs::write(path, image)
    }

    fn frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        slint::platform::update_timers_and_animations();
        match &mut self.target {
            Target::Software { window, buffer } => {
                let mut region = None;
                window.draw_if_needed(|renderer| {
                    region = Some(renderer.render(buffer, PIXELS.0 as usize));
                });
                self.redrawn = region.map_or(0, |region| {
                    region
                        .iter()
                        .map(|(_, size)| u64::from(size.width) * u64::from(size.height))
                        .sum()
                });
            }
            Target::Gpu(gpu) => {
                gpu.window.renderer.render_to_texture(&gpu.target)?;
                // Retire finished work without waiting, as a window's present does.
                let _ = gpu.device.poll(wgpu::PollType::Poll);
                self.redrawn = u64::from(PIXELS.0) * u64::from(PIXELS.1);
            }
        }
        Ok(())
    }
}

fn measure(
    renderer: Renderer,
    case: &str,
    harness: &mut Harness,
    mut change: impl FnMut(usize),
) -> Result<(), Box<dyn std::error::Error>> {
    let gpu = renderer == Renderer::FemtoVG;
    if gpu {
        gpu_time::install();
    }
    for turn in 0..WARMUP {
        change(turn);
        harness.frame()?;
    }
    if gpu {
        std::thread::sleep(GPU_SETTLE);
        gpu_time::take();
    }
    let (mut ms, mut instructions, mut allocations) = (Vec::new(), Vec::new(), Vec::new());
    let mut redrawn = Vec::new();
    for turn in WARMUP..WARMUP + SAMPLES {
        let (i0, a0) = (rusage()[31], ALLOCATIONS.load(Ordering::Relaxed));
        let t0 = Instant::now();
        change(turn);
        harness.frame()?;
        ms.push(t0.elapsed().as_secs_f64() * 1e3);
        instructions.push(rusage()[31].saturating_sub(i0) as f64 / 1e6);
        allocations.push(ALLOCATIONS.load(Ordering::Relaxed).saturating_sub(a0) as f64);
        redrawn.push(harness.redrawn as f64 / f64::from(PIXELS.0 * PIXELS.1) * 100.0);
    }
    let footprint = footprint_mb();
    if let Some(path) = std::env::var_os("SLINT_COMPARE_DUMP") {
        harness.dump(path)?;
    }
    let (gpu_p50, gpu_p95) = if gpu {
        std::thread::sleep(GPU_SETTLE);
        let buffers = gpu_time::take();
        let mut gpu_ms: Vec<f64> = match buffers.len() / SAMPLES {
            per_frame if per_frame > 0 && buffers.len() % SAMPLES == 0 => buffers
                .chunks(per_frame)
                .map(|frame| frame.iter().sum())
                .collect(),
            _ => Vec::new(),
        };
        if gpu_ms.len() == SAMPLES {
            (percentile(&mut gpu_ms, 0.5), percentile(&mut gpu_ms, 0.95))
        } else {
            eprintln!(
                "{} {case}: {} command buffers completed for {SAMPLES} frames; GPU time not reported",
                renderer.label(),
                buffers.len()
            );
            (f64::NAN, f64::NAN)
        }
    } else {
        (f64::NAN, f64::NAN)
    };
    println!(
        "{} {case} instructions_m={:.2} ms_p50={:.3} ms_p95={:.3} allocations={:.0} footprint_mb={footprint:.1} redrawn_pct={:.2} gpu_ms_p50={gpu_p50:.3} gpu_ms_p95={gpu_p95:.3}",
        renderer.label(),
        percentile(&mut instructions, 0.5),
        percentile(&mut ms, 0.5),
        percentile(&mut ms, 0.95),
        percentile(&mut allocations, 0.5),
        percentile(&mut redrawn, 0.5),
    );
    Ok(())
}

fn run(renderer: Renderer, case: &str) -> Result<(), Box<dyn std::error::Error>> {
    match case {
        // Process start to the first `list_full` frame rendered (software) or
        // submitted (FemtoVG), device creation included.
        "startup" => {
            let to_fixture = ms_since_process_start();
            let t0 = Instant::now();
            let mut harness = Harness::new(renderer)?;
            let ui = ListFull::new()?;
            harness.show(&ui)?;
            harness.frame()?;
            let first_frame = t0.elapsed().as_secs_f64() * 1e3;
            println!(
                "{} startup process_to_fixture_ms={to_fixture:.1} first_frame_ms={first_frame:.1} process_to_first_frame_ms={:.1} footprint_mb={:.1}",
                renderer.label(),
                ms_since_process_start(),
                footprint_mb()
            );
        }
        "list_full" => {
            let mut harness = Harness::new(renderer)?;
            let ui = ListFull::new()?;
            harness.show(&ui)?;
            measure(renderer, case, &mut harness, |turn| {
                ui.set_selected((turn % LIST_ROWS) as i32);
            })?;
        }
        "list_virtual_1k" | "list_virtual_100k" => {
            let count = if case == "list_virtual_1k" {
                1_000
            } else {
                100_000
            };
            let mut harness = Harness::new(renderer)?;
            let ui = ListVirtual::new()?;
            ui.set_count(count as i32);
            harness.show(&ui)?;
            ui.set_scroll(-(count as f32 / 2.0 * ROW_HEIGHT));
            measure(renderer, case, &mut harness, |turn| {
                ui.set_selected((count / 2 + turn % VISIBLE_ROWS) as i32);
            })?;
        }
        "text_paragraphs" => {
            let mut harness = Harness::new(renderer)?;
            let ui = Paragraphs::new()?;
            let model = Rc::new(VecModel::from(
                (0..PARAGRAPHS)
                    .map(|index| paragraph(index).into())
                    .collect::<Vec<slint::SharedString>>(),
            ));
            ui.set_paragraphs(model.clone().into());
            harness.show(&ui)?;
            let base = paragraph(EDITED_PARAGRAPH);
            measure(renderer, case, &mut harness, |turn| {
                model.set_row_data(EDITED_PARAGRAPH, format!("{base} revision {turn}").into());
            })?;
        }
        "quad_grid" => {
            let mut harness = Harness::new(renderer)?;
            let ui = QuadGrid::new()?;
            let cells = GRID.0 * GRID.1;
            let model = Rc::new(VecModel::from(
                (0..cells)
                    .map(|index| cell_color(index / GRID.0, index % GRID.0))
                    .collect::<Vec<_>>(),
            ));
            ui.set_cells(model.clone().into());
            harness.show(&ui)?;
            // The white cell moves by one each frame: the previous one gets its color back.
            measure(renderer, case, &mut harness, |turn| {
                let changed = turn % cells;
                let previous = (changed + cells - 1) % cells;
                model.set_row_data(previous, cell_color(previous / GRID.0, previous % GRID.0));
                model.set_row_data(changed, slint::Color::from_rgb_u8(255, 255, 255));
            })?;
        }
        _ => return Err(format!("unknown case {case}; cases: {}", CASES.join(" ")).into()),
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--list") {
        println!("{}", CASES.join("\n"));
        return Ok(());
    }
    let mut renderer = Renderer::Software;
    if let Some(at) = args.iter().position(|arg| arg == "--renderer") {
        renderer = match args.get(at + 1).map(String::as_str) {
            Some("software") => Renderer::Software,
            Some("femtovg") => Renderer::FemtoVG,
            other => return Err(format!("--renderer software|femtovg, not {other:?}").into()),
        };
        args.drain(at..at + 2);
    }
    slint::platform::set_platform(Box::new(BenchPlatform {
        start: Instant::now(),
    }))
    .map_err(|error| format!("{error:?}"))?;
    if args.is_empty() {
        CASES.iter().try_for_each(|case| run(renderer, case))
    } else {
        args.iter().try_for_each(|case| run(renderer, case))
    }
}
