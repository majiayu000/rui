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

| Case | iced | RUI |
|---|---|---|
| `list_full` | `scrollable` of a `column` of 2000 rows; each row is a `lazy` `container` (28 px, background) around a `row` of two `text`s | 2000 `div` rows of two `text`s, laid out by taffy |
| `list_virtual_1k`, `_100k` | iced has no virtual list: written here as a `scrollable` whose `column` holds the rows on screen (`lazy`) between two spacers, scrolled to the middle with the `scroll_to` operation | `virtual_list` |
| `text_paragraphs` | 200 wrapping `text`s in a `scrollable` `column`, padding 16, spacing 8, line height 1.4; the 199 that never change are `lazy` | `text(..).wrap()` |
| `quad_grid` | one `canvas`: the 12 000 cells in a geometry `Cache` that is never cleared, the changed cell drawn on top each frame | one `canvas`, every cell painted each frame |
| `startup` | process start to the first `list_full` frame submitted, font loading and device creation included | the same, renderer creation included |

Each fixture is written the fastest idiomatic way iced offers, so iced's numbers are
not handicapped. Two of those choices change what is compared:

- **`lazy` rows.** iced rebuilds only the two rows whose selection changed; RUI
  rebuilds all 2000 (RUI's retained version is `list_full_one_row`, not compared here).
  Without `lazy`, one run of iced's `list_full` took 19.6 M instructions and 22 064
  allocations.
- **Cached grid.** iced tessellates the grid once and reuses the mesh; RUI paints all
  12 000 cells every frame. With the cache replaced by a fresh `Frame` each frame, two
  runs of iced's `quad_grid` took 16.1 M instructions and 12 143 allocations.

Instructions are the whole process's, allocations are counted by each binary's
allocator, and GPU time comes from `benchmarks/gpui_compare/gpu_time.rs` on both
sides. wgpu commits several command buffers per iced frame; a frame's GPU time is
their sum. In `text_paragraphs` some frames commit one more buffer (250 for 60 frames),
so buffers cannot be split into frames there and only the mean per frame is reported
(`gpu_ms_mean`). No MSAA, as iced's `Settings::antialiasing` is off by default. Text
uses iced's default shaping, `Shaping::Auto`, which shapes ASCII text with
cosmic-text's basic shaper.

## Results (RUI `main` 2840887, iced 0.14.0)

Apple M2 Max, macOS 26.5.2, load average 154–175 (the machine was shared). Three
rounds, each running every case once per side, iced then RUI, each in its own process.
Milliseconds are not reported for that reason; instructions and allocations are.

| Case | iced instructions (M) | RUI instructions (M) | iced allocations | RUI allocations | iced GPU ms p50 | RUI GPU ms p50 |
|---|---|---|---|---|---|---|
| list_full | 12.64–12.67 | 17.23–17.25 | 12 084 | 4 077 | 0.27 | 0.15 |
| list_virtual_1k | 1.19–1.27 | 0.73–0.74 | 296–297 | 151 | 0.12–0.17 | 0.15–0.16 |
| list_virtual_100k | 1.20–1.32 | 0.70–0.75 | 295 | 151 | 0.14–0.17 | 0.16 |
| text_paragraphs | 5.24–5.28 | 5.00–5.03 | 1 242 | 908 | 0.10–0.12 (mean) | 0.13 |
| quad_grid | 0.70–1.23 | 5.88–5.91 | 78 | 16 | 0.07–0.14 | 0.15 |

| Startup, process start to first frame | iced | RUI |
|---|---|---|
| three runs, interleaved | 86–172 ms | 53–133 ms (one more run stalled 9 s before `main`, left out) |
| footprint | 45 MB | 25 MB |

## Where each is ahead

- **iced is ahead on `list_full`**, by 1.36x in instructions, because `lazy` keeps
  1998 of the 2000 rows from being rebuilt. Without `lazy` it is 1.14x behind RUI. RUI
  still makes 3x fewer allocations and uses half the GPU time.
- **iced is ahead on `quad_grid`**, by 5–8x in instructions and about half the GPU time,
  because its geometry cache keeps the tessellated grid and only one cell is drawn
  fresh. RUI has no retained geometry for a canvas; it rebuilds 12 000 instances each
  frame. Drawn the same way as RUI (no cache), iced is 2.7x behind.
- **iced's GPU time is lower on `text_paragraphs`** (mean 0.10–0.12 ms against 0.13).
- **RUI is ahead on the virtual lists** (1.7x fewer instructions, half the
  allocations), slightly ahead on `text_paragraphs` (5% fewer instructions, 27% fewer
  allocations), makes fewer allocations in every case, and starts with about half
  the memory.

## Not compared yet

The editor, resize and one-row cases; draw calls on iced's side; and iced at a quiet
machine, where its startup (its first run here took 1.7 s to the first frame, later
runs under 0.2 s) can be profiled.
