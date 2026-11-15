//! egui side of the comparison. Prints one line per case, in the format of RUI's
//! `benches/compare_frame.rs`, whose fixtures these match: same content and sizes, a
//! 1440x900 window at scale 2, 14 px Arial text. Each frame changes one thing, runs
//! egui, tessellates, uploads and draws through `egui_wgpu::Renderer` into an
//! offscreen texture, and submits without waiting for the GPU.
//!
//! `cargo run --release -- <case>...` runs the named cases, `--list` lists them.
//! Instructions are the whole process's retired instructions; allocations are counted
//! by this binary's allocator; GPU time comes from the same Metal hook RUI and GPUI
//! use, `benchmarks/gpui_compare/gpu_time.rs`, summed over the command buffers wgpu
//! commits for a frame. Draw calls are not counted here.

use egui::{Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, Vec2};
use egui_wgpu::wgpu;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

mod capture;
#[path = "../../complex_text.rs"]
mod complex_text;
#[path = "../../gpui_compare/gpu_time.rs"]
mod gpu_time;

mod paths;

const CASES: [&str; 9] = [
    "startup",
    "list_full",
    "list_virtual_1k",
    "list_virtual_100k",
    "text_paragraphs",
    "text_complex",
    "quad_grid",
    "paths",
    "paths_icons",
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
/// A face: name, file and index in the file.
type Face = (&'static str, &'static str, u32);
/// RUI's default face, so both sides shape and rasterize the same glyphs.
const ARIAL: Face = ("arial", "/System/Library/Fonts/Supplemental/Arial.ttf", 0);
/// `text_complex`'s fallback chain: the faces RUI's fallback picks for each script
/// (Arial also has Arabic), in an order where each character's first face with it is
/// RUI's. egui takes the first face with the character.
const COMPLEX_FONTS: [Face; 5] = [
    ARIAL,
    ("hiragino", "/System/Library/Fonts/Hiragino Sans GB.ttc", 0),
    ("kohinoor", "/System/Library/Fonts/Kohinoor.ttc", 0),
    ("thonburi", "/System/Library/Fonts/ThonburiUI.ttc", 0),
    ("emoji", "/System/Library/Fonts/Apple Color Emoji.ttc", 0),
];
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
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

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

fn hex(color: u32) -> Color32 {
    Color32::from_rgb((color >> 16) as u8, (color >> 8) as u8, color as u8)
}

/// One row: a background, a title on the left and a time on the right.
fn row(ui: &mut egui::Ui, index: usize, selected: bool) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), ROW_HEIGHT),
        egui::Sense::hover(),
    );
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, hex(if selected { 0x2d3748 } else { 0x1a202c }));
    let font = FontId::proportional(FONT_SIZE);
    painter.text(
        rect.left_center(),
        Align2::LEFT_CENTER,
        format!("task {index}: fix the failing check"),
        font.clone(),
        Color32::WHITE,
    );
    painter.text(
        rect.right_center(),
        Align2::RIGHT_CENTER,
        format!("{} min", index % 60),
        font,
        Color32::WHITE,
    );
}

fn no_spacing(ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = Vec2::ZERO;
}

fn list(ui: &mut egui::Ui, selected: usize) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        no_spacing(ui);
        for index in 0..LIST_ROWS {
            row(ui, index, index == selected);
        }
    });
}

fn virtual_rows(ui: &mut egui::Ui, count: usize, selected: usize) {
    egui::ScrollArea::vertical()
        .vertical_scroll_offset(count as f32 / 2.0 * ROW_HEIGHT)
        .show_rows(ui, ROW_HEIGHT, count, |ui, range| {
            no_spacing(ui);
            for index in range {
                row(ui, index, index == selected);
            }
        });
}

