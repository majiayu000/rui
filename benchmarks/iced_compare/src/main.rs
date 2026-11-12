//! iced side of the comparison. Prints one line per case, in the format of RUI's
//! `benches/compare_frame.rs`, whose fixtures these match: same content and sizes, a
//! 1440x900 window at scale 2, 14 px Arial text. Each frame changes one thing, then,
//! as `iced_winit`'s event loop does for a redraw after a message: builds the view into
//! `iced_runtime`'s `UserInterface` against the kept widget tree (diff and layout),
//! updates it with a `RedrawRequested` event, draws it, and presents through
//! `iced_wgpu::Renderer` into an offscreen texture, submitting without waiting.
//!
//! `cargo run --release -- <case>...` runs the named cases, `--list` lists them.
//! Instructions are the whole process's retired instructions; allocations are counted
//! by this binary's allocator; GPU time comes from the same Metal hook RUI and GPUI
//! use, `benchmarks/gpui_compare/gpu_time.rs`, summed over the command buffers wgpu
//! commits for a frame. Draw calls are not counted here.

use iced_core::widget::operation::scrollable::{AbsoluteOffset, scroll_to};
use iced_core::{
    Alignment, Color, Element as CoreElement, Event, Font, Length, Pixels, Point, Rectangle, Size,
    clipboard, mouse, renderer, window,
};
use iced_graphics::Viewport;
use iced_runtime::user_interface::{Cache as UiCache, UserInterface};
use iced_wgpu::wgpu;
use iced_widget::canvas::{self, Cache as GeometryCache, Geometry};
use iced_widget::{Renderer, Theme, column, container, lazy, row, scrollable, space, text};
use std::alloc::{GlobalAlloc, Layout, System};
use std::borrow::Cow;
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

#[path = "../../gpui_compare/gpu_time.rs"]
mod gpu_time;

type Element<'a> = CoreElement<'a, (), Theme, Renderer>;

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
const VIEWPORT: (f32, f32) = (1440.0, 900.0);
const SCALE_FACTOR: f32 = 2.0;
const LIST_ROWS: usize = 2_000;
const ROW_HEIGHT: f32 = 28.0;
const VISIBLE_ROWS: usize = 32;
const FONT_SIZE: f32 = 14.0;
/// RUI's default face, so both sides shape and rasterize the same glyphs.
const FONT_PATH: &str = "/System/Library/Fonts/Supplemental/Arial.ttf";
const FONT: Font = Font::with_name("Arial");
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
const CELL: (f32, f32) = (7.0, 15.0);
/// What `iced_wgpu`'s compositor picks for a Metal surface.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
const SCROLLABLE: &str = "list";

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

fn hex(color: u32) -> Color {
    Color::from_rgb8((color >> 16) as u8, (color >> 8) as u8, color as u8)
}

/// One row: a background, a title on the left and a time on the right.
fn row_view(index: usize, selected: bool) -> Element<'static> {
    let background = hex(if selected { 0x2d3748 } else { 0x1a202c });
    container(
        row![
            text(format!("task {index}: fix the failing check")),
            space::horizontal(),
            text(format!("{} min", index % 60)),
        ]
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(ROW_HEIGHT)
    .align_y(Alignment::Center)
    .style(move |_| container::Style {
        background: Some(background.into()),
        ..container::Style::default()
    })
    .into()
}

/// A row that rebuilds only when its inputs change, as `lazy` keeps its element.
fn lazy_row(index: usize, selected: bool) -> Element<'static> {
    cached((index, selected), |&(index, selected)| {
        row_view(index, selected)
    })
}

fn cached<D: Hash + 'static>(
    dependency: D,
    view: impl Fn(&D) -> Element<'static> + 'static,
) -> Element<'static> {
    lazy(dependency, view).into()
}

fn list(selected: usize) -> Element<'static> {
    let rows = (0..LIST_ROWS).map(|index| lazy_row(index, index == selected));
    // `lazy` hints `Shrink`, so the column is told to fill or its rows get no width.
    scrollable(column(rows).width(Length::Fill))
        .id(SCROLLABLE)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// iced has no virtual list: the rows on screen sit between two spacers that stand
/// for the rows above and below, inside a `scrollable` whose offset picks the range.
fn virtual_rows(count: usize, offset: f32, selected: usize) -> Element<'static> {
    let first = ((offset / ROW_HEIGHT) as usize).min(count);
    let last = (first + (VIEWPORT.1 / ROW_HEIGHT).ceil() as usize + 1).min(count);
    let rows = (first..last).map(|index| lazy_row(index, index == selected));
    scrollable(
        column![space().height(first as f32 * ROW_HEIGHT)]
            .extend(rows)
            .push(space().height((count - last) as f32 * ROW_HEIGHT))
            .width(Length::Fill),
    )
    .id(SCROLLABLE)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// Paragraph `index`: 8 to 127 words, the same on both sides.
