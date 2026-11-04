# Performance Benchmarks

## Performance gate (CI)

`perf_gate` fails when a frame costs more than `benchmarks/perf_budgets.json` allows. CI runs it after the tests:

```sh
cargo bench --bench perf_gate
```

| Check | Where | Why |
|---|---|---|
| Heap allocations per frame, by case | CI and local | Counted by the gate's own allocator; exact on any machine. |
| CPU instructions per frame (build, layout, paint), by case | Wherever the machine exposes the counter | Repeat within about 1% locally. Hosted VMs may not expose the counter; the report then says the instruction budgets were not checked. |
| `virtual_list` at 100 000 rows vs 1 000 | CI and local | By instructions, or by allocations without the counter. Must stay within 1.2x. |
| Resident footprint | CI and local | Includes the Metal renderer when a device is present. |
| Pixel checksum of the long list | Local, `PERF_GATE_PIXELS=1` | Different GPUs antialias differently. Run before and after a renderer change. |

Cases use the long-list fixture (2000 rows of 28 px, 1440x900) with one row's selection changing per frame, and the same rows in a `virtual_list`. Metal work is not gated by instructions: driver threads make it move too much.

Budgets carry 25% headroom over the calibration in the file for instructions and 10% for allocations. After an intended cost change, rerun the gate, update the numbers and the `calibration` note in the same PR.

Correctness checks that guard the incremental paths run in `cargo test`: `tests/retained_layout.rs` compares retained and fresh layouts over randomly changing trees.

The head-to-head comparison with GPUI is local only: see `docs/gpui-comparison.md`.

## Runtime baselines

`runtime_baselines` is a stable, CI-friendly benchmark harness for the runtime paths that are most likely to regress user-facing responsiveness.

Run the full benchmark report with:

```sh
cargo bench --bench runtime_baselines
```

The command prints JSON with one result per benchmark. Each result includes `median`, `p95`, the committed baseline values, percent deltas, and a `status` field. Status values are `stable`, `warning`, `measurement_noise`, and `blocking`.

The committed baseline lives in `benchmarks/runtime_baselines.json`. It records the baseline revision, capture environment, warning threshold, blocking thresholds, and one baseline row for each benchmark category:

- `layout.flex_tree`
- `text.measure_raster`
- `text.multi_frame_layout`
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

## Long-list stress benchmark

`runtime_baselines` covers small, fixed workloads. `stress_frame` covers a long list:

```sh
cargo bench --bench stress_frame
STRESS_ROWS=500 cargo bench --bench stress_frame
```

It prints JSON with the cost of each frame stage on 2000 rows (a background and two texts each), by default: milliseconds and retired instructions for the whole process. Compare instructions across runs, since milliseconds move with machine load. `pixel_checksum` hashes one rendered frame, so a renderer optimisation should leave it unchanged. `footprint_mb` is the process's physical footprint after start, after the first layout and at the end. It reports only: it has no baseline and fails nothing yet.
