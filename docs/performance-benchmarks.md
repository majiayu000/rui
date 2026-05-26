# Performance Benchmarks

`runtime_baselines` is a stable, CI-friendly benchmark harness for the runtime paths that are most likely to regress user-facing responsiveness.

Run the full benchmark report with:

```sh
cargo bench --bench runtime_baselines
```

The command prints JSON with one result per benchmark. Each result includes `median`, `p95`, the committed baseline values, percent deltas, and a `status` field. Status values are `stable`, `warning`, `measurement_noise`, and `blocking`.

The committed baseline lives in `benchmarks/runtime_baselines.json`. It records the baseline revision, capture environment, warning threshold, blocking thresholds, and one baseline row for each benchmark category:

- `layout.flex_tree`
- `text.measure_raster`
- `scene.build`
- `event.pointer_dispatch`
- `renderer.recording_throughput`

Threshold policy:

- Warning: median slower by more than 15 percent.
- Blocking: median slower by more than 25 percent and p95 slower by more than 30 percent.
- Measurement noise: only one blocking signal is crossed.

`enforcement_enabled` is currently `false`, so local and CI runs report status without failing the command. Enable enforcement only after the baseline is refreshed on stable CI hardware and noise has been observed across several runs.

Validate the baseline schema and threshold policy with:

```sh
cargo test benchmark_config
```

Refresh flow:

1. Run `cargo bench --bench runtime_baselines` on the baseline machine.
2. Copy the new `median` and `p95` values into `benchmarks/runtime_baselines.json`.
3. Update `generated_at`, `baseline_revision`, and environment notes.
4. Run `cargo test benchmark_config`, `cargo check`, and the benchmark command again.
