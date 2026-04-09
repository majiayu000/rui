use rui::core::entity::EntityStore;
use rui::core::geometry::{Point, Rect};
use std::hint::black_box;
use std::time::Instant;

#[derive(Clone)]
struct Counter {
    value: i32,
}

fn bench_get(iterations: usize, entity_count: usize) -> f64 {
    let mut store = EntityStore::new();
    let ids: Vec<_> = (0..entity_count)
        .map(|i| store.insert(Counter { value: i as i32 }))
        .collect();

    let start = Instant::now();
    let mut sum: i64 = 0;

    for _ in 0..iterations {
        for &id in &ids {
            let counter = store.get::<Counter>(id).unwrap();
            sum += counter.value as i64;
        }
    }

    black_box(sum);
    let elapsed = start.elapsed();
    elapsed.as_nanos() as f64 / (iterations * entity_count) as f64
}

fn bench_get_mut(iterations: usize, entity_count: usize) -> f64 {
    let mut store = EntityStore::new();
    let ids: Vec<_> = (0..entity_count)
        .map(|i| store.insert(Counter { value: i as i32 }))
        .collect();

    let start = Instant::now();

    for _ in 0..iterations {
        for &id in &ids {
            let mut counter = store.get_mut::<Counter>(id).unwrap();
            counter.value = counter.value.wrapping_add(1);
        }
    }

    let mut checksum: i64 = 0;
    for &id in &ids {
        checksum += store.get::<Counter>(id).unwrap().value as i64;
    }
    black_box(checksum);

    let elapsed = start.elapsed();
    elapsed.as_nanos() as f64 / (iterations * entity_count) as f64
}

fn bench_type_miss(iterations: usize, entity_count: usize) -> f64 {
    let mut store = EntityStore::new();
    let ids: Vec<_> = (0..entity_count)
        .map(|i| store.insert(Counter { value: i as i32 }))
        .collect();

    let start = Instant::now();
    let mut misses = 0usize;

    for _ in 0..iterations {
        for &id in &ids {
            if store.get::<String>(id).is_none() {
                misses += 1;
            }
        }
    }

    black_box(misses);
    let elapsed = start.elapsed();
    elapsed.as_nanos() as f64 / (iterations * entity_count) as f64
}

fn bench_rect_contains(iterations: usize, rect_count: usize) -> f64 {
    let rects: Vec<_> = (0..rect_count)
        .map(|i| {
            let x = (i % 512) as f32;
            let y = (i / 512) as f32;
            Rect::from_xywh(x, y, 64.0, 48.0)
        })
        .collect();
    let points: Vec<_> = (0..rect_count)
        .map(|i| Point::new((i % 512) as f32 + 24.0, (i / 512) as f32 + 12.0))
        .collect();

    let start = Instant::now();
    let mut hits = 0usize;
    for _ in 0..iterations {
        for i in 0..rect_count {
            if rects[i].contains(points[i]) {
                hits += 1;
            }
        }
    }
    black_box(hits);
    let elapsed = start.elapsed();
    elapsed.as_nanos() as f64 / (iterations * rect_count) as f64
}

fn bench_rect_intersection(iterations: usize, rect_count: usize) -> f64 {
    let rects_a: Vec<_> = (0..rect_count)
        .map(|i| {
            let x = (i % 512) as f32;
            let y = (i / 512) as f32;
            Rect::from_xywh(x, y, 80.0, 60.0)
        })
        .collect();
    let rects_b: Vec<_> = (0..rect_count)
        .map(|i| {
            let x = (i % 512) as f32 + 10.0;
            let y = (i / 512) as f32 + 8.0;
            Rect::from_xywh(x, y, 80.0, 60.0)
        })
        .collect();

    let start = Instant::now();
    let mut total_area = 0.0f32;
    for _ in 0..iterations {
        for i in 0..rect_count {
            if let Some(intersection) = rects_a[i].intersection(&rects_b[i]) {
                total_area += intersection.width() * intersection.height();
            }
        }
    }
    black_box(total_area);
    let elapsed = start.elapsed();
    elapsed.as_nanos() as f64 / (iterations * rect_count) as f64
}

fn main() {
    let entity_count = 20_000;
    let iterations = 100;
    let rect_count = 20_000;

    let get_ns = bench_get(iterations, entity_count);
    let get_mut_ns = bench_get_mut(iterations / 2, entity_count);
    let miss_ns = bench_type_miss(iterations, entity_count);
    let contains_ns = bench_rect_contains(iterations, rect_count);
    let intersection_ns = bench_rect_intersection(iterations, rect_count);

    println!("entity_get_ns_per_op={get_ns:.2}");
    println!("entity_get_mut_ns_per_op={get_mut_ns:.2}");
    println!("entity_type_miss_ns_per_op={miss_ns:.2}");
    println!("rect_contains_ns_per_op={contains_ns:.2}");
    println!("rect_intersection_ns_per_op={intersection_ns:.2}");
}
