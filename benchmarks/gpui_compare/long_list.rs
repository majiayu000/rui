//! GPUI side of the RUI comparison: the long-list fixture from `benches/compare_frame.rs`
//! in RUI, drawn by GPUI through its headless Metal renderer.
//!
//! `run.sh` in RUI copies this file into `crates/gpui_platform/benches` of a pinned Zed
//! checkout. Keep the fixture identical to RUI's: rows of 28 px, a background, a title
//! and a time, in one column of a 1440x900 window, with nothing culled.
//!
//! - `full_redraw`: the root is notified, so every row is rebuilt, laid out and painted.
//! - `one_row_cached`: each row is its own view drawn with `.cached(..)`, and only one
//!   row is notified. This is GPUI's own incremental path, written by hand.

use std::time::Duration;

use gpui::{
    AnyView, AppContext as _, BenchAppContext, Context, Entity, IntoElement, ParentElement, Render, Styled,
    Window, div, px, rgb, size,
};

fn rows() -> usize {
    std::env::var("STRESS_ROWS")
        .ok()
        .and_then(|rows| rows.parse().ok())
        .unwrap_or(2_000)
}

fn row_element(index: usize, selected: bool) -> impl IntoElement {
    div()
        .w_full()
        .h(px(28.))
        .flex()
        .flex_row()
        .justify_between()
        .bg(rgb(if selected { 0x2d3748 } else { 0x1a202c }))
        .child(format!("task {index}: fix the failing check"))
        .child(format!("{} min", index % 60))
}

struct List {
    selected: usize,
}

impl Render for List {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let selected = self.selected;
        div()
            .size_full()
            .flex()
            .flex_col()
            .children((0..rows()).map(|index| row_element(index, index == selected)))
    }
}

struct Row {
    index: usize,
    selected: bool,
}

impl Render for Row {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        row_element(self.index, self.selected)
    }
}

struct CachedList {
    rows: Vec<Entity<Row>>,
    turn: usize,
}

impl Render for CachedList {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().flex().flex_col().children(self.rows.iter().map(|row| {
            AnyView::from(row.clone()).cached(gpui::StyleRefinement::default().w_full().h(px(28.)))
        }))
    }
}

unsafe extern "C" {
    fn proc_pid_rusage(pid: i32, flavor: i32, buffer: *mut u64) -> i32;
}

/// `ri_phys_footprint` from `rusage_info_v4`, as RUI's `compare_frame` reads it.
fn footprint_mb() -> f64 {
    let mut words = [0u64; 64];
    // SAFETY: flavor 4 writes a rusage_info_v4, far shorter than 64 words.
    let status = unsafe { proc_pid_rusage(std::process::id() as i32, 4, words.as_mut_ptr()) };
    assert_eq!(status, 0, "proc_pid_rusage failed");
    words[9] as f64 / 1_048_576.0
}

fn viewport() -> gpui::Size<gpui::Pixels> {
    size(px(1440.), px(900.))
}

#[gpui::bench(fps = 120)]
fn full_redraw(cx: &mut BenchAppContext) {
    let mut window = cx.add_empty_window();
    window.simulate_resize(viewport());
    let root = window.update(|window, cx| window.replace_root(cx, |_, _| List { selected: 0 }));
    cx.run_until_idle();
    cx.bench_renderer(root, |list, _, cx| {
        list.selected = (list.selected + 1) % rows();
        cx.notify();
    });
    eprintln!("long_list full_redraw footprint_mb={:.1}", footprint_mb());
    window.update(|window, _| window.remove_window());
    cx.run_until_idle();
}

#[gpui::bench(fps = 120)]
fn one_row_cached(cx: &mut BenchAppContext) {
    let mut window = cx.add_empty_window();
    window.simulate_resize(viewport());
    let root = window.update(|window, cx| {
        window.replace_root(cx, |_, cx| CachedList {
            rows: (0..rows())
                .map(|index| {
                    cx.new(|_| Row {
                        index,
                        selected: index == 0,
                    })
                })
                .collect(),
            turn: 0,
        })
    });
    cx.run_until_idle();
    cx.bench_renderer(root, |list, _, cx| {
        let row = list.rows[list.turn % list.rows.len()].clone();
        list.turn += 1;
        row.update(cx, |row, cx| {
            row.selected = !row.selected;
            cx.notify();
        });
    });
    eprintln!("long_list one_row_cached footprint_mb={:.1}", footprint_mb());
    window.update(|window, _| window.remove_window());
    cx.run_until_idle();
}

gpui::bench_group!(
    name = benches;
    config = criterion::Criterion::default()
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
        .sample_size(20);
    targets = full_redraw, one_row_cached
);
gpui::bench_main!(benches);
