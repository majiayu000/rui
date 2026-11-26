# RUI vs GPUI: Same Fixtures, Same Machine

Both frameworks draw the same fixtures through Metal on one machine. Tracks #197.

> **GPU sampling (#277):** each side now runs one window with 10 warm-up frames
> and 60 sampled frames by default. GPUI collects this sequence separately from
> Criterion's CPU iterations. The 2026-11-16 results below used the same per-window
> frame range but included multiple GPUI windows, some of which contributed only
> part of that range. Use the fixed single-window run for new comparisons.

Run it with:

```sh
benchmarks/gpui_compare/run.sh [zed-checkout-dir]
```

The script builds both sides once, launches each binary once so macOS's first-launch
signature check stays out of the startup case, then runs every case in its own
process, alternating RUI/GPUI and GPUI/RUI order by round, `REPEATS` times (default 3).
It prints one table with the range over the repeats. After checkout preparation, each run creates a fresh
`target/gpui_compare/run.XXXXXXXX/` directory (printed on stderr), preserving
`results.txt`, full per-round stdout/stderr, build logs, execution order and commands.
`metadata.txt` records actual revisions, dirty status, parameters, both toolchains
and the host. The runner archives both resolved Cargo lockfiles, source patches and
the copied GPUI fixtures.
Untracked files other than those copied fixtures are listed, not archived. Use a
clean dedicated checkout when reproducing a run. The runner modifies the supplied
Zed checkout's manifest and bench files.
`CASES="quad_grid text_paragraphs"` picks cases. A last table gives a verdict per
case: RUI ahead, behind, or a tie when the two ranges overlap or the midpoints are
within 5% (instructions) or 10% (wall p50).

The runner's artifact handling can be checked without building Zed or using the GPU:
`python3 benchmarks/gpui_compare/test_runner.py`. This uses disposable fake tools;
it verifies paths, retained outputs, alternating order and failure propagation, not
benchmark validity.

- RUI: `benches/compare_frame.rs`, through `MetalRenderer::render_offscreen`.
- GPUI: `benchmarks/gpui_compare/fixtures.rs`, copied into a Zed checkout as
  `crates/gpui_platform/benches/rui_compare.rs`. Its benches are registered exactly as
  `#[gpui::bench(fps = 120)]` registers them, except that GPUI's headless Metal
  renderer is wrapped to count draw calls. It runs with
  `BENCH_MEASUREMENT=instructions` and is built with
  `--features bench-support,runtime_shaders,font-kit`: `runtime_shaders` because this
  machine has no Xcode `metal` tool, `font-kit` because without it `gpui_macos` uses
  `NoopTextSystem`, which shapes with fixed advances and rasterizes no glyphs. The
  bench fails if a text fixture draws no glyphs. `run.sh` also adds Zed's workspace
  `image` crate as a dev-dependency of `gpui_platform`, for the wrapper's signature.

> **Correction to #216.** The first version of this suite, and every GPUI number in
> the earlier long-list table below, was built without `font-kit`. GPUI's text cost
> nothing but layout with fake advances, and its scenes held no glyphs. With real
> text GPUI's wrapped-text frame rises from 13.6 M to 19.4 M instructions. Its list
> frames barely change (162 M to 160 M); its `.cached()` frame drops from 356 M to
> 262 M.

## Fixtures

Every case uses a 1440x900 window at scale 2 and 14 px text with line height 1.4 on
both sides. Each frame changes one thing, then rebuilds, lays out, paints and submits
to Metal without waiting for the GPU.

| Case | Content | Change per frame | RUI | GPUI |
|---|---|---|---|---|
| `list_full` | 2000 rows of 28 px, each a background, a title and a time | the selected row moves | `div` column | `div` column, root notified |
| `list_full_one_row` | same | one row toggles its selection | each row a view of its own entity (`cx.rows`), only that row notified | each row a view drawn with `.cached(..)`, only that row notified |
| `list_virtual_1k`, `list_virtual_100k` | same rows, 1000 or 100 000, scrolled to the middle | the selection moves within the visible rows | `virtual_list` | `uniform_list` |
| `text_paragraphs` | 200 wrapped paragraphs of 8 to 127 words, padding 16, gap 8 | paragraph 3 (on screen) gets a new revision suffix | `text(..).wrap()` | string children, which wrap by default |
| `quad_grid` | 200x60 cells of 7x15 px, one color each | one cell turns white | one `canvas`, `Painter::fill` | one `canvas`, `window.paint_quad(fill(..))` |
| `resize_full` | the 2000-row list | width alternates 1440 / 1280 | new viewport size | `window.resize` + `window.bounds_changed`, as the platform resize callback does |
| `resize_virtual` | the 1000-row virtual list | width alternates 1440 / 1280 | same | same |
| `startup` | the 2000-row list | none | see below | see below |
| `editor_scroll` | a 10 000-line code view: 20 px lines of 13 px SF Mono, a 64 px gutter of line numbers, 11 colored tokens per line (a comment every ninth line), the cursor's line highlighted; starts at line 5001 | scrolls down one line; the cursor moves with it | `virtual_list` of rows: gutter `text` + `shared_styled_text` of `span_range(..).color(..)` over the line's `Arc<str>` (the editor run below used `styled_text` of `span(..)`, a `String` per token) | `uniform_list` of rows: gutter string + `StyledText::with_highlights` |
| `editor_edit` | same, not scrolling | the cursor line's first identifier ends in a new letter | the line's tokens change and its `Arc<str>` and ranges are rebuilt | the line's tokens change and its `SharedString` and highlights are rebuilt |
| `terminal_rows` | a terminal screen: 45 rows of 20 px in 13 px SF Mono, each a frame counter or `static`, the row number and 60 hex characters | rows 0–19 get the new frame counter; rows 20–44 keep their text | one `canvas`, a `Primitive::Text` per row (`benches/compare_terminal`) | one `canvas`, `shape_line` + `ShapedLine::paint` per row (`benchmarks/gpui_compare/terminal.rs`) |
| `workbench_pane`, `workbench_full` | a 288 px virtual sidebar with 2,000 session rows, 8 spaces and 60 bindings, beside the same terminal screen | terminal ticks each turn. The full case also invalidates the sidebar and root | two entity-backed views with a `virtual_list` and terminal canvas | cached `uniform_list` view and terminal canvas. Full redraw notifies both views |

The workbench cases share their data and geometry in
`benchmarks/gpui_compare/workbench_data.rs`. Both sides check 45 visible sidebar
rows and 45 terminal rows, and ensure pane-only changes retain the sidebar. The
fixture measures a synthetic application frame. Fibo's real workbench also has
session subscriptions, live terminal processes, its own formatting and a host
process, which are outside this fixture.

The editor and terminal cases use the same font file on both sides,
`/System/Library/Fonts/SFNSMono.ttf`. RUI calls it "SF Mono"; Core Text hides it from
apps, so the GPUI fixture loads that file with `TextSystem::add_fonts` and uses the
family name inside it, ".SF NS Mono". It panics if GPUI resolves the family to
anything else. Both sides keep the document as tokens and build only the visible rows
each frame.

`canvas` is the idiomatic tool for a dense grid on both sides; `canvas_grid` in RUI's
examples and GPUI's `primitives` bench paint this way.

### What is measured

- Instructions / frame: the whole process's retired instructions. RUI: median of 60
  frames after 10 warm-up frames, from `proc_pid_rusage`. GPUI: Criterion's middle
  estimate per iteration (one update and one frame), the same counter.
- ms / frame p50: RUI: median wall time of the 60 frames, build to submit. GPUI: the
  bench report's median wall time per iteration, which includes the present.
- ms / frame p95: RUI: p95 of the same 60 frames. GPUI keeps no per-iteration p95, so
  this is the p95 of its `window draw` histogram, which **excludes the present**. GPUI's
  p95 is therefore a lower bound.
- Allocations / frame: heap allocations counted by a wrapping global allocator on both
  sides (RUI's bench installs one; `gpui::bench_main!` installs GPUI's).
- Draw calls and instances / frame: RUI: median of `MetalRenderer::last_frame_stats`
  over the 60 frames. GPUI: median over every scene the wrapped headless renderer
  received, counting one draw per `Scene::batches()` entry, which is what its Metal
  renderer encodes for the primitives these fixtures use (paths and surfaces would add
  draws; no fixture has them).
- GPU ms / frame, both sides, through one hook,
  `benchmarks/gpui_compare/gpu_time.rs`. RUI's bench includes it and `run.sh` copies
  it next to GPUI's.
  - GPUI's headless renderer commits its command buffer inside `render_scene` and
    keeps it. So the hook replaces `-commit` on the device's Metal command buffer
    class with one that adds a completed handler, then commits as before. Zed itself
    is unchanged.
  - Neither side waits for the GPU in the sampled frames. A frame's buffer can
    therefore start before the previous one ends, so each frame counts from the later
    of its `GPUStartTime` and the previous frame's `GPUEndTime`: the GPU busy time it
    added.
  - Both sides tag the same fixed sequence: frames 0–9 warm up and frames 10–69
    are sampled. `COMPARE_SAMPLES` sets the sample count for both. GPUI runs these
    updates in a dedicated window before Criterion's CPU loop. Its setup draws
    and Criterion's frames stay out of the sample. A missing or duplicate sampled
    frame fails the run. `tests/gpu_sample_window.rs` checks those rules without a
    GPU, and each side reports its actual sample count.
  - With `GPU_REDRAWS=N`, each sampled scene is drawn N more times. Both sides
    measure these redraws outside their CPU timings.
  - `startup` takes no GPU time on either side: each window draws one frame.
  - Before this, RUI's GPU column came from a separate pass that waited for every
    frame, and GPUI had none.
- Footprint: `ri_phys_footprint`. RUI: after its 70 frames. GPUI: after the first call
  of the bench function (Criterion calls it many times, each with a new window), and
  again at the end, after thousands of frames.
- Startup, on both sides from the same kernel timestamp (`ri_proc_start_abstime`):
  - process start to fixture code: RUI reaches `main`; GPUI reaches the bench
    function, after Criterion and `BenchAppContext` set up the app, platform and text
    system.
  - first frame: from before the renderer (RUI) or window (GPUI) exists to the first
    long-list frame submitted. GPUI's is read on its first update, after
    `bench_renderer` drew the pending frame; the bench asserts the list was rendered.
  - Neither side opens an on-screen window or waits for a presented frame. Startup of
    a real app window is not measured here.

## GPU time with the GPU kept busy (2026-11-16)

RUI `8df7ee7`, GPUI `784374c`, Apple M2 Max, `GPU_REDRAWS=16 REPEATS=3`, all cases.
Load average 16.8 / 10.5 / 6.4 before and 9.0 / 9.7 / 6.9 after.

Why: in the matched-sample run below, a frame of about 0.1 ms left the GPU idle most
of the time, and its time followed the clock the GPU picked rather than the work.
The same empty RUI frame (every sprite culled) took 0.059 ms in `editor_scroll`,
0.117 ms in `text_paragraphs` and 0.129 ms in `quad_grid`. Drawing 6271 glyphs
added only about 0.011 ms on top of it in `text_paragraphs`. That case's 1.8x
"behind" was the clock, not the glyph pass.

With `GPU_REDRAWS=N`, each side draws every frame's scene N more times right after
it, back to back. The GPU stays busy, so its clock is up. `gpu_time::steady_redraws`
reads the mean time of a frame's redraws, for the same frames 10 to 69 of every window.
On RUI the mean was the same with 16 and 64 redraws (0.045 to 0.058 ms across cases).

GPU saturated ms / frame p50, the range over the three runs:

| case | RUI | GPUI | GPUI frames | verdict (10% noise) |
|---|---|---|---|---|
| list_full | 0.044-0.052 | 0.162-0.172 | 126 | RUI ahead 3.5x |
| list_full_one_row | 0.041-0.044 | 2.955-17.339 | 1 | RUI ahead 239x |
| list_virtual_1k | 0.041-0.048 | 0.079-0.081 | 1124 | RUI ahead 1.8x |
| list_virtual_100k | 0.045-0.058 | 0.079-0.081 | 1124-1164 | RUI ahead 1.6x |
| text_paragraphs | 0.046-0.052 | 0.041-0.056 | 938-1072 | tie (ranges overlap) |
| quad_grid | 0.053-0.076 | 0.115-0.19 | 381 | RUI ahead 2.4x |
| resize_full | 0.043-0.05 | 0.108-0.141 | 126 | RUI ahead 2.7x |
| resize_virtual | 0.045-0.048 | 0.077-0.078 | 1124-1164 | RUI ahead 1.7x |
| editor_scroll | 0.048-0.051 | 0.081-0.083 | 1072-1124 | RUI ahead 1.7x |
| editor_edit | 0.049 | 0.081-0.083 | 1124-1164 | RUI ahead 1.7x |

Observations for this sampling run:

- With the clock taken out, RUI uses less GPU time than GPUI in every case but
  `text_paragraphs`, which is a tie. This includes the virtual-list and resize cases,
  by 1.6x to 2.7x.
- RUI's frames sit near 0.05 ms, which is about what clearing and storing the
  2880x1800 target costs. The draws themselves add little.
- `list_full_one_row`: GPUI's frames take about 4 ms of GPU each (3983 draw calls),
  so with the redraws only 1 frame per run reached the sample. The gap is large, but
  that number rests on very few frames.

Two limits of this run:

- The redraws run inside the frames Criterion times on the GPUI side, so this run's
  GPUI CPU columns are not valid. For CPU, use a run without `GPU_REDRAWS`.
- A frame's own buffer can overlap the redraws of the frame before it, so the plain
  GPU column reads 0 in some GPUI cases. Only the saturated column is read here.

## GPU time with matched samples (2026-11-16)

RUI `d016531`, GPUI `784374c`, Apple M2 Max, macOS 27.2.1, `REPEATS=3`, all cases.
The load average was 2.6 / 3.2 / 3.8 before the run and 8.5 / 6.5 / 5.1 after, on
12 cores (`mediaanalysisd` and a VM were running; the bench itself adds to the
end figure). A machine below 3 for five minutes did not come in three hours of
waiting, so this is the quietest run available, not an idle one.

GPU ms / frame p50, the range over the three runs, and the samples behind it:

| case | RUI | GPUI | GPUI frames (windows) | verdict by GPU p50 (10% noise) |
|---|---|---|---|---|
| list_full | 0.084-0.148 | 0.104-0.179 | 126 (16) | tie (ranges overlap) |
| list_full_one_row | 0.049-0.148 | 4.452-4.461 | 10 (2) | RUI ahead 45.2x |
| list_virtual_1k | 0.072-0.101 | 0.097 | 1497 (28) | tie (ranges overlap) |
| list_virtual_100k | 0.083-0.103 | 0.097 | 1494 (28) | tie (ranges overlap) |
| text_paragraphs | 0.129 | 0.07-0.072 | 1164 (26) | RUI behind 1.8x |
| quad_grid | 0.05-0.146 | 0.125-0.126 | 381 (21) | tie (ranges overlap) |
| resize_full | 0.049-0.147 | 0.103-0.174 | 126 (16) | tie (ranges overlap) |
| resize_virtual | 0.098-0.103 | 0.097-0.098 | 1391 (27) | tie (ranges overlap) |
| editor_scroll | 0.051-0.152 | 0.099 | 1364 (27) | tie (ranges overlap) |
| editor_edit | 0.072-0.153 | 0.099 | 1364 (27) | tie (ranges overlap) |

RUI samples 60 frames from one window in every case. At this scale both sides spend
about 0.1 ms, and RUI's p50 moved between about 0.05 and 0.15 ms from run to run with
the GPU's clock. This run read `text_paragraphs` as RUI behind 1.8x (0.129 ms against
0.07 ms). [GPU time with the GPU kept busy](#gpu-time-with-the-gpu-kept-busy-2026-11-16)
shows a tie in that setup, consistent with a clock effect. A first run the same day
at load 14 to 19 moved GPUI's numbers in the same way.

The same run's CPU verdicts match the earlier ones: RUI ahead in every case, 2.6x to
133x by wall p50, and 8.1x from process start to first frame.

## One changed row with rows as views

`list_full_one_row` used to run the same rebuilt-from-the-root frame as
`list_full` on RUI's side. It now matches GPUI's: each of the 2000 rows is a view
of its own entity, written with `cx.rows`, and each frame toggles one row's
selection and notifies only that row. Three runs, load average 7–47:

| Side | Instructions (M) / frame | ms p50 | Allocations / frame | Draw calls |
|---|---|---|---|---|
| RUI | 2.38 | 0.15–0.22 | 20 | 1 |
| GPUI `.cached()` (final results below) | 252.9–253.1 | 24.3–24.5 | 6 092 | 3 983 |

RUI is 106x ahead by instructions and allocates 300x less. It was the last
fixture where RUI allocated more than GPUI.

## Shared text and identity-kept layout (RUI `8099b1f`)

RUI side only, from `benches/compare_frame.rs`, against the GPUI numbers of the
runs below (same GPUI build). The machine was under load average 100–185 from
other builds, so only instructions and allocations are quoted; between rounds
they moved by under 1%. Two changes since `27a9be7`:

- `Primitive::Text` holds a `SharedStr`, a range of an `Arc<str>`, and an
  `Arc<str>` family. Text, styled-text pieces and wrapped lines paint slices of
  the text they own instead of a new `String` per piece. Text made from a
  `String` keeps it until it is first laid out as wrapped text or painted, so
  rows that are never painted cost no copy.
- A retained layout pass hands each element the nodes of the element with the
  same identity in the previous pass, not the nodes at the same position. It
  looks at the next node in order first and builds an identity map only when
  that is someone else's, so an unchanged tree hashes nothing. A scrolled list's
  rows keep their nodes and taffy's cached layout; before, each row slot took a
  new line's text and every row was laid out again.

| Case | RUI instructions (M), before → now | GPUI | RUI allocations, before → now | GPUI |
|---|---|---|---|---|
| editor_scroll | 5.72 → 2.83 | 8.48–8.76 | 2 819 → 902 | 1 636 |
| editor_edit | 2.64 → 2.12 | 8.49–8.68 | 1 667 → 506 | 1 636 |
| text_paragraphs | 10.37 → 10.18 | 19.31 | 1 553 → 1 143 | 1 842 |
| list_full | 24.04 → 24.17 | 159.5 | 16 015 → 14 015 | 50 046 |
| list_full_one_row | 24.05 → 24.17 | 252.9 | 16 015 → 14 015 | 6 092 |
| list_virtual_1k | 0.94 → 0.93 | 3.92 | 299 → 262 | 787 |
| resize_full | 47.36 → 46.92 | 164.3 | 28 026 → 26 026 | 50 045 |
| resize_virtual | 1.37 → 1.37 | 9.03 | 532 → 495 | 788 |
| quad_grid | 5.30 → 5.35 | 79.97 | 16 → 16 | 14 |

- **Editor allocations are now below GPUI's**: 902 against 1 636 when scrolling,
  506 against 1 636 when editing. Scrolling costs a third of GPUI's instructions.
- **Cost:** `perf_gate`'s `list.one_row_changed_rows` and `_views`, which keep
  2000 row subtrees a frame, rise from 2.0 M to 2.5 M instructions: each kept
  subtree's nodes are now copied into the new pass's list. Both stay in budget.
- **Still behind on allocations:** `quad_grid`, by 2. `list_full_one_row` drew the
  rebuilt list here; with rows as views (above) it allocates 20 against GPUI's
  6 092.

## Run on `29544b1` (2026-11-26, archived)

`benchmarks/gpui_compare/run.sh` with every case, three rounds, GPUI at Zed `784374c`.
The raw output, metadata, run order and commands are in
`benchmarks/gpui_compare/results/2026-11-26-29544b1/`. Apple M2 Max, macOS 27.2.1. The
machine was busy (load average 99 before, 214 after), so instructions and allocations
are the numbers to compare; wall and GPU time moved with the load. It is the first
archived run after #340, so RUI draws text at the same em as GPUI here, unlike the
runs below.

| Case | RUI instructions (M) | GPUI instructions (M) | GPUI / RUI | RUI allocations | GPUI allocations | RUI footprint (MB) | GPUI footprint (MB) |
|---|---|---|---|---|---|---|---|
| list_full | 16.96–16.97 | 156.8–158.5 | 9.3x | 4 077 | 50 046–50 053 | 57–60 | 88–95 |
| list_full_one_row | 2.57–2.58 | 248.2–263.1 | 99x | 14 | 6 092–6 141 | 62–63 | 199–207 |
| list_virtual_1k | 0.76–0.77 | 3.89–3.97 | 5.2x | 151 | 787–788 | 48 | 76–85 |
| list_virtual_100k | 0.62–0.77 | 3.97–4.09 | 5.6x | 151 | 787–788 | 48 | 79–96 |
| text_paragraphs | 3.78–3.80 | 19.70–20.04 | 5.2x | 898 | 1 842–1 845 | 54–55 | 72–76 |
| quad_grid | 5.92–5.97 | 80.01–81.86 | 13.6x | 16 | 14–26 | 57–58 | 75–79 |
| resize_full | 39.87 | 161.5–163.3 | 4.1x | 16 088 | 50 045–50 063 | 58–62 | 88–93 |
| resize_virtual | 1.23–1.24 | 9.39–9.46 | 7.6x | 384 | 788–789 | 48 | 79–143 |
| editor_scroll | 2.50–2.51 | 8.76–9.05 | 3.6x | 677 | 1 636–1 638 | 59–60 | 66–89 |
| editor_edit | 2.19–2.21 | 8.79–9.13 | 4.1x | 354 | 1 636–1 640 | 59–60 | 68–75 |
| terminal_rows | 3.16 | 7.25–7.72 | 2.4x | 885 | 357–359 | 55 | 63–80 |
| workbench_pane | 3.42–3.44 | 8.30–8.56 | 2.5x | 893 | 367–369 | 55 | 65–85 |
| workbench_full | 3.76–3.78 | 10.49–10.66 | 2.8x | 1 082 | 894–895 | 55 | 56–87 |

Startup, process start to the first frame: RUI 20–35 ms, GPUI 269–436 ms.

- **The workbench figures #179 closed on reproduce:** GPUI 8.30–8.56 M and 10.49–10.66 M
  against the 8.27 M and 10.34 M recorded there, and RUI 3.42 M and 3.77 M as before.
- **GPUI allocates less on `terminal_rows` (2.5x) and the workbench cases (1.2–2.4x).**
  On the editor cases RUI now allocates less.
- **GPU time is level or RUI ahead in every case** in this run; `text_paragraphs`, which
  the matched-sample run below had RUI behind on, is a tie.

## Final results (RUI `c2e3596`)

Apple M2 Max (12 cores, 96 GB), macOS 26.5.2. RUI `c2e3596`: every lane of #179
merged (retained layout, default subtree replay, view-level rebuild, damage regions,
one sprite shader, analytic paths, text caches, font fallback, rich text, color emoji,
off-thread renderer creation, display pacing, multi-window, spawn). GPUI at Zed
`784374c` with `runtime_shaders` and `font-kit`, as above. Three rounds, each case in a
fresh process; cells are the range.

The machine was nearly idle: load average 7.1 at the start and 10.5 at the end (one
unrelated Python process was using about one core). Earlier runs were at 80–225.

| Case | Side | Instructions (M) / frame | ms p50 | ms p95 | Allocations / frame | Draw calls | Footprint (MB) |
|---|---|---|---|---|---|---|---|
| list_full | RUI | 25.13–25.17 | 1.64–1.87 | 2.04–12.06* | 16 015 | 1 | 67–70 |
| list_full | GPUI | 159.5–159.9 | 13.1–13.6 | 14.0–14.5 | 50 046 | 2 | 78–79 |
| list_full_one_row | RUI | 25.18–25.19 | 1.63–1.68 | 2.00–2.09 | 16 015 | 1 | 67 |
| list_full_one_row (GPUI `.cached()`) | GPUI | 252.9–253.1 | 24.3–24.5 | 26.0–26.4 | 6 092 | 3 983 | 190–191 |
| list_virtual_1k | RUI | 0.93 | 0.22 | 0.46–0.48 | 299 | 1 | 56 |
| list_virtual_1k | GPUI | 3.92 | 0.31 | 0.32–0.33 | 787 | 2 | 92 |
| list_virtual_100k | RUI | 0.94 | 0.22 | 0.48 | 299 | 1 | 57 |
| list_virtual_100k | GPUI | 3.98 | 0.31–0.32 | 0.33 | 787 | 2 | 92 |
| text_paragraphs | RUI | 10.47 | 0.74–0.76 | 0.84–0.92 | 1 553 | 1 | 63–64 |
| text_paragraphs | GPUI | 19.31–19.32 | 1.39–1.41 | 1.49–1.53 | 1 842 | 1 | 68–69 |
| quad_grid | RUI | 5.58–5.59 | 0.33–0.35 | 0.68–0.69 | 16 | 1 | 64–66 |
| quad_grid | GPUI | 79.97–79.98 | 6.22–6.31 | 6.34–6.55 | 14 | 1 | 74–76 |
| resize_full | RUI | 48.9–49.2 | 3.03–3.10 | 3.43–3.50 | 28 026 | 1 | 65–67 |
| resize_full | GPUI | 164.3–164.4 | 13.1–13.4 | 13.9–14.4 | 50 045 | 2 | 78–79 |
| resize_virtual | RUI | 1.38–1.39 | 0.21–0.23 | 0.48 | 532 | 1 | 57 |
| resize_virtual | GPUI | 9.03–9.09 | 0.52–0.57 | 0.37–0.54 | 788 | 2 | 92 |

\* One round's p95; the other two were 2.04 and 2.08 ms.

| Startup | Process start to fixture code | First frame | Process start to first frame | Footprint |
|---|---|---|---|---|
| RUI | 5.1–5.8 ms | 46.7–49.2 ms | 52.5–54.3 ms | 24–26 MB |
| GPUI | 68.7–77.5 ms | 113.9–115.7 ms | 182.8–191.4 ms | 76–102 MB |

Both startups are warm: the system Metal shader cache already held each side's
shaders. RUI now embeds its shaders precompiled, so a cold start no longer compiles
them either: first frame p50 69 ms against 207 ms before (#177,
`docs/performance-architecture.md`). GPUI ships a metallib by default but is built
here with `runtime_shaders`.

| Case | By instructions | By wall p50 |
|---|---|---|
| startup | – | RUI ahead 3.5x |
| list_full | RUI ahead 6.4x | RUI ahead 7.6x |
| list_full_one_row | RUI ahead 10.0x | RUI ahead 14.7x |
| list_virtual_1k | RUI ahead 4.2x | RUI ahead 1.4x |
| list_virtual_100k | RUI ahead 4.2x | RUI ahead 1.5x |
| text_paragraphs | RUI ahead 1.8x | RUI ahead 1.9x |
| quad_grid | RUI ahead 14.3x | RUI ahead 18.4x |
| resize_full | RUI ahead 3.3x | RUI ahead 4.3x |
| resize_virtual | RUI ahead 6.5x | RUI ahead 2.4x |

### Verdict

On every fixture in this suite RUI is ahead of GPUI by instructions and by wall time,
and its footprint is lower in every case. That is the claim, and its limits:

- **Allocations are not uniformly lower.** On `list_full_one_row` RUI allocates
  16 015 per frame against 6 092 for GPUI's `.cached()` path, because this fixture
  rebuilds the list from the root. With rows as views (#234), the same edit costs
  23 allocations and 2.0 M instructions (perf_gate `list.one_row_changed_views`), but
  that path is not in this fixture. On `quad_grid` the two are equal (16 vs 14).
- **GPU time** was reported for RUI only in this run. Both sides are now measured; see
  [historical GPU measurements](#historical-gpu-time-on-both-sides-rui-a5cae0b).
  Their unequal sampling populations do not support a cross-framework GPU verdict.
- **One fixture set, one machine, one GPUI revision.** It says nothing about
  Linux, Windows or the web, where GPUI runs and RUI does not.
- **Fonts differ:** RUI's default is Arial, GPUI's SF Pro.


## Historical GPU time on both sides (RUI `a5cae0b`)

These measurements predate #277: GPUI was sampled across Criterion warm-up and every window's first frames. They remain
historical observations; do not use them as a current regression gate or a causal
explanation. The runner emits no automatic GPU winner/loser verdict.

All eleven cases, three rounds, with the hook above. Load average 26 at the start and
43 at the end. The CPU columns of this run agree with the final results and the editor
run: RUI is ahead on every case by instructions (1.1x to 15.5x). Its startup is ahead
3.8x. Only GPU time is new here.

| Case | RUI GPU ms p50 | GPUI GPU ms p50 | RUI p95 | GPUI p95 |
|---|---|---|---|---|
| list_full | 0.28–0.51 | 0.31 | 0.37–0.64 | 0.32–0.42 |
| list_full_one_row | 0.51 | 6.43–6.90 | 0.51–0.64 | 11.3–12.5 |
| list_virtual_1k | 0.22–0.34 | 0.097 | 0.35–0.47 | 0.10–0.13 |
| list_virtual_100k | 0.22 | 0.097 | 0.26–0.51 | 0.10–0.13 |
| text_paragraphs | 0.13–0.27 | 0.20 | 0.16–0.44 | 0.20–0.91 |
| quad_grid | 0.31 | 0.38 | 0.67–0.75 | 0.44–0.76 |
| resize_full | 0.51 | 0.28 | 0.51–0.89 | 0.32–0.52 |
| resize_virtual | 0.24–0.47 | 0.22–0.28 | 0.34–1.11 | 0.30–0.51 |
| editor_scroll | 0.24–0.25 | 0.30 | 0.29–0.52 | 0.31–0.60 |
| editor_edit | 0.24–0.26 | 0.30 | 0.48–0.55 | 0.31–0.64 |

Recorded observations and limits:

- GPUI's `.cached()` fixture recorded 3 983 draws per frame. The virtual-list and
  resized-list instance counts were 1 056 vs 1 217 and 1 033 vs 1 683, respectively.
  These counts do not establish what caused the recorded GPU durations.
- The historical RUI virtual-list trace recorded overlapping buffers ending about
  0.22 ms apart, with CPU frames of 0.21–0.33 ms wall time and waits at the three-frame
  in-flight limit. Its earlier isolated-frame pass recorded 0.16 ms. These are
  observations of different submission conditions, not a controlled comparison with
  GPUI's durations.
- RUI's `list_full` p50 varied between rounds from 0.28 to 0.51 ms. This run did not
  isolate GPU clock state, system contention or other causes of the variation.
- Sampling populations and submission conditions must be matched before assigning
  GPU winners or investigating a cross-framework cause. The CPU instruction and
  startup observations above are separate metrics; they do not resolve this GPU gap.

## Terminal rows (RUI `a646536`)

`terminal_rows` is the load of a terminal pane that a program keeps printing over:
Fibo's terminal draws its screen as one canvas, a row at a time. Same machine and
GPUI revision as above, three rounds, each case in a fresh process, load average 8
to 30. `editor_edit` ran alongside as a control. GPU columns use the matched samples
described above (frames 10 to 69 of every window) with `GPU_REDRAWS=0`; how many
frames and windows each side sampled was not recorded.

| Case | Side | Instructions (M) / frame | ms p50 | ms p95 | GPU ms p50 | Allocations / frame | Draw calls | Instances | Footprint (MB) |
|---|---|---|---|---|---|---|---|---|---|
| terminal_rows | RUI | 4.15–4.16 | 0.26–0.27 | 0.30–0.34 | 0.085–0.151 | 865 | 1 | 3 236 | 55 |
| terminal_rows | GPUI | 7.25–7.26 | 0.51–0.52 | 0.57–0.58 | 0.132–0.237 | 358 | 2 | 3 421 | 60–91 |
| editor_edit | RUI | 2.15 | 0.17 | 0.20 | 0.078–0.153 | 354 | 2 | 1 466 | 59 |
| editor_edit | GPUI | 8.77 | 0.64–0.65 | 0.63–0.66 | 0.169–0.302 | 1 636 | 2 | 1 908 | 81 |

| Case | By instructions | By wall p50 |
|---|---|---|
| terminal_rows | RUI ahead 1.7x | RUI ahead 2.0x |
| editor_edit | RUI ahead 4.1x | RUI ahead 3.8x |

- **No GPU verdict.** At about 0.1 ms a frame leaves the GPU idle and its time
  follows the GPU's clock, as
  [GPU time with matched samples](#gpu-time-with-matched-samples-2026-11-16) found;
  only a run with the GPU kept busy gives a GPU verdict, and this one did not keep
  it busy. The plain p50 ranges overlap for `terminal_rows`. For `editor_edit` they
  read RUI ahead 2.0x here, a tie in the matched-sample table and RUI ahead 1.7x
  with the GPU kept busy, which is the verdict to use.
- **GPUI's instruction count rose** from 6.60 to 7.25 M per terminal frame (and 8.5
  to 8.8 M in `editor_edit`). The 6.60 M is from the first run of this fixture, on
  `3d1f031` (based on `516b8c0`, before the GPU sampling rewrite #337); this run is
  `6d2dfc6`. RUI's count did not move. The cause is not known. It is not the
  `counting_renderer` wrapper: that already walked every scene before #337, and #337
  added only a few stores per frame to it, tens of instructions against 650 000.
- **RUI allocates 2.4x more** per terminal frame (865 against 358). Both sides format
  the same 45 strings each frame; where RUI's extra ones go was not measured.
- **Not the best a terminal can do on either side.** Both fixtures shape each row as
  a string, so every changed row is shaped again. Fibo's GPUI painter keeps each
  ASCII glyph once and paints it per cell with `window.paint_glyph`, which skips
  shaping. RUI's `Painter` has no per-glyph call yet (#176), so that path has no RUI
  equivalent to compare.
- **Glyphs were smaller on RUI at the same number in this run.** RUI then took
  `font_size` as the ascent-to-descent height (#327), so 13 px SF Mono was about an
  11 px em in RUI and a 13 px em in GPUI, about 1/1.39 of GPUI's glyph area. Since
  #340 `font_size` is the em, as in GPUI. Run alone on RUI before and after #340,
  `terminal_rows` read 4.17 and 4.21 M instructions with the same 865 allocations
  and 3 236 instances, so the CPU columns hold; the GPU column was measured with the
  smaller glyphs.

## Editor-scale fixture (RUI `a31e116`)

Same machine and GPUI revision as the final results above. Three rounds, each case in a
fresh process. Load average 41 at the start and 56 at the end (other sessions were
building), so trust instructions and allocations over milliseconds here.
`list_virtual_1k` ran alongside as a control and matched the final results (0.94 M vs
3.92–4.02 M).

| Case | Side | Instructions (M) / frame | ms p50 | ms p95 | Allocations / frame | Draw calls | Instances | Footprint (MB) |
|---|---|---|---|---|---|---|---|---|
| editor_scroll | RUI | 7.79–7.80 | 0.50–0.52 | 0.52–0.60 | 4 456 | 1 | 1 470 | 62–64 |
| editor_scroll | GPUI | 8.47–8.55 | 0.66–0.77 | 0.71–1.87 | 1 636 | 2 | 1 918 | 82–84 |
| editor_edit | RUI | 3.57–3.60 | 0.24–0.66 | 0.35–1.00 | 3 283 | 1 | 1 464 | 63–64 |
| editor_edit | GPUI | 8.51–8.66 | 0.65–1.57 | 0.69–4.05 | 1 636 | 2 | 1 908 | 83–113 |

RUI GPU time: 0.24–0.29 ms (scroll) and 0.17–0.52 ms (edit) per frame.

| Case | By instructions | By wall p50 |
|---|---|---|
| editor_scroll | RUI ahead 1.1x | RUI ahead 1.4x |
| editor_edit | RUI ahead 2.4x | tie (ranges overlap) |

### Verdict

- **Editing** a visible line costs RUI 2.4x fewer instructions than GPUI.
- **Scrolling** is a near tie by instructions (RUI 8% ahead). That is the smallest
  margin in the suite; every other case is 1.8x or more.
- **Allocations are behind on both cases**: 2.7x on scroll, 2.0x on edit.
  `span(..)` takes an owned `String`, so each visible row allocates a string per token
  (11 per token-run row, 49 rows built). GPUI clones one reference-counted `SharedString`
  and a highlight `Vec` per row.
- **Footprint** is lower on RUI in both cases (62–64 MB vs 82–113 MB).

Why scrolling costs RUI twice what an edit does: in a sample of `editor_scroll`, about
30% of the main thread hashes `RichSpan`s with SipHash. The callers are
`TextMeasureCache::wrapped_rich_size` and `wrap_rich`, which key the styled-text cache
on every span. Another ~4% is `FrameLru::evict`, which sorts the whole measure cache
each frame once new lines keep arriving. Neither is fixed here.

### After the text follow-ups (#260 and shared spans)

This run is on main plus #253, #256, #260 and the shared-spans change, with the editor
fixture's RUI side using `shared_styled_text`. Three rounds, load average 33–35.

| Case | Side | Instructions (M) / frame | Allocations / frame | Verdict by instructions |
|---|---|---|---|---|
| editor_scroll | RUI | 5.62–5.64 (was 7.79–7.80) | 2 819 (was 4 456) | RUI ahead 1.5x (was 1.1x) |
| editor_scroll | GPUI | 8.48–8.76 | 1 636–1 638 | |
| editor_edit | RUI | 2.60–2.67 (was 3.57–3.60) | 1 667 (was 3 283) | RUI ahead 3.3x (was 2.4x) |
| editor_edit | GPUI | 8.49–8.68 | 1 636 | |

- **Edit:** allocations are now level with GPUI's (1.02x).
- **Scroll:** still 1.7x GPUI's allocations. Each `Primitive::Text` the renderer
  receives owns its piece's text and family name, which is renderer work left to lane G.
  Measuring the line that scrolls in also allocates.

## Run on `ec7660d` (before the remaining lanes)

Apple M2 Max (12 cores, 96 GB), macOS 26.5.2. RUI `ec7660d` (main, with the text caches
of #218 and display pacing of #215), rustc 1.99.1. GPUI at Zed `784374c`, rustc 1.98.1
(Zed's toolchain file), with `runtime_shaders` and `font-kit`. Three rounds, each case
in a fresh process; cells are the range.

Other sessions were compiling on the machine; the load average rose from 93 to 111
during the run. **Wall times are noisy on both sides; instructions and allocations
moved by under 3% between rounds (RUI's virtual lists up to 40% in instructions,
see below) and are the numbers to compare.**

| Case | Side | Instructions (M) / frame | ms / frame p50 | ms / frame p95 | Allocations / frame | Draw calls | Instances | GPU ms p50 | Footprint (MB) | GPUI footprint at end (MB) |
|---|---|---|---|---|---|---|---|---|---|---|
| `list_full` | RUI | 22.7–22.8 | 3.5–4.7 | 4.7–19 | 16 014 | 1 | 1 033 | 0.38–0.65 | 58–61 | |
| | GPUI | 161.9–162.4 | 30–41 | 35–69 | 50 060 | 2 | 1 683 | | 78–95 | 258–266 |
| `list_full_one_row` | RUI | 22.7–22.8 | 2.7–4.9 | 6.9–21 | 16 014 | 1 | 1 033 | 0.19–0.42 | 58–64 | |
| | GPUI `.cached()` | 267–273 | 40–68 | 66–120 | 6 114–6 141 | **3 983** | 75 972 | | 160–191 | 223–505 |
| `list_virtual_1k` | RUI | 1.3–1.9 | 0.19–0.27 | 0.4–1.1 | 298 | 1 | 1 056 | 0.18–0.63 | **99–141** | |
| | GPUI | 4.0 | 0.48–0.90 | 1.1–1.5 | 788 | 2 | 1 217 | | 62–91 | 122–129 |
| `list_virtual_100k` | RUI | 1.6–1.9 | 0.11–0.36 | 0.2–1.1 | 298 | 1 | 1 122 | 0.19–0.50 | **121–164** | |
| | GPUI | 4.0–4.1 | 0.56–0.88 | 1.1–1.5 | 788 | 2 | 1 281 | | 61–71 | 122–148 |
| `text_paragraphs` | RUI | 16.0–16.1 | 1.2–2.2 | 3.3–4.1 | **8 558** | 1 | 6 271 | 0.28–0.39 | 57–59 | |
| | GPUI | 19.5–19.6 | 2.5–3.6 | 4.4–7.0 | 1 843 | 1 | 6 375 | | 67–71 | 90–97 |
| `quad_grid` | RUI | 5.1 | 0.58–0.69 | 0.7–1.2 | 15 | 1 | 12 000 | 0.49–0.76 | 74–110 | |
| | GPUI | 80.5–80.6 | 15–23 | 18–38 | 17–18 | 1 | 12 000 | | 75–76 | 94–99 |
| `resize_full` | RUI | 53.0 | 7.4–8.6 | 10–35 | 28 025 | 1 | 1 033 | 0.31–0.55 | 64–74 | |
| | GPUI | 165.5–166.8 | 32–47 | 39–84 | 50 065 | 2 | 1 683 | | 75–96 | 254–266 |
| `resize_virtual` | RUI | 2.6 | 0.54–0.60 | 1.5–8.7 | 531 | 1 | 1 056 | 0.34–0.64 | **158–224** | |
| | GPUI | 8.7–9.0 | 0.98–1.25 | 1.7–2.6 | 788 | 2 | 1 217 | | 70–92 | 274–329 |

RUI's GPU p95 ranged up to 4–7 ms in every case in at least one round, against p50s
under 0.8 ms: the GPU was shared with the other sessions' work, so only the p50 is
quoted.

| Startup | Process start to fixture code (ms) | First frame (ms) | Process start to first frame (ms) | Footprint (MB) |
|---|---|---|---|---|
| RUI | 11–14 | 188–247 | 202–259 | 22 |
| GPUI | 159–174 | 253–257 | 411–431 | 101–102 |

Verdict, as `run.sh` prints it:

| Case | By instructions | By wall p50 |
|---|---|---|
| `startup` | | RUI ahead 1.8x (process start to first frame) |
| `list_full` | RUI ahead 7.1x | RUI ahead 8.7x |
| `list_full_one_row` | RUI ahead 11.9x | RUI ahead 14.2x |
| `list_virtual_1k` | RUI ahead 2.5x | RUI ahead 3.0x |
| `list_virtual_100k` | RUI ahead 2.3x | RUI ahead 3.1x |
| `text_paragraphs` | RUI ahead 1.2x | RUI ahead 1.8x |
| `quad_grid` | RUI ahead 15.9x | RUI ahead 29.7x |
| `resize_full` | RUI ahead 3.1x | RUI ahead 5.0x |
| `resize_virtual` | RUI ahead 3.4x | RUI ahead 2.0x |

No case is behind by instructions or by wall p50. RUI is still behind GPUI on
allocations in wrapped text and on footprint in the virtual-list cases (below).

### Reading that run

What changed since the last run (RUI `f7968cc`, same GPUI build):

- Wrapped text: 286 M to 16 M instructions and 163 821 to 8 558 allocations per frame,
  from #218's text caches. It was 14.8 times behind GPUI by instructions; it is now
  1.2 times ahead, and 1.8 times ahead in wall p50.
- Startup: process start to first frame from 1.3–1.6 s to 0.20–0.26 s. It was 6.4 times
  behind; it is now 1.8 times ahead. GPUI's own startup read slower in this run
  (411–431 ms against 212–245 ms before, same build): its first frame took 253–257 ms
  against 120–134 ms, which is load. Against GPUI's earlier 212–245 ms the two overlap
  (RUI 202–259 ms), so the startup lead depends on load; recheck it on an idle machine.
- Long list: 25.7 M to 22.7 M instructions and 24 014 to 16 014 allocations.

Where RUI is ahead:

- Long list, full redraw: 7.1 times fewer instructions than GPUI and a third of the
  allocations. Retained layout and renderer culling mean the offscreen rows cost
  build, layout and paint but no Metal work: RUI submits 1 033 instances, GPUI 1 683.
- One changed row: GPUI's `.cached()` path costs 1.7 times its own full redraw at 2000
  rows and is 11.9 times RUI's frame. Replaying cached views breaks GPUI's batching:
  3 983 draw calls and 75 972 instances per frame. GPUI still allocates 2.6 times less
  than RUI on that path.
- Virtual lists: 2.3 to 2.5 times fewer instructions and 2.6 times fewer allocations
  than `uniform_list`, flat from 1000 to 100 000 rows on both sides. RUI's instruction
  count varies by up to 40% between rounds here (1.3 to 1.9 M); the slowest round is
  still 2 times ahead.
- Quad grid: 16 times fewer instructions for the same 12 000 quads in one draw.
  GPUI's side was not profiled.
- Resize: 3.1 to 3.4 times fewer instructions.
- Text, by a small margin: 1.2 times fewer instructions, with about the same glyphs
  submitted (6 271 vs 6 375 instances).
- Draw calls: RUI draws every fixture in 1 call; GPUI uses 2 (quads, then glyph
  sprites) except the text-only and quad-only fixtures.

Where RUI is still behind:

- **Allocations in wrapped text: 8 558 against GPUI's 1 843 per frame, 4.6 times
  more.** A per-stage split of RUI's frame (a local, uncommitted counter in the bench)
  puts about 800 in building the 200 paragraphs, 5 864 in layout, 416 in paint and
  1 476 in the renderer.
- Where the text frame's time goes, from a 3 s `sample` of the steady frame:
  - Layout is about 60% of the frame. Most of it is `TextMeasureCache::wrap_uncached`
    (about 40% of the frame), reached through `TextWrapRequest::wrapped_size` from
    Taffy's measure callback. Only the edited paragraph should miss the cache, so
    each frame's misses look like that paragraph measured under Taffy's width probes.
    That was not confirmed by counting.
  - The renderer is about 36%: building glyph instances in `TextGlyphCache::glyph` and
    `TextGlyphCache::shape`, with SipHash hashing of `GlyphKey` (`RandomState`) at
    about a fifth of the frame.
- **Footprint in the virtual-list cases:**
  - Virtual lists: 99–164 MB against GPUI's 61–91 MB after its first call.
  - Resize on the virtual list: 158–224 MB against 70–92 MB. RUI's offscreen path
    still reallocates its target texture on every size change.
  - GPUI's footprint grows over thousands of frames in one process (122–329 MB at
    the end), so this compares RUI after 70 frames with GPUI after its first call.
- Startup footprint is RUI's clear memory win (22 MB vs 101 MB).

## Known unfairness

- Load: the final results were taken at load average 7–10; the earlier runs below
  them at 93–111 with other builds running. Quote milliseconds from the final run.
- GPUI runs inside its test platform: each iteration also pumps the foreground
  executor and the frame profiler is enabled. Its instructions include that harness
  work; RUI's bench calls the pipeline directly.
- GPUI's p95 is draw-only (no present); RUI's p95 includes submit. GPUI's p95 can
  therefore read lower than its p50.
- GPUI's per-iteration statistics include Criterion warm-up; Criterion's instruction
  estimate does not.
- GPUI's draw calls are counted from scene batches, not from the encoder.
- Footprint is read at different points (RUI after 70 frames, GPUI after its first
  bench call and at the end).
- RUI's bench drains an autorelease pool per frame, as RUI's native runner does per
  event; without one, Metal's autoreleased objects grew RUI's footprint past 1.4 GB in
  the resize cases.
- `virtual_list` builds 2 rows beyond each edge; `uniform_list` builds only the visible
  rows. In the editor cases that is 49 rows against 45.
- Runs before "One changed row with rows as views" drew `list_full_one_row` on RUI
  as the same frame as `list_full`, rebuilt from the root.
- Resize on GPUI calls what its resize callback calls (`window.resize`,
  `window.bounds_changed`) from inside a frame update, since `simulate_resize` cannot be
  called there. Neither side resizes a real `CAMetalLayer`.
- Fonts differ: with no family set, RUI uses the first face it loads (Arial), GPUI
  `.SystemUIFont` (SF Pro) through Core Text. Both rasterize glyphs at device scale into
  an atlas with subpixel variants.
- Font size: every run recorded here was taken before #340, when RUI took
  `font_size` as the ascent-to-descent height, so RUI drew a smaller em than GPUI at
  the same number: 1/1.12 in Arial, 1/1.18 in the editor and terminal cases' SF Mono.
  Since #340 both sides take `font_size` as the em.
- Toolchains differ (rustc 1.99.1 for RUI, 1.98.1 for Zed) and both build with their own
  release profile.

## Measurement hook in RUI

The comparison reads GPU time through `benchmarks/gpui_compare/gpu_time.rs` on both
sides and needs nothing from `src/`. `MetalRenderer::render_offscreen_gpu_time`, which
waits for a frame and returns its `GPUEndTime - GPUStartTime`, is still used by
`perf_gate` and `renderer_frame` for isolated single-frame GPU time.

## Not measured yet

- Startup to a presented frame in a real on-screen window.
- Input latency against GPUI. RUI's own is measured natively since #244
  (`frame_pacing` latency mode: 24–25 ms event to photon at 120 Hz).

## Previous run (before #218)

RUI `f7968cc`, same GPUI build with `font-kit`, load 80–120. Instructions in the other
cases moved by under 15% since:

| Case | RUI | GPUI | Verdict then |
|---|---|---|---|
| `text_paragraphs` instructions / frame | 286–287 M | 19.3–19.4 M | RUI behind 14.8x |
| `text_paragraphs` allocations / frame | 163 821 | 1 842 | |
| Process start to first frame | 1 340–1 569 ms | 212–245 ms | RUI behind 6.4x |
| `list_full` instructions / frame | 25.7 M | 159.5–159.7 M | RUI ahead 6.2x |

## Earlier long-list results

Same machine, before this suite, GPUI at the same revision with its default 16 px text
(the rows now use 14 px on both sides) and **without `font-kit`, so GPUI drew no
glyphs**:

| Case | Instructions / frame | Wall time / frame | Footprint |
|---|---|---|---|
| RUI main `5690b8f`, full redraw | 873 M | 63.2 ms | 1150 MB |
| RUI with PR #181 | 91 M | 4.6 ms | 1134 MB |
| RUI with #171 and a retained layout tree (#173) | 64 M | 3.8 ms | 117 MB |
| RUI with offscreen culling in the renderer (#174) | 24 M | 1.5 ms | 57 MB |
| GPUI, full redraw | 157 M | 11.7 ms | 75 to 90 MB |
| GPUI, one row changed, rows as `.cached()` views | 346 M | 31.7 ms | 90 to 110 MB |

Those GPUI wall times were taken on a quieter machine than the table above, which is
why GPUI's wall time there is 4 to 8 times lower at about the same instruction count.
