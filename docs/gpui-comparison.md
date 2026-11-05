# RUI vs GPUI: Same Fixtures, Same Machine

Both frameworks draw the same fixtures through Metal on one machine. Tracks #197.

Run it with:

```sh
benchmarks/gpui_compare/run.sh [zed-checkout-dir]
```

The script builds both sides once, launches each binary once so macOS's first-launch
signature check stays out of the startup case, then runs every case in its own
process: RUI, then GPUI, `REPEATS` times (default 3). It prints one table with the
range over the repeats and keeps the raw lines in `target/gpui_compare/results.txt`.
`CASES="quad_grid text_paragraphs"` picks cases. A last table gives a verdict per
case: RUI ahead, behind, or a tie when the two ranges overlap or the midpoints are
within 5% (instructions) or 10% (wall p50).

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
| `list_full_one_row` | same | one row toggles its selection | same frame as `list_full`: RUI has no per-view cache | each row a view drawn with `.cached(..)`, only that row notified |
| `list_virtual_1k`, `list_virtual_100k` | same rows, 1000 or 100 000, scrolled to the middle | the selection moves within the visible rows | `virtual_list` | `uniform_list` |
| `text_paragraphs` | 200 wrapped paragraphs of 8 to 127 words, padding 16, gap 8 | paragraph 3 (on screen) gets a new revision suffix | `text(..).wrap()` | string children, which wrap by default |
| `quad_grid` | 200x60 cells of 7x15 px, one color each | one cell turns white | one `canvas`, `Painter::fill` | one `canvas`, `window.paint_quad(fill(..))` |
| `resize_full` | the 2000-row list | width alternates 1440 / 1280 | new viewport size | `window.resize` + `window.bounds_changed`, as the platform resize callback does |
| `resize_virtual` | the 1000-row virtual list | width alternates 1440 / 1280 | same | same |
| `startup` | the 2000-row list | none | see below | see below |

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
- GPU ms / frame (RUI only): a separate pass of 30 frames after the CPU samples, whose
  frames wait for the command buffer and report `GPUEndTime - GPUStartTime`. Waiting
  never happens in the CPU samples. GPUI's headless renderer commits its command
  buffer inside `render_scene` and returns nothing, so its GPU time cannot be read
  without changing Zed; it is not reported.
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

## Results

