//! What the text caches do while a window is resized and a list is scrolled.
//! Prints JSON and fails when text is shaped again that the caches should keep.
//!
//! `cargo bench --bench text_cache` (`TEXT_CACHE_ROWS=6000` for a longer list)
//!
//! Resize: paragraphs that wrap, laid out at a width that grows a pixel a frame.
//! Nothing is shaped again, and a paragraph is wrapped again only when one of its
//! lines changes.
//!
//! Scroll: the rows a viewport shows of a long list, built per frame the way a
//! virtual list does, scrolled to the end and back. Layout measures and the
//! renderer's glyph cache shape each row once while the list fits their targets
//! (4 096 strings); a longer list evicts the rows scrolled furthest away.
use rui::core::frame_pipeline::FramePipeline;
use rui::elements::element::LayoutNodes;
use rui::renderer::text::{TextCacheStats, TextGlyphCache, TextMeasureCache, TextRequest};
use rui::{Div, ElementId, Size, div, text};
use std::process::ExitCode;
use taffy::prelude::TaffyTree;

const PARAGRAPHS: usize = 200;
const RESIZE_FRAMES: usize = 200;
const START_WIDTH: f32 = 700.0;
const VIEWPORT_HEIGHT: f32 = 900.0;
const DEFAULT_ROWS: usize = 3_000;
const VISIBLE_ROWS: usize = 40;
const SCROLL_STEP: usize = 3;
const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 1.4;

unsafe extern "C" {
    fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut u64) -> i32;
}

/// Retired instructions of this process, `ri_instructions` of `rusage_info_v4`.
fn instructions() -> u64 {
    let mut words = [0u64; 64];
    // SAFETY: flavor 4 writes a rusage_info_v4, far shorter than 64 words.
    let status = unsafe { proc_pid_rusage(std::process::id() as i32, 4, words.as_mut_ptr()) };
    assert_eq!(status, 0, "proc_pid_rusage failed");
    words[31]
}

fn paragraph(index: usize) -> String {
    let words = [
        "layout",
        "shaping",
        "glyph",
        "atlas",
        "frame",
        "window",
        "paragraph",
        "wraps",
        "the",
        "same",
        "for",
        "a",
        "range",
        "of",
        "widths",
    ];
    let body = (0..40 + index % 30)
        .map(|word| words[(word * 7 + index) % words.len()])
        .collect::<Vec<_>>()
        .join(" ");
    format!("{index}: {body}")
}

fn paragraphs() -> Div {
    div()
        .w_full()
        .flex_col()
        .children((0..PARAGRAPHS).map(|index| text(paragraph(index)).wrap()))
}

fn row_text(row: usize) -> String {
    format!("task {row}: fix the failing check")
}

fn visible_rows(top: usize) -> Div {
    div()
        .w_full()
        .flex_col()
        .children((top..top + VISIBLE_ROWS).map(|row| div().h(28.0).child(text(row_text(row)))))
}

fn delta(after: TextCacheStats, before: TextCacheStats) -> TextCacheStats {
    TextCacheStats {
        shapes: after.shapes - before.shapes,
        measure_hits: after.measure_hits - before.measure_hits,
        measure_misses: after.measure_misses - before.measure_misses,
        wrap_hits: after.wrap_hits - before.wrap_hits,
        wrap_misses: after.wrap_misses - before.wrap_misses,
    }
}

fn stats_json(stats: TextCacheStats) -> String {
    format!(
        "{{\"shapes\": {}, \"measure_hits\": {}, \"measure_misses\": {}, \"wrap_hits\": {}, \"wrap_misses\": {}}}",
        stats.shapes, stats.measure_hits, stats.measure_misses, stats.wrap_hits, stats.wrap_misses
    )
}

