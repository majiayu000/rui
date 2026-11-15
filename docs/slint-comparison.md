# RUI vs Slint: Same Fixtures, Same Machine

Slint 1.18.1 draws RUI's comparison fixtures on the same machine, through two of its
renderers. The harness is `benchmarks/slint_compare`, a crate of its own so Slint
stays out of RUI's build:

```sh
cargo run --release --manifest-path benchmarks/slint_compare/Cargo.toml -- --renderer software list_full
cargo run --release --manifest-path benchmarks/slint_compare/Cargo.toml -- --renderer femtovg list_full
cargo bench --bench compare_frame -- list_full
```

All print one line per case in the same format. The Slint lines add `redrawn_pct`,
the share of the window's pixels the frame drew.

## Renderers

| Label | Renderer | Target | GPU time |
|---|---|---|---|
| `slint-software` | `SoftwareRenderer` in a `MinimalSoftwareWindow` with `RepaintBufferType::ReusedBuffer`, behind a custom `slint::platform::Platform` | a 2880x1800 `PremultipliedRgbaColor` buffer the harness owns | n/a: CPU only |
| `slint-femtovg-wgpu` | `FemtoVGWGPURenderer` (feature `renderer-femtovg-wgpu`, wgpu 30 on Metal) in a custom `WindowAdapter` | an offscreen 2880x1800 `Bgra8Unorm` texture; submits without waiting | `gpu_time.rs`, summed over the frame's command buffers |

The software renderer is Slint's headless and embedded path. With a reused buffer it
redraws only the dirty region, as it ships. FemtoVG redraws the whole window every
frame (`FemtoVGRenderer::render` has no partial path), so its `redrawn_pct` is 100.

Slint's Skia renderer also renders offscreen into a wgpu texture
(`slint::platform::skia_renderer::SkiaWGPU30Renderer`), so it could be a third column.
It is not measured here: it needs Skia's prebuilt binaries, a much larger build.

## Fixtures

The fixtures are `benches/compare_frame.rs`'s: a 1440x900 window at scale 2, 14 px
Arial on both sides (`ui/fixtures.slint` imports
`/System/Library/Fonts/Supplemental/Arial.ttf` and sets it as `default-font-family`).
The UI is written in `.slint` markup with the Fluent style. Each frame changes one
thing through a property or a model, then calls
`slint::platform::update_timers_and_animations()` and renders.

| Case | Slint | RUI |
|---|---|---|
| `list_full` | `ScrollView` > `VerticalLayout` > `for index in 2000: Row`; `Row` is a `Rectangle` with a `HorizontalLayout { alignment: space-between }` of two `Text`s; the frame sets `selected` | 2000 `div` rows of two `text`s, laid out by taffy |
| `list_virtual_1k`, `_100k` | `ListView { for index in count: Row }`, scrolled to the middle through `content-y` | `virtual_list` |
| `text_paragraphs` | `ScrollView` > `VerticalLayout { padding: 16px; spacing: 8px }` > `for content in paragraphs: Text { wrap: word-wrap; line-height-factor: 1.4 }`; the frame replaces row 3 of the `VecModel` | `text(..).wrap()` |
| `quad_grid` | a repeater of 12 000 7x15 `Rectangle`s whose colors come from a `[color]` `VecModel`; the frame sets two rows (the old white cell back, the new one white) | one `canvas` |
| `startup` | process start to the first `list_full` frame rendered (software) or submitted (FemtoVG), wgpu device creation included | the same, renderer creation included |

The grid is a repeater over a color model: Slint has no immediate-mode canvas, and a
model lets a frame touch only the two cells that change. A `Path` per cell or an
image would be other choices.