/// Paragraph `index`: 8 to 127 words, the same on both sides.
fn paragraph(index: usize) -> String {
    let words = 8 + (index * 37) % 120;
    (0..words)
        .map(|word| WORDS[(index * 7 + word * 3) % WORDS.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

fn paragraphs(ui: &mut egui::Ui, revision: usize, paragraph: fn(usize) -> String) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Frame::NONE.inner_margin(16.0).show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 8.0);
            for index in 0..PARAGRAPHS {
                let mut content = paragraph(index);
                if index == EDITED_PARAGRAPH {
                    content.push_str(&format!(" revision {revision}"));
                }
                ui.add(egui::Label::new(content).wrap());
            }
        });
    });
}

fn cell_color(row: usize, column: usize) -> u32 {
    (((column * 255 / GRID.0) as u32) << 16) | (((row * 255 / GRID.1) as u32) << 8) | 0x60
}

fn quad_grid(ui: &mut egui::Ui, changed: usize) {
    let size = Vec2::new(GRID.0 as f32 * CELL.0, GRID.1 as f32 * CELL.1);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter();
    for row in 0..GRID.1 {
        for column in 0..GRID.0 {
            let min = rect.min + Vec2::new(column as f32 * CELL.0, row as f32 * CELL.1);
            let color = if row * GRID.0 + column == changed {
                0xffffff
            } else {
                cell_color(row, column)
            };
            painter.rect_filled(Rect::from_min_size(min, CELL.into()), 0.0, hex(color));
        }
    }
}

/// What one runtime keeps across frames: the egui context, the device and renderer.
struct Harness {
    ctx: egui::Context,
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: egui_wgpu::Renderer,
    texture: wgpu::Texture,
    target: wgpu::TextureView,
    start: Instant,
}

impl Harness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_fonts(&[ARIAL])
    }

    fn with_fonts(fonts: &[Face]) -> Result<Self, Box<dyn std::error::Error>> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
        let renderer = egui_wgpu::Renderer::new(&device, FORMAT, Default::default());
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("egui_compare target"),
            size: wgpu::Extent3d {
                width: (VIEWPORT.0 * SCALE_FACTOR) as u32,
                height: (VIEWPORT.1 * SCALE_FACTOR) as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target = texture.create_view(&Default::default());
        Ok(Self {
            ctx: context(fonts),
            device,
            queue,
            renderer,
            texture,
            target,
            start: Instant::now(),
        })
    }

    fn frame(&mut self, ui: impl FnMut(&mut egui::Ui)) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, VIEWPORT.into())),
            time: Some(self.start.elapsed().as_secs_f64()),
            ..Default::default()
        };
        let output = self.ctx.run_ui(input, ui);
        let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                self.renderer
                    .update_texture(&self.device, &self.queue, *id, delta);
            }
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [
                (VIEWPORT.0 * SCALE_FACTOR) as u32,
                (VIEWPORT.1 * SCALE_FACTOR) as u32,
            ],
            pixels_per_point: output.pixels_per_point,
        };
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut buffers = self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &primitives,
            &screen,
        );
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui_compare"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            self.renderer
                .render(&mut pass.forget_lifetime(), &primitives, &screen);
        }
        for id in &output.textures_delta.free {
            self.renderer.free_texture(id);
        }
        buffers.push(encoder.finish());
        self.queue.submit(buffers);
        // Retire finished work without waiting, as a window's present does.
        let _ = self.device.poll(wgpu::PollType::Poll);
    }
}