Apple M2 Max (12 cores, 96 GB), macOS 26.5.2. RUI `f7968cc` (main) with this change,
rustc 1.99.1. GPUI at Zed `784374c`, rustc 1.98.1 (Zed's toolchain file), with
`runtime_shaders` and `font-kit`. Three rounds, each case in a fresh process; cells
are the range.

Other sessions were compiling on the machine; the load average was 80 to 120. **Wall
times are noisy on both sides; instructions and allocations moved by under 3% between
rounds (GPUI's cached case 3.5%) and are the numbers to compare.**

| Case | Side | Instructions (M) / frame | ms / frame p50 | ms / frame p95 | Allocations / frame | Draw calls | Instances | GPU ms p50 | Footprint (MB) | GPUI footprint at end (MB) |
|---|---|---|---|---|---|---|---|---|---|---|
| `list_full` | RUI | 25.7 | 1.53 | 1.6–1.8 | 24 014 | 1 | 1 033 | 0.37–0.51 | 58–61 | |
| | GPUI | 159.5–159.7 | 11.7–17.4 | 12–47 | 50 046 | 2 | 1 683 | | 78 | 270–277 |
| `list_full_one_row` | RUI | 25.8 | 1.54–1.57 | 2.4–23 | 24 014 | 1 | 1 033 | 0.50 | 60–65 | |
| | GPUI `.cached()` | 257–266 | 26–32 | 33–66 | 6 100–6 114 | **3 983** | 75 972 | | 193–224 | 226–573 |
| `list_virtual_1k` | RUI | 1.8–2.0 | 0.11–0.21 | 0.22–0.29 | 446 | 1 | 1 056 | 0.16–0.21 | 126–152 | |
| | GPUI | 3.9–4.0 | 0.32–0.35 | 0.33–0.71 | 787 | 2 | 1 217 | | 62–91 | 164–249 |
| `list_virtual_100k` | RUI | 1.5–2.0 | 0.12–0.20 | 0.22–0.50 | 446 | 1 | 1 122 | 0.16–0.17 | 132–149 | |
| | GPUI | 4.0–4.1 | 0.31–0.37 | 0.29–0.76 | 787 | 2 | 1 281 | | 71–92 | 89–132 |
| `text_paragraphs` | RUI | **286–287** | **17.0–19.3** | **17–48** | **163 821** | 1 | 6 271 | 0.28 | 46–50 | |
| | GPUI | 19.3–19.4 | 1.36–1.49 | 1.4–3.9 | 1 842 | 1 | 6 375 | | 69–70 | 88–98 |
| `quad_grid` | RUI | 5.1 | 0.31 | 0.32–0.49 | 15 | 1 | 12 000 | 0.27 | 83–102 | |
| | GPUI | 80.0–80.1 | 6.4–8.2 | 8.7–32 | 14–15 | 1 | 12 000 | | 76–78 | 95–101 |
| `resize_full` | RUI | 55.8–56.2 | 2.8–3.8 | 3.1–8.4 | 36 025 | 1 | 1 033 | 0.45–0.50 | 60–71 | |
| | GPUI | 164.3–164.7 | 12.7–13.9 | 19–21 | 50 045 | 2 | 1 683 | | 79 | 274–277 |
| `resize_virtual` | RUI | 2.6 | 0.34–0.41 | 0.8–1.6 | 679 | 1 | 1 056 | 0.50 | **150–261** | |
| | GPUI | 8.9–9.0 | 0.56–0.67 | 0.8–1.2 | 788 | 2 | 1 217 | | 71 | 390–503 |

RUI's GPU p95 was within 0.12 ms of its p50 in every case.

| Startup | Process start to fixture code (ms) | First frame (ms) | Process start to first frame (ms) | Footprint (MB) |
|---|---|---|---|---|
| RUI | 6–9 | **1 334–1 559** | **1 340–1 569** | 22 |
| GPUI | 92–116 | 120–134 | 212–245 | 75–106 |

Verdict, as `run.sh` prints it:

| Case | By instructions | By wall p50 |
|---|---|---|
| `startup` | | **RUI behind 6.4x** (process start to first frame) |
| `list_full` | RUI ahead 6.2x | RUI ahead 9.5x |
| `list_full_one_row` | RUI ahead 10.2x | RUI ahead 18.6x |
| `list_virtual_1k` | RUI ahead 2.1x | RUI ahead 2.1x |
| `list_virtual_100k` | RUI ahead 2.3x | RUI ahead 2.1x |
| `text_paragraphs` | **RUI behind 14.8x** | **RUI behind 12.7x** |
| `quad_grid` | RUI ahead 15.8x | RUI ahead 23.2x |
| `resize_full` | RUI ahead 2.9x | RUI ahead 4.0x |
| `resize_virtual` | RUI ahead 3.5x | RUI ahead 1.6x |

## Reading the numbers

Where RUI wins:

- Long list, full redraw: 6.2 times fewer instructions than GPUI and half the
  allocations. Retained layout and renderer culling (`Primitive::may_touch`) mean
  the offscreen rows cost build, layout and paint but no Metal work: RUI submits
  1 033 instances, GPUI 1 683.
- One changed row: GPUI's own incremental path (`.cached()` views) costs 1.6 times its
  full redraw at 2000 rows, so RUI's full rebuild is 10 times cheaper. Replaying the
  cached views also breaks GPUI's batching: **3 983 draw calls and 75 972 instances**
  per frame, every row's glyphs included, against 2 draws when the list is rebuilt.
  GPUI still allocates 4 times less than RUI on that path.
- Virtual lists: 2.1 to 2.3 times fewer instructions than `uniform_list`, flat from
  1000 to 100 000 rows on both sides.
- Quad grid: 16 times fewer instructions for the same 12 000 quads in one draw on each
  side. GPUI's side was not profiled, so where its 80 M go is not established here.
- Resize: 2.9 to 3.5 times fewer instructions.
- Draw calls: RUI draws every fixture in 1 call (#217 draws quads and glyphs with one
  shader); GPUI uses 2 (quads, then glyph sprites) except the text-only and quad-only
  fixtures.
- GPU time: RUI's frames take 0.16 to 0.51 ms of GPU time. There is no GPUI number to
  compare it with.

Where RUI loses:

- **Wrapped text: 14.8 times more instructions than GPUI, 89 times more allocations,
  and about 13 times the wall time.** Both sides submit about the same glyphs (6 271
  vs 6 375 instances), so the gap is all CPU. Sampling showed about three quarters of
  the frame in layout: `TextMeasureCache::wrap` reshapes every paragraph every frame
  (`shape_with_primary`), and `TextShapingFont::supports_cluster` builds a rustybuzz
  face (`hb_font_t::from_slice`) per cluster it checks. GPUI's line layout cache
  reuses the previous frame's wrapped lines (`LineLayoutCache::previous_frame`), so
  one changed paragraph costs about one paragraph. This is the largest gap in the
  suite.
- **Startup: 1.3 to 1.6 s to the first frame against GPUI's 0.21 to 0.25 s**, 6.4 times
  slower. Renderer creation is about 60 ms; the rest is shaping the first frame's
  4000 strings, mostly in the same `supports_cluster` → `hb_font_t::from_slice` path.
  GPUI loads its text system before the bench function, inside its 92–116 ms to
  fixture code, and is still far faster to the first frame.
- Footprint while resizing: RUI's offscreen path keeps one target texture and
  reallocates it on every size change, and footprint reaches 150–261 MB against GPUI's
  71 MB after its first call. RUI's GPU time also triples while resizing (0.50 ms
  against 0.16 ms for the same virtual list without resizing). GPUI's own footprint
  grows to 390–503 MB over thousands of resizes in one process, so neither is flat.
- Virtual-list footprint is higher on RUI (126–152 MB vs 62–92 MB after GPUI's first
  call).
- Startup footprint is RUI's clear memory win (22 MB vs 75–106 MB), partly because RUI
  has loaded less by then.

## Known unfairness

- Load: every number above was taken at load average 80–120 with other builds
  running. Repeat on an idle machine before quoting milliseconds.
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
  rows.
- `list_full_one_row` on RUI is the same frame as `list_full`: RUI has no per-view
  cache, so there is no closer equivalent of GPUI's `.cached()` path.
- Resize on GPUI calls what its resize callback calls (`window.resize`,
  `window.bounds_changed`) from inside a frame update, since `simulate_resize` cannot be
  called there. Neither side resizes a real `CAMetalLayer`.
- Fonts differ: with no family set, RUI uses the first face it loads (Arial), GPUI
  `.SystemUIFont` (SF Pro) through Core Text. Both rasterize glyphs at device scale into
  an atlas with subpixel variants.
- Toolchains differ (rustc 1.99.1 for RUI, 1.98.1 for Zed) and both build with their own
  release profile.

## Measurement hook in RUI

GPU time needed one addition to `src/`: `MetalRenderer::render_offscreen_gpu_time`
renders like `render_offscreen`, waits for the command buffer, and returns
`GPUEndTime - GPUStartTime`. The command buffer never leaves the renderer, and
metal-rs 0.31 does not wrap those two properties, so a bench cannot read them from
outside. The existing paths are unchanged apart from sharing the offscreen-target
lookup.

## Not measured yet

- GPU time on GPUI (see above).
- Startup to a presented frame in a real on-screen window.
- Scrolling (offset changing per frame) and input latency.

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