`line-height-factor` multiplies the font's natural line height, not the font size, so
Slint's paragraphs sit a little further apart than RUI's 1.4 (not checked against
RUI's definition).

### How much each Slint frame redraws

`redrawn_pct` sums the software renderer's dirty region:

- `list_full`, both virtual lists: **100%**. Every row's background binds
  `index == root.selected`; when `selected` changes, every row on screen is marked
  dirty and redrawn, though only two change color. Replacing the binding with a
  constant on all rows but one brought the region down to that one row (3.1%), so the
  binding is the cause. A per-row model would redraw two rows, but the property is how
  Slint's examples write a selection.
- `text_paragraphs`: **11.4%**, the edited paragraph's area.
- `quad_grid`: **0.02%**, the two cells. The renderer still walks all 12 000 items.

## Results (RUI `main` 2840887, Slint 1.18.1)

Apple M2 Max, macOS 26.5.2. Three interleaved rounds, each case in its own process:
RUI, then Slint software, then Slint FemtoVG. The machine was shared with other
agents: `uptime` load average 146–212 during the runs, so milliseconds are noisy
and the tables below use instructions, allocations and footprint. Instructions moved
by under 1.5% between rounds on every side, allocations not at all.

| Case | RUI instr. (M) | Slint SW instr. (M) | Slint FemtoVG instr. (M) | RUI allocs | Slint SW allocs | Slint FemtoVG allocs |
|---|---|---|---|---|---|---|
| list_full | 17.24–17.25 | 33.24–33.39 | 16.59–16.67 | 4 077 | 12 435 | 14 861 |
| list_virtual_1k | 0.71–0.73 | 19.62–19.64 | 5.85–5.99 | 151 | 637 | 3 059 |
| list_virtual_100k | 0.73–0.74 | 20.94–20.97 | 6.07–6.10 | 151 | 637 | 3 125 |
| text_paragraphs | 4.99–5.01 | 22.74–22.77 | 17.29–17.38 | 908 | 5 007 | 4 705 |
| quad_grid | 5.89–5.91 | 44.48–44.72 | 201.8–202.9 | 16 | 36 025 | 144 195 |

| Case | RUI footprint MB | Slint SW footprint MB | Slint FemtoVG footprint MB | RUI GPU ms p50 | Slint FemtoVG GPU ms p50 | Slint SW redrawn |
|---|---|---|---|---|---|---|
| list_full | 65–68 | 47 | 139–149 | 0.15 | 0.56–0.60 | 100% |
| list_virtual_1k | 56 | 28 | 120–173 | 0.15 | 0.24–0.54 | 100% |
| list_virtual_100k | 56–57 | 28 | 134–148 | 0.16 | 0.26–0.38 | 100% |
| text_paragraphs | 64–66 | 32 | 137–151 | 0.13 | 0.38–0.41 | 11.4% |
| quad_grid | 64–66 | 32 | 466–471 | 0.15 | 0.91–0.92 | 0.02% |

Footprint is `ri_phys_footprint` after the 70 frames. Slint software's includes its
20 MB pixel buffer; on Apple silicon, RUI's and FemtoVG's include their Metal
allocations.

| Startup, process start to first frame | RUI | Slint SW | Slint FemtoVG |
|---|---|---|---|
| three interleaved rounds | 78–89 ms (a cold first run: 8.8 s) | 368–833 ms | 387–740 ms |
| footprint | 25–28 MB | 47 MB | 42–43 MB |

Six earlier startup runs at a load of about 130 gave 214–354 ms for software and
239–367 ms for FemtoVG.

## Where each is ahead

- **Slint's software renderer has the smallest footprint**: 28–47 MB against RUI's
  56–68 MB, pixel buffer included, with no GPU device at all. It runs where there is
  no GPU.
- **Slint's software renderer redraws only what changed** when the change is
  local: 11% of the window for the paragraph edit, 0.02% for the grid. RUI redraws
  the whole window every frame; its GPU does that in about 0.15 ms here.
- **Slint FemtoVG matches RUI on `list_full` instructions** (16.6 M against 17.2 M),
  though it allocates 3.6x as much.
- **RUI is ahead everywhere else** in instructions: 27–28x fewer than Slint software
  and 8x fewer than FemtoVG on the virtual lists; 3.5–4.6x on text; 7.5x (software)
  and 34x (FemtoVG) on the grid, where FemtoVG builds a path per rectangle. RUI
  allocates 3x to 9000x less, its GPU time is 1.6–6x lower than FemtoVG's, and it
  starts at least 4x sooner.
- **Slint software spends half a list frame repainting rows that did not change**: in
  `list_full`, the run where only one row stayed dirty took 15.8 M instructions
  instead of 33.3 M, near RUI's 17.2 M.

## Not compared yet

The editor, resize and one-row cases; Slint's Skia renderer; a per-row-model
selection for Slint, which would let the software renderer redraw two rows; draw
calls on Slint's side.
