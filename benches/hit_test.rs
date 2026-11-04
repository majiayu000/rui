//! Cost of pointer hit testing on a long list. Prints JSON.
//!
//! `cargo bench --bench hit_test` (`STRESS_ROWS=500` for another length)
//!
//! Each row of the list registers a hit region, as a list of clickable rows does,
//! and the column registers one covering them all. Queries sweep the viewport.
use rui::renderer::Scene;
use rui::{Bounds, ElementId, Point};
use std::hint::black_box;
use std::time::Instant;

const DEFAULT_ROWS: usize = 2_000;
const ROW_HEIGHT: f32 = 28.0;
const SAMPLES: usize = 9;
const QUERIES: usize = 10_000;
const VIEWPORT: (f32, f32) = (1440.0, 900.0);

fn rows() -> usize {
    std::env::var("STRESS_ROWS")
        .ok()
        .and_then(|rows| rows.parse().ok())
        .unwrap_or(DEFAULT_ROWS)
}

fn register(scene: &mut Scene, rows: usize) {
    scene.clear();
    scene.register_hit_region(
        ElementId::from(1),
        Bounds::from_xywh(0.0, 0.0, VIEWPORT.0, rows as f32 * ROW_HEIGHT),
    );
    for row in 0..rows {
        scene.register_hit_region(
            ElementId::from(row as u64 + 2),
            Bounds::from_xywh(0.0, row as f32 * ROW_HEIGHT, VIEWPORT.0, ROW_HEIGHT),
        );
    }
}

/// Median nanoseconds of `SAMPLES` timed runs, each divided by `per`.
fn median_ns(per: usize, mut run: impl FnMut()) -> f64 {
    run();
    let mut samples: Vec<f64> = (0..SAMPLES)
        .map(|_| {
            let t0 = Instant::now();
            run();
            t0.elapsed().as_nanos() as f64 / per as f64
        })
        .collect();
    samples.sort_by(f64::total_cmp);
    samples[SAMPLES / 2]
}

fn main() {
    let rows = rows();
    let mut scene = Scene::new();
    let register_ns = median_ns(1, || register(&mut scene, rows));

    let points: Vec<Point> = (0..QUERIES)
        .map(|index| {
            let x = (index * 37 % 1_000) as f32 / 1_000.0 * VIEWPORT.0;
            let y = (index * 61 % 1_000) as f32 / 1_000.0 * VIEWPORT.1;
            Point::new(x, y)
        })
        .collect();
    let mut hits = 0usize;
    let hit_test_ns = median_ns(QUERIES, || {
        for point in &points {
            hits += black_box(scene.hit_test(*point)).is_some() as usize;
        }
    });
    assert_eq!(
        hits,
        QUERIES * (SAMPLES + 1),
        "every query should hit a row"
    );

    println!("{{");
    println!("  \"rows\": {rows},");
    println!("  \"register_frame_us\": {:.2},", register_ns / 1e3);
    println!("  \"hit_test_ns\": {hit_test_ns:.1}");
    println!("}}");
}