/// A context whose fonts are `faces`, tried in order.
fn context(faces: &[Face]) -> egui::Context {
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(SCALE_FACTOR);
    let mut fonts = FontDefinitions::empty();
    for &(name, path, index) in faces {
        let bytes = std::fs::read(path).unwrap_or_else(|_| panic!("{path} is a macOS system font"));
        let mut data = FontData::from_owned(bytes);
        data.index = index;
        fonts.font_data.insert(name.into(), Arc::new(data));
    }
    let chain: Vec<String> = faces.iter().map(|face| face.0.into()).collect();
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts.families.insert(family, chain.clone());
    }
    ctx.set_fonts(fonts);
    ctx.all_styles_mut(|style| {
        style.override_font_id = Some(FontId::proportional(FONT_SIZE));
        style.visuals.panel_fill = Color32::BLACK;
    });
    ctx
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
        gpu_time::set_frame(turn);
        let (i0, a0) = (rusage()[31], ALLOCATIONS.load(Ordering::Relaxed));
        let t0 = Instant::now();
        frame(harness, turn);
        ms.push(t0.elapsed().as_secs_f64() * 1e3);
        instructions.push(rusage()[31].saturating_sub(i0) as f64 / 1e6);
        allocations.push(ALLOCATIONS.load(Ordering::Relaxed).saturating_sub(a0) as f64);
    }
    let footprint = footprint_mb();
    std::thread::sleep(GPU_SETTLE);
    // wgpu commits several command buffers a frame: a frame's GPU time is the busy
    // time the buffers committed in it add together.
    let (gpu_p50, gpu_p95) = match gpu_time::take_frames(WARMUP..WARMUP + SAMPLES) {
        Some(mut gpu_ms) => (percentile(&mut gpu_ms, 0.5), percentile(&mut gpu_ms, 0.95)),
        None => {
            eprintln!("egui {case}: a frame completed no command buffer; GPU time not reported");
            (f64::NAN, f64::NAN)
        }
    };
    println!(
        "egui {case} instructions_m={:.2} ms_p50={:.3} ms_p95={:.3} allocations={:.0} footprint_mb={footprint:.1} gpu_ms_p50={gpu_p50:.3} gpu_ms_p95={gpu_p95:.3}",
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
            harness.frame(|ui| list(ui, 0));
            let first_frame = t0.elapsed().as_secs_f64() * 1e3;
            println!(
                "egui startup process_to_fixture_ms={to_fixture:.1} first_frame_ms={first_frame:.1} process_to_first_frame_ms={:.1} footprint_mb={:.1}",
                ms_since_process_start(),
                footprint_mb()
            );
        }
        "list_full" => {
            let mut harness = Harness::new()?;
            measure(case, &mut harness, |harness, turn| {
                harness.frame(|ui| list(ui, turn % LIST_ROWS));
            });
        }
        "list_virtual_1k" | "list_virtual_100k" => {
            let count = if case == "list_virtual_1k" {
                1_000
            } else {
                100_000
            };
            let mut harness = Harness::new()?;
            measure(case, &mut harness, |harness, turn| {
                let selected = count / 2 + turn % VISIBLE_ROWS;
                harness.frame(|ui| virtual_rows(ui, count, selected));
            });
        }
        // `text_complex` is `text_paragraphs` in seven scripts and emoji.
        "text_paragraphs" | "text_complex" => {
            let (content, fonts): (fn(usize) -> String, &[Face]) = if case == "text_paragraphs" {
                (paragraph, &[ARIAL])
            } else {
                (complex_text::complex_paragraph, &COMPLEX_FONTS)
            };
            let mut harness = Harness::with_fonts(fonts)?;
            measure(case, &mut harness, |harness, turn| {
                harness.frame(|ui| paragraphs(ui, turn, content));
            });
            capture::save_png_if_asked(
                case,
                &harness.device,
                &harness.queue,
                &harness.texture,
                (
                    (VIEWPORT.0 * SCALE_FACTOR) as u32,
                    (VIEWPORT.1 * SCALE_FACTOR) as u32,
                ),
            )?;
        }
        "quad_grid" => {
            let mut harness = Harness::new()?;
            measure(case, &mut harness, |harness, turn| {
                harness.frame(|ui| quad_grid(ui, turn % (GRID.0 * GRID.1)));
            });
        }
        "paths" => {
            let mut harness = Harness::new()?;
            let scene = paths::Paths::new();
            harness.frame(|ui| scene.paint(ui, 0));
            paths::save_png(&harness, case)?;
            measure(case, &mut harness, |harness, turn| {
                harness.frame(|ui| scene.paint(ui, turn));
            });
        }
        "paths_icons" => {
            let mut harness = Harness::new()?;
            let scene = paths::Icons::new();
            harness.frame(|ui| scene.paint(ui, 0));
            paths::save_png(&harness, case)?;
            measure(case, &mut harness, |harness, turn| {
                harness.frame(|ui| scene.paint(ui, turn));
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
