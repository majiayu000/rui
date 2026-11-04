//! Cost of one frame on a long list, by stage. Prints JSON.
//!
//! `cargo bench --bench stress_frame` (`STRESS_ROWS=500` for another length)
//!
//! The rows, each a background, a title and a time, sit in one column. Nothing is
//! scrolled or culled by the benchmark: this measures what the runtime does with a
//! long list today. Instructions are the whole process's retired instructions. Layout
//! repeats within about 1%; the Metal stage moves more (671 M and 769 M in two runs of
//! one build), since the driver's threads count too. Milliseconds move with machine load.
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::{Element, LayoutNodes};
use rui::renderer::Scene;
use rui::renderer::metal::MetalRenderer;
use rui::renderer::text::TextMeasureCache;
use rui::{Bounds, Color, ElementId, Size, div, text};
use std::hint::black_box;
use std::time::Instant;
use taffy::prelude::TaffyTree;

const DEFAULT_ROWS: usize = 2_000;
const SAMPLES: usize = 9;
const VIEWPORT: (f32, f32) = (1440.0, 900.0);

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
fn instructions() -> u64 {
    rusage()[31]
}
fn footprint_mb() -> f64 {
    rusage()[9] as f64 / 1_048_576.0
}

fn rows() -> usize {
    std::env::var("STRESS_ROWS")
        .ok()
        .and_then(|rows| rows.parse().ok())
        .unwrap_or(DEFAULT_ROWS)
}

fn list(selected: usize) -> impl Element {
    div()
        .w_full()
        .flex_col()
        .children((0..rows()).map(move |row| {
            let background = if row == selected {
                Color::hex(0x2d3748)
            } else {
                Color::hex(0x1a202c)
            };
            div()
                .w_full()
                .h(28.0)
                .flex_row()
                .justify_between()
                .bg(background)
                .child(text(format!("task {row}: fix the failing check")))
                .child(text(format!("{} min", row % 60)))
        }))
}

struct Sample {
    ns: f64,
    instructions: f64,
}

fn median(mut samples: Vec<Sample>) -> Sample {
    samples.sort_by(|a, b| a.instructions.total_cmp(&b.instructions));
    samples.swap_remove(samples.len() / 2)
}

fn measure(mut run: impl FnMut(usize)) -> Sample {
    run(0);
    median(
        (1..=SAMPLES)
            .map(|round| {
                let (i0, t0) = (instructions(), Instant::now());
                run(round);
                Sample {
                    ns: t0.elapsed().as_nanos() as f64,
                    instructions: instructions().saturating_sub(i0) as f64,
                }
            })
            .collect(),
    )
}

fn line(name: &str, sample: &Sample, last: bool) {
    println!(
        "    \"{name}\": {{\"ms\": {:.2}, \"instructions_m\": {:.1}}}{}",
        sample.ns / 1e6,
        sample.instructions / 1e6,
        if last { "" } else { "," }
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let viewport = Size::new(VIEWPORT.0, VIEWPORT.1);
    let start_footprint = footprint_mb();
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut scene = Scene::new();

    let build = measure(|round| {
        black_box(list(round));
    });
    // The native runner keeps one measure cache across frames, so the frame measured
    // here does too. The first pass over new text is reported apart.
    let mut measurer = TextMeasureCache::new();
    let mut root = list(0);
    let t0 = Instant::now();
    let bounds: Bounds = FramePipeline::layout_root_with_text_measurer(
        &mut root,
        &mut taffy,
        &mut measurer,
        viewport,
    )?;
    let first_layout_ms = t0.elapsed().as_secs_f64() * 1e3;
    let footprint_after_layout = footprint_mb();
    let layout = measure(|_| {
        black_box(
            FramePipeline::layout_root_with_text_measurer(
                &mut root,
                &mut taffy,
                &mut measurer,
                viewport,
            )
            .unwrap(),
        );
    });
    // A kept layout tree, as the runtime has: each round selects another row, which
    // changes one row's background and no layout input.
    let mut retained_taffy = TaffyTree::<ElementId>::new();
    let mut nodes = LayoutNodes::new();
    let mut rounds: Vec<_> = (0..=SAMPLES).map(list).collect();
    let layout_retained = measure(|round| {
        black_box(
            FramePipeline::layout_root_retained(
                &mut rounds[round],
                &mut retained_taffy,
                &mut nodes,
                &mut measurer,
                viewport,
            )
            .unwrap(),
        );
    });
    drop(rounds);
    let paint = measure(|_| FramePipeline::paint_root(&mut root, &taffy, &mut scene, bounds));
    let primitives = scene.primitives().len();

    let t0 = Instant::now();
    let mut renderer = MetalRenderer::new()?;
    let renderer_new_ms = t0.elapsed().as_secs_f64() * 1e3;
    let t0 = Instant::now();
    let pixels = renderer.capture_frame_pixels(&scene, viewport)?;
    let first_render_ms = t0.elapsed().as_secs_f64() * 1e3;
    // Unchanged across a renderer optimisation means the optimisation drew the same frame.
    let pixel_checksum = pixels.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    let render = measure(|_| {
        black_box(renderer.capture_frame_pixels(&scene, viewport).unwrap());
    });

    println!("{{");
    println!("  \"rows\": {},", rows());
    println!("  \"primitives\": {primitives},");
    println!("  \"viewport\": [{}, {}],", VIEWPORT.0, VIEWPORT.1);
    println!("  \"stages\": {{");
    line("build", &build, false);
    line("layout", &layout, false);
    line("layout_retained", &layout_retained, false);
    line("paint", &paint, false);
    line("metal_render_with_readback", &render, true);
    println!("  }},");
    println!("  \"first_layout_ms\": {first_layout_ms:.1},");
    println!("  \"renderer_new_ms\": {renderer_new_ms:.1},");
    println!("  \"first_render_ms\": {first_render_ms:.1},");
    println!("  \"pixel_checksum\": \"{pixel_checksum:016x}\",");
    println!(
        "  \"footprint_mb\": {{\"start\": {start_footprint:.1}, \"after_layout\": {footprint_after_layout:.1}, \"end\": {:.1}}}",
        footprint_mb()
    );
    println!("}}");
    Ok(())
}
