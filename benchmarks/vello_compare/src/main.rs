//! Vello side of the vector comparison. Prints one line per case, in the format of
//! RUI's `benches/compare_frame.rs` and `benchmarks/egui_compare`, whose `paths` and
//! `paths_icons` cases these match: the scenes of `benchmarks/paths_scene.rs` in a
//! 1440x900 window at scale 2. Each frame changes one shape, encodes a `vello::Scene`
//! of kurbo shapes, and renders it with `vello::Renderer` (area antialiasing) into an
//! offscreen `Rgba8Unorm` texture without waiting for the GPU.
//!
//! `cargo run --release -- <case>...` runs the named cases, `--list` lists them.
//! Instructions are the whole process's retired instructions; allocations are counted
//! by this binary's allocator; GPU time comes from the same Metal hook RUI and egui
//! use, `benchmarks/gpui_compare/gpu_time.rs`, summed over the command buffers wgpu
//! commits for a frame. `COMPARE_PNG=<dir>` also saves frame 0 of each case as
//! `<dir>/vello_<case>.png`.

#[path = "../../gpui_compare/gpu_time.rs"]
mod gpu_time;
#[path = "../../paths_scene.rs"]
mod paths_scene;

use paths_scene::{Outline, Segment, Shape};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use vello::kurbo::{Affine, BezPath, Cap, Circle, Join, Stroke};
use vello::peniko::{Color, Fill};
use vello::wgpu;
use vello::{AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene};

const CASES: [&str; 2] = ["paths", "paths_icons"];
const WARMUP: usize = 10;
const GPU_SETTLE: std::time::Duration = std::time::Duration::from_millis(100);
const SAMPLES: usize = 60;
const VIEWPORT: (f32, f32) = (1440.0, 900.0);
const SCALE_FACTOR: f64 = 2.0;
const WIDTH: u32 = (VIEWPORT.0 as f64 * SCALE_FACTOR) as u32;
const HEIGHT: u32 = (VIEWPORT.1 as f64 * SCALE_FACTOR) as u32;
const BACKGROUND: u32 = 0x101418;

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

/// `rusage_info_v4` read as words: index 9 is `ri_phys_footprint`, 31
/// `ri_instructions`.
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

fn percentile(values: &mut [f64], fraction: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    values[((values.len() - 1) as f64 * fraction).round() as usize]
}

fn color(rgba: [u8; 4]) -> Color {
    Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3])
}

fn bez_path(outline: &Outline) -> BezPath {
    let point = |p: [f32; 2]| (f64::from(p[0]), f64::from(p[1]));
    let mut path = BezPath::new();
    path.move_to(point(outline.start));
    for segment in &outline.segments {
        match *segment {
            Segment::Line(p) => path.line_to(point(p)),
            Segment::Cubic(a, b, p) => path.curve_to(point(a), point(b), point(p)),
        }
    }
    if outline.closed {
        path.close_path();
    }
    path
}

/// `shape` into `scene` under `transform`: nonzero fills, miter-joined butt strokes.
fn draw(scene: &mut Scene, transform: Affine, shape: &Shape) {
    match shape {
        Shape::Fill { outline, color: c } => {
            scene.fill(
                Fill::NonZero,
                transform,
                color(*c),
                None,
                &bez_path(outline),
            );
        }
        Shape::Stroke {
            outline,
            width,
            color: c,
        } => {
            let stroke = Stroke::new(f64::from(*width))
                .with_join(Join::Miter)
                .with_miter_limit(f64::from(paths_scene::MITER_LIMIT))
                .with_caps(Cap::Butt);
            scene.stroke(&stroke, transform, color(*c), None, &bez_path(outline));
        }
        Shape::Circle {
            center,
            radius,
            color: c,
        } => {
            let circle = Circle::new(
                (f64::from(center[0]), f64::from(center[1])),
                f64::from(*radius),
            );
            scene.fill(Fill::NonZero, transform, color(*c), None, &circle);
        }
    }
}

/// What one runtime keeps across frames: the device, renderer, scene and target.
struct Harness {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
    scene: Scene,
    texture: wgpu::Texture,
    target: wgpu::TextureView,
}

impl Harness {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
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
        let renderer = Renderer::new(
            &device,
            RendererOptions {
                antialiasing_support: AaSupport::area_only(),
                ..Default::default()
            },
        )?;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("vello_compare target"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target = texture.create_view(&Default::default());
        Ok(Self {
            device,
            queue,
            renderer,
            scene: Scene::new(),
            texture,
            target,
        })
    }

    /// Encodes the scene `build` draws and renders it, without waiting.
    fn frame(&mut self, build: impl FnOnce(&mut Scene)) -> Result<(), Box<dyn std::error::Error>> {
        self.scene.reset();
        build(&mut self.scene);
        let params = RenderParams {
            base_color: Color::from_rgb8(
                (BACKGROUND >> 16) as u8,
                (BACKGROUND >> 8) as u8,
                BACKGROUND as u8,
            ),
            width: WIDTH,
            height: HEIGHT,
            antialiasing_method: AaConfig::Area,
        };
        self.renderer.render_to_texture(
            &self.device,
            &self.queue,
            &self.scene,
            &self.target,
            &params,
        )?;
        // Retire finished work without waiting, as a window's present does.
        let _ = self.device.poll(wgpu::PollType::Poll);
        Ok(())
    }

