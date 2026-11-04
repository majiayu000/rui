//! RUI side of the GPUI comparison: one full frame of a long list, end to end. Prints JSON.
//!
//! `cargo bench --bench compare_frame` (`STRESS_ROWS=500` for another length)
//!
//! The fixture matches `benchmarks/gpui_compare/long_list.rs`: rows of 28 px, a
//! background, a title and a time, in one column of a 1440x900 window at scale 2, with
//! nothing culled. Each frame selects another row, then rebuilds, lays out against the
//! kept layout tree, paints and submits to Metal without waiting, as GPUI's headless
//! renderer does. Instructions are the whole process's retired instructions, as GPUI's
//! `BENCH_MEASUREMENT=instructions` counts.
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::{Element, LayoutNodes};
use rui::renderer::Scene;
use rui::renderer::metal::MetalRenderer;
use rui::renderer::text::TextMeasureCache;
use rui::{Color, ElementId, Size, div, text};
use std::time::Instant;
use taffy::prelude::TaffyTree;

const DEFAULT_ROWS: usize = 2_000;
const WARMUP: usize = 5;
const SAMPLES: usize = 30;
const VIEWPORT: (f32, f32) = (1440.0, 900.0);
const SCALE_FACTOR: f32 = 2.0;

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

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let viewport = Size::new(VIEWPORT.0, VIEWPORT.1);
    let mut taffy = TaffyTree::<ElementId>::new();
    // The runtime keeps its layout tree across frames, so this does too.
    let mut nodes = LayoutNodes::new();
    let mut scene = Scene::new();
    // The native runner keeps one measure cache across frames, so this does too.
    let mut measurer = TextMeasureCache::new();
    let mut renderer = MetalRenderer::new()?;

    let mut frame = |selected: usize| -> Result<(), Box<dyn std::error::Error>> {
        let mut root = list(selected);
        let bounds = FramePipeline::layout_root_retained(
            &mut root,
            &mut taffy,
            &mut nodes,
            &mut measurer,
            viewport,
        )?;
        FramePipeline::paint_root(&mut root, &taffy, &mut scene, bounds);
        renderer.render_offscreen(&scene, viewport, SCALE_FACTOR)?;
        Ok(())
    };

    let t0 = Instant::now();
    frame(0)?;
    let first_frame_ms = t0.elapsed().as_secs_f64() * 1e3;
    for round in 1..=WARMUP {
        frame(round)?;
    }
    let (mut ms, mut instructions) = (Vec::new(), Vec::new());
    for round in 0..SAMPLES {
        let (i0, t0) = (rusage()[31], Instant::now());
        frame(WARMUP + 1 + round)?;
        ms.push(t0.elapsed().as_secs_f64() * 1e3);
        instructions.push(rusage()[31].saturating_sub(i0) as f64 / 1e6);
    }

    println!("{{");
    println!("  \"framework\": \"rui\",");
    println!("  \"rows\": {},", rows());
    println!(
        "  \"full_redraw\": {{\"ms\": {:.2}, \"instructions_m\": {:.1}}},",
        median(ms),
        median(instructions)
    );
    println!("  \"first_frame_ms\": {first_frame_ms:.1},");
    println!(
        "  \"footprint_mb\": {:.1}",
        rusage()[9] as f64 / 1_048_576.0
    );
    println!("}}");
    Ok(())
}
