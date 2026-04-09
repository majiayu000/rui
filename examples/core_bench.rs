use rui::core::entity::EntityStore;
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

fn main() {
    let entity_count = 20_000;
    let iterations = 100;

    let get_ns = bench_get(iterations, entity_count);
    let get_mut_ns = bench_get_mut(iterations / 2, entity_count);
    let miss_ns = bench_type_miss(iterations, entity_count);

    println!("entity_get_ns_per_op={get_ns:.2}");
    println!("entity_get_mut_ns_per_op={get_mut_ns:.2}");
    println!("entity_type_miss_ns_per_op={miss_ns:.2}");
}