fn paragraph(index: usize) -> String {
    let words = 8 + (index * 37) % 120;
    (0..words)
        .map(|word| WORDS[(index * 7 + word * 3) % WORDS.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

fn paragraph_view(content: String) -> Element<'static> {
    text(content).size(FONT_SIZE).line_height(1.4).into()
}

fn paragraphs(revision: usize) -> Element<'static> {
    let texts = (0..PARAGRAPHS).map(move |index| {
        if index == EDITED_PARAGRAPH {
            let mut content = paragraph(index);
            content.push_str(&format!(" revision {revision}"));
            paragraph_view(content)
        } else {
            cached(index, |&index| paragraph_view(paragraph(index)))
        }
    });
    scrollable(column(texts).padding(16).spacing(8).width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn cell_color(row: usize, column: usize) -> u32 {
    (((column * 255 / GRID.0) as u32) << 16) | (((row * 255 / GRID.1) as u32) << 8) | 0x60
}

/// The grid: every cell in a geometry cache that is never cleared, and the one cell
/// that changes drawn on top of it each frame.
struct Grid<'a> {
    cells: &'a GeometryCache,
    changed: usize,
}

impl canvas::Program<(), Theme, Renderer> for Grid<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let size = Size::new(CELL.0, CELL.1);
        let origin =
            |row: usize, column: usize| Point::new(column as f32 * CELL.0, row as f32 * CELL.1);
        let cells = self.cells.draw(renderer, bounds.size(), |frame| {
            for row in 0..GRID.1 {
                for column in 0..GRID.0 {
                    frame.fill_rectangle(origin(row, column), size, hex(cell_color(row, column)));
                }
            }
        });
        let mut changed = canvas::Frame::new(renderer, bounds.size());
        changed.fill_rectangle(
            origin(self.changed / GRID.0, self.changed % GRID.0),
            size,
            Color::WHITE,
        );
        vec![cells, changed.into_geometry()]
    }
}

fn quad_grid(cells: &GeometryCache, changed: usize) -> Element<'_> {
    iced_widget::canvas(Grid { cells, changed })
        .width(GRID.0 as f32 * CELL.0)
        .height(GRID.1 as f32 * CELL.1)
        .into()
}

/// What one runtime keeps across frames: the widget tree, the device and renderer.
struct Harness {
    renderer: Renderer,
    device: wgpu::Device,
    cache: Option<UiCache>,
    target: wgpu::TextureView,
    viewport: Viewport,
    theme: Theme,
}

impl Harness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let arial = std::fs::read(FONT_PATH).expect("Arial is a macOS system font");
        iced_graphics::text::font_system()
            .write()
            .expect("font system")
            .load_font(Cow::Owned(arial));
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("iced_compare target"),
            size: wgpu::Extent3d {
                width: (VIEWPORT.0 * SCALE_FACTOR) as u32,
                height: (VIEWPORT.1 * SCALE_FACTOR) as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let target = texture.create_view(&Default::default());
        // No MSAA: iced's `Settings::antialiasing` is off by default.
        let engine = iced_wgpu::Engine::new(
            &adapter,
            device.clone(),
            queue,
            FORMAT,
            None,
            iced_graphics::Shell::headless(),
        );
        Ok(Self {
            renderer: Renderer::new(engine, FONT, Pixels(FONT_SIZE)),
            device,
            cache: Some(UiCache::new()),
            target,
            viewport: Viewport::with_physical_size(
                iced_core::Size::new(
                    (VIEWPORT.0 * SCALE_FACTOR) as u32,
                    (VIEWPORT.1 * SCALE_FACTOR) as u32,
                ),
                SCALE_FACTOR,
            ),
            theme: Theme::Dark,
        })
    }

    fn build<'a>(&mut self, view: Element<'a>) -> UserInterface<'a, (), Theme, Renderer> {
        UserInterface::build(
            view,
            self.viewport.logical_size(),
            self.cache.take().expect("cache returned after each frame"),
            &mut self.renderer,
        )
    }

    /// Scrolls the list's `scrollable` to `y`, as a `scroll_to` task does.
    fn scroll_to(&mut self, view: Element<'_>, y: f32) {
        let mut ui = self.build(view);
        let mut operation = scroll_to(
            SCROLLABLE.into(),
            AbsoluteOffset {
                x: None,
                y: Some(y),
            },
        );
        ui.operate(&self.renderer, &mut operation);
        self.cache = Some(ui.into_cache());
    }

    fn frame(&mut self, view: Element<'_>) {
        let mut ui = self.build(view);
        let mut messages = Vec::new();
        let _ = ui.update(
            &[Event::Window(window::Event::RedrawRequested(
                iced_core::time::Instant::now(),
            ))],
            mouse::Cursor::Unavailable,
            &mut self.renderer,
            &mut clipboard::Null,
            &mut messages,
        );
        ui.draw(
            &mut self.renderer,
            &self.theme,
            &renderer::Style {
                text_color: Color::WHITE,
            },
            mouse::Cursor::Unavailable,
        );
        self.cache = Some(ui.into_cache());
        self.renderer
            .present(Some(Color::BLACK), FORMAT, &self.target, &self.viewport);
        // Retire finished work without waiting, as a window's present does.
        let _ = self.device.poll(wgpu::PollType::Poll);
    }
}