fn main() -> ExitCode {
    let mut failures = Vec::new();

    // Resize.
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut nodes = LayoutNodes::new();
    let mut measurer = TextMeasureCache::new();
    let mut root = paragraphs();
    let mut layout = |root: &mut Div, measurer: &mut TextMeasureCache, width: f32| {
        FramePipeline::layout_root_retained(
            root,
            &mut taffy,
            &mut nodes,
            measurer,
            Size::new(width, VIEWPORT_HEIGHT),
        )
        .expect("layout");
    };
    layout(&mut root, &mut measurer, START_WIDTH);
    let first_frame = measurer.stats();
    let i0 = instructions();
    for frame in 1..=RESIZE_FRAMES {
        layout(&mut root, &mut measurer, START_WIDTH + frame as f32);
    }
    let resize_instructions = instructions() - i0;
    let resize = delta(measurer.stats(), first_frame);

    // How many paragraph wraps the resize changed, from a cache of its own.
    let mut reference = TextMeasureCache::new();
    let texts: Vec<String> = (0..PARAGRAPHS).map(paragraph).collect();
    let lines = |reference: &mut TextMeasureCache, content: &str, width: f32| {
        reference
            .wrap(
                TextRequest::new(content, FONT_SIZE, 400, None, LINE_HEIGHT),
                width,
                None,
            )
            .expect("wrap")
            .lines
    };
    let mut changed_wraps = 0;
    for frame in 1..=RESIZE_FRAMES {
        let (before, after) = (START_WIDTH + (frame - 1) as f32, START_WIDTH + frame as f32);
        changed_wraps += texts
            .iter()
            .filter(|content| {
                lines(&mut reference, content, before) != lines(&mut reference, content, after)
            })
            .count();
    }
    if resize.shapes != 0 {
        failures.push(format!("resize shaped {} strings again", resize.shapes));
    }
    if resize.wrap_misses > changed_wraps {
        failures.push(format!(
            "resize wrapped {} times, but only {changed_wraps} wraps changed",
            resize.wrap_misses
        ));
    }

    // Scroll.
    let rows = std::env::var("TEXT_CACHE_ROWS")
        .ok()
        .and_then(|rows| rows.parse().ok())
        .unwrap_or(DEFAULT_ROWS);
    let mut taffy = TaffyTree::<ElementId>::new();
    let mut nodes = LayoutNodes::new();
    let mut measurer = TextMeasureCache::new();
    let mut glyphs = TextGlyphCache::new();
    let last_top = rows - VISIBLE_ROWS;
    let tops: Vec<usize> = (0..=last_top)
        .step_by(SCROLL_STEP)
        .chain((0..=last_top).rev().step_by(SCROLL_STEP))
        .collect();
    let i0 = instructions();
    for &top in &tops {
        let mut root = visible_rows(top);
        FramePipeline::layout_root_retained(
            &mut root,
            &mut taffy,
            &mut nodes,
            &mut measurer,
            Size::new(START_WIDTH, VIEWPORT_HEIGHT),
        )
        .expect("layout");
        glyphs.begin_frame();
        for row in top..top + VISIBLE_ROWS {
            let content = row_text(row);
            glyphs
                .shape(TextRequest::new(
                    &content,
                    FONT_SIZE,
                    400,
                    None,
                    LINE_HEIGHT,
                ))
                .expect("shape");
        }
    }
    let scroll_instructions = instructions() - i0;
    let scroll = measurer.stats();
    if rows <= 4_096 && (scroll.shapes != rows || glyphs.shape_misses() != rows) {
        failures.push(format!(
            "scrolling {rows} rows shaped {} times in layout and {} in the glyph cache",
            scroll.shapes,
            glyphs.shape_misses()
        ));
    }

    println!("{{");
    println!(
        "  \"resize\": {{\"paragraphs\": {PARAGRAPHS}, \"frames\": {RESIZE_FRAMES}, \"changed_wraps\": {changed_wraps}, \"first_frame\": {}, \"resize_frames\": {}, \"instructions_m_per_frame\": {:.2}}},",
        stats_json(first_frame),
        stats_json(resize),
        resize_instructions as f64 / RESIZE_FRAMES as f64 / 1e6
    );
    println!(
        "  \"scroll\": {{\"rows\": {rows}, \"frames\": {}, \"layout\": {}, \"glyph_cache_shapes\": {}, \"instructions_m_per_frame\": {:.2}}}",
        tops.len(),
        stats_json(scroll),
        glyphs.shape_misses(),
        scroll_instructions as f64 / tops.len() as f64 / 1e6
    );
    println!("}}");

    if failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        for failure in failures {
            eprintln!("text_cache: {failure}");
        }
        ExitCode::FAILURE
    }
}