    /// Saves the last frame as a PNG when `COMPARE_PNG` names a directory.
    fn save_png(&self, case: &str) -> Result<(), Box<dyn std::error::Error>> {
        let Some(dir) = std::env::var_os("COMPARE_PNG") else {
            return Ok(());
        };
        let row = WIDTH * 4;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vello_compare readback"),
            size: u64::from(row * HEIGHT),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row),
                    rows_per_image: None,
                },
            },
            self.texture.size(),
        );
        self.queue.submit([encoder.finish()]);
        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely())?;
        let pixels = buffer.slice(..).get_mapped_range()?.to_vec();
        let path = std::path::Path::new(&dir).join(format!("vello_{case}.png"));
        image::save_buffer(&path, &pixels, WIDTH, HEIGHT, image::ColorType::Rgba8)?;
        eprintln!("saved {}", path.display());
        Ok(())
    }
}

fn measure(
    case: &str,
    harness: &mut Harness,
    mut frame: impl FnMut(&mut Harness, usize) -> Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    gpu_time::install();
    for turn in 0..WARMUP {
        frame(harness, turn)?;
    }
    std::thread::sleep(GPU_SETTLE);
    gpu_time::take();
    let (mut ms, mut instructions, mut allocations) = (Vec::new(), Vec::new(), Vec::new());
    for turn in WARMUP..WARMUP + SAMPLES {
        let (i0, a0) = (rusage()[31], ALLOCATIONS.load(Ordering::Relaxed));
        let t0 = Instant::now();
        frame(harness, turn)?;
        ms.push(t0.elapsed().as_secs_f64() * 1e3);
        instructions.push(rusage()[31].saturating_sub(i0) as f64 / 1e6);
        allocations.push(ALLOCATIONS.load(Ordering::Relaxed).saturating_sub(a0) as f64);
    }
    let footprint = footprint_mb();
    std::thread::sleep(GPU_SETTLE);
    // wgpu may commit several command buffers a frame, the same number each frame:
    // a frame's GPU time is the busy time they add together.
    let buffers = gpu_time::take();
    let mut gpu_ms: Vec<f64> = match buffers.len() / SAMPLES {
        per_frame if per_frame > 0 && buffers.len() % SAMPLES == 0 => buffers
            .chunks(per_frame)
            .map(|frame| frame.iter().sum())
            .collect(),
        _ => Vec::new(),
    };
    let (gpu_p50, gpu_p95) = if gpu_ms.len() == SAMPLES {
        (percentile(&mut gpu_ms, 0.5), percentile(&mut gpu_ms, 0.95))
    } else {
        eprintln!(
            "vello {case}: {} command buffers completed for {SAMPLES} frames; GPU time not reported",
            buffers.len()
        );
        (f64::NAN, f64::NAN)
    };
    println!(
        "vello {case} instructions_m={:.2} ms_p50={:.3} ms_p95={:.3} allocations={:.0} footprint_mb={footprint:.1} gpu_ms_p50={gpu_p50:.3} gpu_ms_p95={gpu_p95:.3}",
        percentile(&mut instructions, 0.5),
        percentile(&mut ms, 0.5),
        percentile(&mut ms, 0.95),
        percentile(&mut allocations, 0.5),
    );
    Ok(())
}

fn run(case: &str) -> Result<(), Box<dyn std::error::Error>> {
    let scale = Affine::scale(SCALE_FACTOR);
    match case {
        // The shapes are kept; each frame encodes them again, the moving one turned.
        "paths" => {
            let mut harness = Harness::new()?;
            let shapes = paths_scene::paths();
            let frame = |harness: &mut Harness, turn: usize| {
                let moving = paths_scene::moving(&shapes, turn);
                harness.frame(|scene| {
                    for (index, shape) in shapes.iter().enumerate() {
                        let shape = if index == paths_scene::MOVING {
                            &moving
                        } else {
                            shape
                        };
                        draw(scene, scale, shape);
                    }
                })
            };
            frame(&mut harness, 0)?;
            harness.save_png(case)?;
            measure(case, &mut harness, frame)?;
        }
        // Each design is encoded once into a scene of its own, as an SVG loader
        // does; a frame appends the 300 icons with their transforms.
        "paths_icons" => {
            let mut harness = Harness::new()?;
            let designs: Vec<Scene> = paths_scene::icon_designs()
                .iter()
                .map(|design| {
                    let mut scene = Scene::new();
                    for shape in design {
                        draw(&mut scene, Affine::IDENTITY, shape);
                    }
                    scene
                })
                .collect();
            let icon_scale = f64::from(paths_scene::ICON_SIZE / paths_scene::ICON_BOX);
            let count = paths_scene::ICON_GRID.0 * paths_scene::ICON_GRID.1;
            let frame = |harness: &mut Harness, turn: usize| {
                harness.frame(|scene| {
                    for index in 0..count {
                        let (design, [x, y]) = paths_scene::icon_place(index, turn);
                        let place = scale
                            * Affine::translate((f64::from(x), f64::from(y)))
                            * Affine::scale(icon_scale);
                        scene.append(&designs[design], Some(place));
                    }
                })
            };
            frame(&mut harness, 0)?;
            harness.save_png(case)?;
            measure(case, &mut harness, frame)?;
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
