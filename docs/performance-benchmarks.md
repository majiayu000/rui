# Performance Benchmarks

## Performance gate (CI)

`perf_gate` fails when a frame costs more than `benchmarks/perf_budgets/` allows: one `<case>.json` per case and `shared.json` for the footprint, churn, damage, scaling and pixel checksum budgets and the calibration note. CI runs it after the tests:

```sh
cargo bench --bench perf_gate
```

| Check | Where | Why |
|---|---|---|
| Heap allocations per frame, by case | CI and local | Counted by the gate's own allocator; exact on any machine. |
| CPU instructions per frame (build, layout, paint), by case | Wherever the machine exposes the counter | Repeat within about 1% locally. GitHub's macOS runners do not expose the counter (seen on CI), and the report then says the instruction budgets were not checked; allocations, which match local runs exactly, still are. |
| `virtual_list` at 100 000 rows vs 1 000 | CI and local | By instructions, or by allocations without the counter. Must stay within 1.2x. |
| Resident footprint before the renderer | CI and local | RUI's own memory for the cases above; the same on any machine, so its budget is tight. |
| Resident footprint with the renderer | CI and local | Adds GPU driver memory, which depends on the GPU: GitHub's macOS VMs measure about 20 MB more than a local M2 Max, so this budget is looser. |
| Pixel checksum of the long list | Local, `PERF_GATE_PIXELS=1` | Different GPUs antialias differently. Run before and after a renderer change. |

Cases use the long-list fixture (2000 rows of 28 px, 1440x900) with one row's selection changing per frame, and the same rows in a `virtual_list`. Metal work is not gated by instructions: driver threads make it move too much.

Budgets carry 25% headroom over the calibration in the file for instructions and 10% for allocations. After an intended cost change, rerun the gate, update the numbers and the `calibration` note in the same PR.

Correctness checks that guard the incremental paths run in `cargo test`: `tests/retained_layout.rs` compares retained and fresh layouts over randomly changing trees.

The head-to-head comparison with GPUI is local only: see `docs/gpui-comparison.md`.

## CI execution and regression evidence

The `CI` workflow runs on pull requests, pushes to `main`, and manual dispatch.
New pushes to a PR cancel its superseded run; branch pushes do not duplicate PR
checks. The existing `rust` job keeps the MSRV, tests, Clippy and performance
gate together. Command output is retained for 14 days in a `ci-<sha>-<attempt>`
artifact, including on failure. Pipelines use `pipefail`, so saving a log cannot
turn a failing check green. Setup failures remain in the Actions step logs.

The separate `Replay verification` workflow runs daily at 19:23 UTC and can be
started manually. Reproduce it locally with:

```sh
RUI_VERIFY_REPLAY=1 cargo test --locked
```

For painted frames exercised by those tests, this compares replayed visible
primitives against a full repaint and reports the first difference. The
`replay-<sha>-<attempt>` artifact retains its output. This is not a pixel oracle
or an AppKit/IME/VoiceOver acceptance test. A new workflow becomes scheduled
and manually dispatchable after it lands on the default branch.

For each confirmed bug, add a focused test that fails before the fix and passes
after it. Reuse the existing headless harness and retained/fresh oracles rather
than introducing another test framework. Nightly replay failures need triage;
they are not a required PR check. Keep wall-clock and GPU timing informational
until equivalent workloads have been calibrated on a fixed machine.

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
