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
`CASES="quad_grid text_paragraphs"` picks cases.

- RUI: `benches/compare_frame.rs`, through `MetalRenderer::render_offscreen`.
- GPUI: `benchmarks/gpui_compare/fixtures.rs`, copied into a Zed checkout as
  `crates/gpui_platform/benches/rui_compare.rs`. It runs under `#[gpui::bench]` with
  GPUI's headless Metal renderer and `BENCH_MEASUREMENT=instructions`. It is built with
  `--features bench-support,runtime_shaders`, because this machine has no Xcode
  `metal` tool.

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

Apple M2 Max (12 cores, 96 GB), macOS 26.5.2. RUI `78f3fc8` (main) with this change,
rustc 1.99.1. GPUI at Zed `784374c`, rustc 1.98.1 (Zed's toolchain file), with
`runtime_shaders`. Three rounds, each case in a fresh process; cells are the range.

The machine was far from idle: other sessions were compiling, and the load average was
150 to 225 throughout. **Wall times are inflated and noisy on both sides; instructions
and allocations moved by under 2% between rounds and are the numbers to compare.**

| Case | Side | Instructions (M) / frame | ms / frame p50 | ms / frame p95 | Allocations / frame | Footprint (MB) | GPUI footprint at end (MB) |
|---|---|---|---|---|---|---|---|
| `list_full` | RUI | 25.3–25.4 | 1.6–3.0 | 12–41 | 24 020 | 57–59 | |
| | GPUI | 162–163 | 48–90 | 85–149 | 50 060 | 74–90 | 163–207 |
| `list_full_one_row` | RUI | 25.3 | 1.5–2.2 | 12–37 | 24 020 | 54–62 | |
| | GPUI `.cached()` | 356–357 | 105–181 | 212–254 | 6 205 | 93–114 | 121–167 |
| `list_virtual_1k` | RUI | 1.4–1.6 | 0.10–0.23 | 0.7–3.8 | 451 | 73–96 | |
| | GPUI | 3.6–3.8 | 0.66–1.14 | 1.8–2.0 | 787 | 19–60 | 109–157 |
| `list_virtual_100k` | RUI | 1.5–1.9 | 0.17–0.24 | 0.6–5.0 | 452 | 107–128 | |
| | GPUI | 3.6–3.8 | 0.60–1.02 | 1.4–2.2 | 787 | 22–55 | 156–188 |
| `text_paragraphs` | RUI | **286–287** | **44–50** | **77–88** | **163 833** | 45 | |
| | GPUI | 13.6–13.7 | 2.2–4.4 | 10–23 | 1 848 | 20–71 | 66–77 |
| `quad_grid` | RUI | 4.4 | 0.37–0.39 | 1.0–1.5 | 28 | 67–73 | |
| | GPUI | 80–84 | 14–66 | 35–200 | 18–39 | 75–78 | 86–93 |
| `resize_full` | RUI | 55.7–55.8 | 6.2–11.8 | 32–65 | 36 031 | 58–73 | |
| | GPUI | 166 | 37–74 | 81–127 | 50 060 | 71–92 | 223–242 |
| `resize_virtual` | RUI | 2.8–3.6 | 0.36–0.43 | 0.8–3.7 | 684 | **118–142** | |
| | GPUI | 8.4–9.1 | 0.94–1.5 | 1.7–5.6 | 788 | 22–73 | 310–463 |

| Startup | Process start to fixture code (ms) | First frame (ms) | Process start to first frame (ms) | Footprint (MB) |
|---|---|---|---|---|
| RUI | 9–11 | **3 450–8 620** | **3 460–8 630** | 20–22 |
| GPUI | 100–132 | 46–89 | 146–221 | 90–103 |

## Reading the numbers

Where RUI wins, by instructions:

- Long list, full redraw: 6.4 times fewer instructions than GPUI and half the
  allocations. Retained layout and renderer culling (`Primitive::may_touch`) mean
  the 1970 offscreen rows cost build, layout and paint but no Metal work.
- One changed row: GPUI's own incremental path (`.cached()` views) costs 2.2 times its
  full redraw at 2000 rows, so RUI's full rebuild is 14 times cheaper than it. GPUI
  does allocate 4 times less on that path.
- Virtual lists: 2.4 times fewer instructions than `uniform_list`, flat from 1000 to
  100 000 rows on both sides.
- Quad grid: 18 times fewer instructions for 12 000 quads. GPUI's side was not
  profiled, so where its 80 M go is not established here. Allocations are similar
  (GPUI 18–39, RUI 28).
- Resize: 3 times fewer instructions on both resize cases.

Where RUI loses:

- **Wrapped text is 21 times more instructions than GPUI, 89 times more allocations,
  and 10 to 20 times the wall time.** Sampling shows about three quarters of the
  frame in layout: `TextMeasureCache::wrap` reshapes every paragraph every frame
  (`shape_with_primary`), and `TextShapingFont::supports_cluster` builds a rustybuzz
  face (`hb_font_t::from_slice`) per cluster it checks. GPUI's line layout cache
  reuses the previous frame's wrapped lines, so one changed paragraph costs about one
  paragraph. This is the
  largest gap in the suite.
- **Startup: 3.5 s to the first frame in the best round (8.6 s in the worst) against
  GPUI's 0.15 to 0.22 s.** Renderer creation is only about 60 ms; the rest is shaping
  the first frame's 4000 strings, about three quarters of it in the same
  `supports_cluster` → `hb_font_t::from_slice` path. GPUI's text system loads fonts
  before the bench function, which is inside its 100–132 ms to fixture code, and its
  whole startup is still about 20 times faster.
- Footprint while resizing: RUI's offscreen path keeps one target texture and
  reallocates it on every size change, and footprint reaches 118–142 MB against GPUI's
  22–73 MB after its first call. GPUI's own footprint grows to 310–463 MB over
  thousands of resizes in one process, so neither is flat.
- Startup footprint is RUI's only clear memory win (20 MB vs 90–103 MB), partly because
  RUI has loaded less by then.

## Known unfairness

- Load: every number above was taken at load average 150–225 with other builds
  running. Wall-time ranges overlap heavily between rounds; repeat on an idle machine
  before quoting milliseconds.
- GPUI runs inside its test platform: each iteration also pumps the foreground
  executor and the frame profiler is enabled. Its instructions include that harness
  work; RUI's bench calls the pipeline directly.
- GPUI's p95 is draw-only (no present); RUI's p95 includes submit.
- GPUI's per-iteration statistics include Criterion warm-up; Criterion's instruction
  estimate does not.
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
- Fonts: each side uses its default font; they were not checked to be the same face.
  Both rasterize glyphs at device scale into an atlas with subpixel variants.
- Toolchains differ (rustc 1.99.1 for RUI, 1.98.1 for Zed) and both build with their own
  release profile.

## Not measured yet

- GPU time (`MTLCommandBuffer` `GPUStartTime` / `GPUEndTime`) and draw calls, which #197
  also asks for. Neither harness exposes them; both submit without waiting.
- Startup to a presented frame in a real on-screen window.
- Scrolling (offset changing per frame) and input latency.

## Earlier long-list results

Same machine, before this suite, GPUI at the same revision with its default 16 px text
(the rows now use 14 px on both sides):

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
