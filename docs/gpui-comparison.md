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
| `editor_scroll` | a 10 000-line code view: 20 px lines of 13 px SF Mono, a 64 px gutter of line numbers, 11 colored tokens per line (a comment every ninth line), the cursor's line highlighted; starts at line 5001 | scrolls down one line; the cursor moves with it | `virtual_list` of rows: gutter `text` + `shared_styled_text` of `span_range(..).color(..)` over the line's `Arc<str>` (the editor run below used `styled_text` of `span(..)`, a `String` per token) | `uniform_list` of rows: gutter string + `StyledText::with_highlights` |
| `editor_edit` | same, not scrolling | the cursor line's first identifier ends in a new letter | the line's tokens change and its `Arc<str>` and ranges are rebuilt | the line's tokens change and its `SharedString` and highlights are rebuilt |

The editor cases use the same font file on both sides,
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
  - Both benches fail unless exactly one command buffer completed per frame.
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
  [GPU time on both sides](#gpu-time-on-both-sides-rui-a5cae0b). There RUI is behind on
  the virtual lists and the resized long list.
- **One fixture set, one machine, one GPUI revision.** It says nothing about
  Linux, Windows or the web, where GPUI runs and RUI does not.
- **Fonts differ:** RUI's default is Arial, GPUI's SF Pro.


## GPU time on both sides (RUI `a5cae0b`)

All eleven cases, three rounds, with the hook above. Load average 26 at the start and
43 at the end. The CPU columns of this run agree with the final results and the editor
run: RUI is ahead on every case by instructions (1.1x to 15.5x). Its startup is ahead
3.8x. Only GPU time is new here.

| Case | RUI GPU ms p50 | GPUI GPU ms p50 | RUI p95 | GPUI p95 | RUI vs GPUI |
|---|---|---|---|---|---|
| list_full | 0.28–0.51 | 0.31 | 0.37–0.64 | 0.32–0.42 | tie (ranges overlap) |
| list_full_one_row | 0.51 | 6.43–6.90 | 0.51–0.64 | 11.3–12.5 | RUI ahead 13x |
| list_virtual_1k | 0.22–0.34 | 0.097 | 0.35–0.47 | 0.10–0.13 | RUI behind 2.9x |
| list_virtual_100k | 0.22 | 0.097 | 0.26–0.51 | 0.10–0.13 | RUI behind 2.2x |
| text_paragraphs | 0.13–0.27 | 0.20 | 0.16–0.44 | 0.20–0.91 | tie (ranges overlap) |
| quad_grid | 0.31 | 0.38 | 0.67–0.75 | 0.44–0.76 | RUI ahead 1.2x |
| resize_full | 0.51 | 0.28 | 0.51–0.89 | 0.32–0.52 | RUI behind 1.8x |
| resize_virtual | 0.24–0.47 | 0.22–0.28 | 0.34–1.11 | 0.30–0.51 | tie (ranges overlap) |
| editor_scroll | 0.24–0.25 | 0.30 | 0.29–0.52 | 0.31–0.60 | RUI ahead 1.2x |
| editor_edit | 0.24–0.26 | 0.30 | 0.48–0.55 | 0.31–0.64 | RUI ahead 1.2x |

What this shows:

- **The GPU is not where RUI wins.** Its CPU lead is 1.1x to 15x. On the GPU it is
  ahead only where GPUI's `.cached()` replay splits a frame into 3 983 draws (13x),
  and slightly on the grid and the editor (1.2x).
- **RUI is behind** on the virtual lists (2.2–2.9x) and on the resized long list
  (1.8x). Their instance counts are close: 1 056 vs 1 217 and 1 033 vs 1 683.
- **The cause is unknown**, and nothing here measures it. One shader draws every
  primitive on RUI's side, with per-fragment clip edges, where GPUI has a shader per
  primitive kind.
- **RUI's virtual list is GPU-bound** in this bench. Its buffers overlap and each ends
  about 0.22 ms after the previous one. The CPU frame (0.21–0.33 ms wall) waits
  whenever three frames are in flight. Its isolated frame, from the old waiting pass,
  was 0.16 ms, still 1.7x GPUI's.
- **RUI's ranges are wider** between rounds, for example `list_full` at 0.28 or 0.51
  ms. GPU clock state between processes is the likely cause; this run does not
  isolate it.

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