fn measure(case: &str, harness: &mut Harness, mut frame: impl FnMut(&mut Harness, usize)) {
    gpu_time::install();
    for turn in 0..WARMUP {
        frame(harness, turn);
    }
    std::thread::sleep(GPU_SETTLE);
    gpu_time::take();
    let (mut ms, mut instructions, mut allocations) = (Vec::new(), Vec::new(), Vec::new());
    for turn in WARMUP..WARMUP + SAMPLES {
        let (i0, a0) = (rusage()[31], ALLOCATIONS.load(Ordering::Relaxed));
        let t0 = Instant::now();
        frame(harness, turn);
        ms.push(t0.elapsed().as_secs_f64() * 1e3);
        instructions.push(rusage()[31].saturating_sub(i0) as f64 / 1e6);
        allocations.push(ALLOCATIONS.load(Ordering::Relaxed).saturating_sub(a0) as f64);
    }
    let footprint = footprint_mb();
    std::thread::sleep(GPU_SETTLE);
    // A frame's GPU time is the busy time of the command buffers it commits together.
    let buffers = gpu_time::take();
    let mut gpu_ms: Vec<f64> = match buffers.len() / SAMPLES {
        per_frame if per_frame > 0 && buffers.len() % SAMPLES == 0 => buffers
            .chunks(per_frame)
            .map(|frame| frame.iter().sum())
            .collect(),
        _ => Vec::new(),
    };
    // Some frames commit an extra command buffer, so buffers cannot always
    // be split into frames; the mean busy time per frame is reported either way.
    let gpu_mean = buffers.iter().sum::<f64>() / SAMPLES as f64;
    let (gpu_p50, gpu_p95) = if gpu_ms.len() == SAMPLES {
        (percentile(&mut gpu_ms, 0.5), percentile(&mut gpu_ms, 0.95))
    } else {
        eprintln!(
            "iced {case}: {} command buffers completed for {SAMPLES} frames; GPU percentiles not reported",
            buffers.len()
        );
        (f64::NAN, f64::NAN)
    };
    println!(
        "iced {case} instructions_m={:.2} ms_p50={:.3} ms_p95={:.3} allocations={:.0} footprint_mb={footprint:.1} gpu_ms_p50={gpu_p50:.3} gpu_ms_p95={gpu_p95:.3} gpu_ms_mean={gpu_mean:.3}",
        percentile(&mut instructions, 0.5),
        percentile(&mut ms, 0.5),
        percentile(&mut ms, 0.95),
        percentile(&mut allocations, 0.5),
    );
}

fn run(case: &str) -> Result<(), Box<dyn std::error::Error>> {
    match case {
        "startup" => {
            let to_fixture = ms_since_process_start();
            let t0 = Instant::now();
            let mut harness = Harness::new()?;
            harness.frame(list(0));
            let first_frame = t0.elapsed().as_secs_f64() * 1e3;
            println!(
                "iced startup process_to_fixture_ms={to_fixture:.1} first_frame_ms={first_frame:.1} process_to_first_frame_ms={:.1} footprint_mb={:.1}",
                ms_since_process_start(),
                footprint_mb()
            );
        }
        "list_full" => {
            let mut harness = Harness::new()?;
            measure(case, &mut harness, |harness, turn| {
                harness.frame(list(turn % LIST_ROWS));
            });
        }
        "list_virtual_1k" | "list_virtual_100k" => {
            let count = if case == "list_virtual_1k" {
                1_000
            } else {
                100_000
            };
            let offset = count as f32 / 2.0 * ROW_HEIGHT;
            let mut harness = Harness::new()?;
            harness.scroll_to(virtual_rows(count, offset, 0), offset);
            measure(case, &mut harness, |harness, turn| {
                let selected = count / 2 + turn % VISIBLE_ROWS;
                harness.frame(virtual_rows(count, offset, selected));
            });
        }
        "text_paragraphs" => {
            let mut harness = Harness::new()?;
            measure(case, &mut harness, |harness, turn| {
                harness.frame(paragraphs(turn));
            });
        }
        "quad_grid" => {
            let mut harness = Harness::new()?;
            let cells = GeometryCache::new();
            measure(case, &mut harness, |harness, turn| {
                harness.frame(quad_grid(&cells, turn % (GRID.0 * GRID.1)));
            });
        }
        _ => return Err(format!("unknown case {case}; cases: {}", CASES.join(" ")).into()),
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|arg| arg == "--list") {
        println!("{}", CASES.join("\n"));
        return Ok(());
    }
    let named: Vec<String> = std::env::args().skip(1).collect();
    if named.is_empty() {
        CASES.iter().try_for_each(|case| run(case))
    } else {
        named.iter().try_for_each(|case| run(case))
    }
}
