# RUI vs egui: Same Fixtures, Same Machine

egui 0.37 draws RUI's comparison fixtures through `egui_wgpu` 0.37 (wgpu 30, Metal)
on the same machine. The harness is `benchmarks/egui_compare`, a crate of its own so
egui stays out of RUI's build:

```sh
cargo run --release --manifest-path benchmarks/egui_compare/Cargo.toml -- list_full
cargo bench --bench compare_frame -- list_full
```

Both print one line per case in the same format.

## Fixtures

The fixtures are `benches/compare_frame.rs`'s: a 1440x900 window at scale 2, 14 px
Arial on both sides (egui loads `/System/Library/Fonts/Supplemental/Arial.ttf`, RUI's
default face). Each frame changes one thing, then egui runs the UI, tessellates,
uploads and draws into an offscreen texture and submits without waiting, as RUI's
offscreen frames do.

| Case | egui | RUI |
|---|---|---|
| `list_full` | `ScrollArea` of 2000 rows; each row allocates 28 px and paints its background and two texts with `Painter` | 2000 `div` rows of two `text`s, laid out by taffy |
| `list_virtual_1k`, `_100k` | `ScrollArea::show_rows`, scrolled to the middle | `virtual_list` |
| `text_paragraphs` | 200 wrapped `Label`s, margin 16, spacing 8 | `text(..).wrap()` |
| `quad_grid` | 12 000 `Painter::rect_filled` | one `canvas` |
| `startup` | process start to the first `list_full` frame submitted, device creation included | the same, renderer creation included |

The egui rows paint with `Painter` rather than nesting `Label`s in layouts: that is
egui at its cheapest, so its row numbers are a lower bound for egui.

Instructions are the whole process's, allocations are counted by each binary's
allocator, and GPU time comes from `benchmarks/gpui_compare/gpu_time.rs` on both
sides. wgpu commits several command buffers a frame, the same number every frame, so
egui's frame GPU time is their sum.

## Results (RUI `main` 27a9be7, egui 0.37.0)

Apple M2 Max, macOS 26.5.2, load average 11–51. Three runs per case, each in its own
process; instructions and allocations moved by under 3% between runs (egui's virtual
lists up to 20%).

| Case | egui instructions (M) | RUI instructions (M) | egui allocations | RUI allocations | egui GPU ms p50 | RUI GPU ms p50 |
|---|---|---|---|---|---|---|
| list_full | 17.2–17.4 | 24.0 | 12 097 | 16 015 | 0.18–0.27 | 0.18–0.22 |
| list_virtual_1k | 1.22–1.32 | 0.85–0.93 | 288 | 299 | 0.06–0.13 | 0.15–0.22 |
| list_virtual_100k | 1.27–1.48 | 0.91–0.94 | 288 | 299 | 0.07–0.17 | 0.10–0.17 |
| text_paragraphs | 5.38–5.50 | 10.36–10.39 | 941 | 1 553 | 0.04–0.09 | 0.23 |
| quad_grid | 22.8–23.0 | 5.30–5.32 | 101 | 16 | 0.33 | 0.17–0.30 |

| Startup, process start to first frame | egui | RUI |
|---|---|---|
| five warm runs, interleaved | 35.9–57.6 ms | 60.1–74.8 ms |
| footprint | 28 MB | 25–30 MB |

## Where each is ahead

- **egui is ahead on `list_full`**, by 1.4x in instructions: it has no layout engine,
  and its rows place themselves top to bottom. RUI lays out 2000 flex rows.
- **egui is ahead on `text_paragraphs`**, by 1.9x in instructions and 4x in GPU time.
  #301 (glyph bounding boxes read once) brings RUI to 7.84 M, still 1.4x behind.
- **egui starts faster**, about 37 ms against 60 ms.
- **egui's GPU time is lower on the virtual lists and text**: it draws tessellated
  triangles with one small shader, where RUI's one shader handles every primitive.
- **RUI is ahead on the virtual lists** (1.4x fewer instructions) and on
  `quad_grid` (4.3x fewer instructions, half the GPU time): egui tessellates every
  rectangle into triangles each frame.

## Not compared yet

The editor, resize and one-row cases; draw calls on egui's side; and a `Label`-based
row for egui, which would cost it more than the `Painter` rows here.
