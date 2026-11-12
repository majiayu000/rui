# RUI vs iced: Same Fixtures, Same Machine

iced 0.14 (`iced_widget` 0.14.2) draws RUI's comparison fixtures through `iced_wgpu`
0.14 (wgpu 27, Metal) on the same machine. The harness is `benchmarks/iced_compare`, a
crate of its own so iced stays out of RUI's build:

```sh
cargo run --release --manifest-path benchmarks/iced_compare/Cargo.toml -- list_full
cargo bench --bench compare_frame -- list_full
```

Both print one line per case in the same format.

## Fixtures

The fixtures are `benches/compare_frame.rs`'s: a 1440x900 window at scale 2, 14 px
Arial on both sides (iced loads `/System/Library/Fonts/Supplemental/Arial.ttf` into its
font system and makes it the default font). Each frame changes one thing, then the
harness does what `iced_winit` does for a redraw after a message: builds the view into
`iced_runtime`'s `UserInterface` against the kept widget tree (diff and layout), sends it
a `RedrawRequested` event, draws it, and presents through `iced_wgpu::Renderer` into an
offscreen texture without waiting for the GPU. iced's offscreen path needed no
stand-in: `Renderer::present` takes any texture view.

Every case has a plain version, which builds the whole view each frame on both sides.
Two cases also have a cached version, written with each library's own opt-in tool.

| Case | iced | RUI |
|---|---|---|
| `list_full` | `scrollable` of a `column` of 2000 rows; each row is a `container` (28 px, background) around a `row` of two `text`s | 2000 `div` rows of two `text`s, laid out by taffy |
| `list_full_rows` (cached) | the same rows, each wrapped in `lazy` keyed on its index and selection, so only the two rows whose selection changed rebuild | the same rows through `cx.rows`, one view per row entity; the two rows whose selection changed are notified and render again |
| `list_virtual_1k`, `_100k` | iced has no virtual list: written here as a `scrollable` whose `column` holds the rows on screen between two spacers, scrolled to the middle with the `scroll_to` operation | `virtual_list` |
| `text_paragraphs` | 200 wrapping `text`s in a `scrollable` `column`, padding 16, spacing 8, line height 1.4 | `text(..).wrap()` |
| `quad_grid` | one `canvas` tessellating all 12 000 cells into a fresh `Frame` each frame, the changed cell white | one `canvas` painting all 12 000 cells each frame, the changed cell white |
| `quad_grid_cached` (cached) | one `canvas`: the cells in a geometry `Cache` that is never cleared, the changed cell drawn on top each frame | the grid in a `canvas` view that is never notified, the changed cell a 7x15 white `div` placed absolutely over it in a view of its own |
| `startup` | process start to the first `list_full` frame submitted, font loading and device creation included | the same, renderer creation included |

Both cached grids draw the same picture as `quad_grid`. On RUI's side, the white quad
lands at the changed cell (checked at frame 69: x 483, y 0, 7x15).

Instructions are the whole process's, allocations are counted by each binary's
allocator, and GPU time comes from `benchmarks/gpui_compare/gpu_time.rs` on both
sides. wgpu commits several command buffers per iced frame; a frame's GPU time is
their sum. In `text_paragraphs` some frames commit one more buffer, so buffers cannot
be split into frames there and only the mean per frame is reported (`gpu_ms_mean`).
No MSAA, as iced's `Settings::antialiasing` is off by default. Text uses iced's default
shaping, `Shaping::Auto`, which shapes ASCII text with cosmic-text's basic shaper.

## Results (RUI `main` 2840887 plus the two cached cases, iced 0.14.0)

Apple M2 Max, macOS 26.5.2, load average 332–344 (the machine was shared). Three
rounds, each running every case once per side, iced then RUI, each in its own process.
Milliseconds are not reported for that reason; instructions and allocations are.

| Case | iced instructions (M) | RUI instructions (M) | iced allocations | RUI allocations | iced GPU ms p50 | RUI GPU ms p50 |
|---|---|---|---|---|---|---|
| list_full | 19.50–19.56 | 17.25–17.37 | 22 064 | 4 077 | 0.26–0.27 | 0.15 |
| list_full_rows (cached) | 12.65–12.72 | 2.62 | 12 084 | 17 | 0.26–0.27 | 0.15 |
| list_virtual_1k | 1.37–1.49 | 0.72–0.73 | 445 | 151 | 0.12–0.16 | 0.14–0.16 |
| list_virtual_100k | 1.35–1.41 | 0.74–0.75 | 445–447 | 151 | 0.16–0.17 | 0.13–0.16 |
| text_paragraphs | 5.89–5.92 | 5.00–5.02 | 1 043 | 908 | 0.10–0.12 (mean) | 0.13 |
| quad_grid | 16.27–16.39 | 5.89–5.90 | 12 136–12 138 | 16 | 0.14–0.17 | 0.15 |
| quad_grid_cached (cached) | 0.64–0.84 | 5.88–5.90 | 78 | 23 | 0.09–0.11 | 0.15 |

| Startup | iced | RUI |
|---|---|---|
| first frame, three interleaved runs | 183–297 ms | 35–73 ms |
| footprint | 45 MB | 25 MB |

The first round of both startups stalled 8–13 s before `main`, so process-to-frame
times are left out.

## Where each is ahead

- **Plain, RUI is ahead in every case**: 1.13x fewer instructions on `list_full`,
  1.9x on the virtual lists, 1.18x on `text_paragraphs` and 2.8x on `quad_grid`, and
  fewer allocations in each. iced tessellates every canvas rectangle into triangles.
- **Cached list, RUI is ahead by 4.8x** (2.62 M against 12.65 M) and makes 17
  allocations against 12 084. `lazy` saves iced the rebuild of unchanged rows but
  still lays out all 2000 each frame; RUI's untouched row views keep their layout and
  replay their paint.
- **Cached grid, iced is ahead by 7–9x** (0.64–0.84 M against 5.9 M) and uses less GPU
  time. RUI's cached variant is no faster than its plain one: `Canvas` does not declare
  itself pure (its closure is opaque), so its paint is never replayed. At frame 69 the
  scene replayed 1 of its 12 003 primitives; the 12 000 grid quads were painted again.
  RUI has no retained geometry for a canvas yet.
- **iced's GPU time is lower on `text_paragraphs`** (mean 0.10–0.12 ms against 0.13).
- **RUI's GPU time is lower on both `list_full` cases** (0.15 ms against 0.26–0.27).
- **RUI starts faster** (35–73 ms to the first frame against 183–297 ms) with half the
  memory.

## Not compared yet

The editor, resize and one-row cases; draw calls on iced's side; cached text (`lazy`
paragraphs on iced's side, paragraph views on RUI's).
