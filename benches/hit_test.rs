//! Cost of pointer hit testing on a long list. Prints JSON.
//!
//! `cargo bench --bench hit_test` (`STRESS_ROWS=500` for one other length)
//!
//! Each row of the list registers a hit region, as a list of clickable rows does,
//! and the column registers one covering them all. The scene is rebuilt every
//! frame, so a frame's queries pay for anything `hit_test` builds; queries sweep
//! the viewport.
use rui::renderer::Scene;
use rui::{Bounds, ElementId, Point};
use std::hint::black_box;
use std::time::{Duration, Instant};

const ROWS: [usize; 3] = [200, 2_000, 20_000];
const ROW_HEIGHT: f32 = 28.0;
const SAMPLES: usize = 9;
const FRAMES: usize = 20;
const QUERIES_PER_FRAME: [usize; 3] = [1, 3, 100];
const VIEWPORT: (f32, f32) = (1440.0, 900.0);

fn rows() -> Vec<usize> {
    match std::env::var("STRESS_ROWS") {
        Ok(rows) => vec![rows.parse().expect("STRESS_ROWS must be a row count")],
        Err(_) => ROWS.to_vec(),
    }
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

fn query_point(index: usize) -> Point {
    let x = (index * 37 % 1_000) as f32 / 1_000.0 * VIEWPORT.0;
    let y = (index * 61 % 1_000) as f32 / 1_000.0 * VIEWPORT.1;
    Point::new(x, y)
}

/// Median over `SAMPLES` of the mean time `run` reports for one of `FRAMES` frames.
fn median_us(mut run: impl FnMut() -> Duration) -> f64 {
    run();
    let mut samples: Vec<f64> = (0..SAMPLES)
        .map(|_| {
            let total: Duration = (0..FRAMES).map(|_| run()).sum();
            total.as_secs_f64() * 1e6 / FRAMES as f64
        })
        .collect();
    samples.sort_by(f64::total_cmp);
    samples[SAMPLES / 2]
}

fn main() {
    let mut scene = Scene::new();
    let cases: Vec<String> = rows()
        .into_iter()
        .map(|rows| {
            let register_us = median_us(|| {
                let t0 = Instant::now();
                register(&mut scene, rows);
                t0.elapsed()
            });
            let queries: Vec<String> = QUERIES_PER_FRAME
                .iter()
                .map(|&count| {
                    let us = median_us(|| {
                        register(&mut scene, rows);
                        let t0 = Instant::now();
                        for index in 0..count {
                            let hit = black_box(scene.hit_test(query_point(index)));
                            assert!(hit.is_some(), "every query should hit a row");
                        }
                        t0.elapsed()
                    });
                    format!("\"{count}\": {us:.2}")
                })
                .collect();
            format!(
                "    {{\"rows\": {rows}, \"register_frame_us\": {register_us:.2}, \"queries_frame_us\": {{{}}}}}",
                queries.join(", ")
            )
        })
        .collect();

    println!("{{");
    println!("  \"cases\": [");
    println!("{}", cases.join(",\n"));
    println!("  ]");
    println!("}}");
}
