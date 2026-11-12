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

## Complex text: `text_complex`

`text_paragraphs` is Latin only. `text_complex` uses the same layout (200 wrapped
paragraphs of 8 to 127 words, 14 px, one paragraph changed each frame) with words of
seven kinds. Both sides take them from `benchmarks/complex_text.rs`:

- Latin
- Simplified Chinese
- Arabic, right to left and joined
- Devanagari with conjuncts: क्षत्रिय, विद्यालय, संस्कृत, हिन्दी
- Thai, with no spaces between its words
- emoji: a ZWJ family, a flag, a skin tone, a face
- Arabic with digits in brackets, a right-to-left run inside left-to-right text

Any seven words in a row include one of each kind.

```sh
RUI_COMPARE_PNG=/tmp cargo bench --bench compare_frame -- text_complex
EGUI_COMPARE_PNG=/tmp cargo run --release --manifest-path benchmarks/egui_compare/Cargo.toml -- text_complex
```

The `*_COMPARE_PNG` variables save the last frame as `/tmp/rui_text_complex.png` and
`/tmp/egui_text_complex.png`.

### Fonts

Both sides use the same macOS faces. Every character takes the first face in this
list that has it:

| Script | Face | File |
|---|---|---|
| Latin, Arabic | Arial | `Supplemental/Arial.ttf` |
| Chinese | Hiragino Sans GB W3 | `Hiragino Sans GB.ttc` #0 |
| Devanagari | Kohinoor Devanagari Regular | `Kohinoor.ttc` #0 |
| Thai | Thonburi (ThonburiUI) | `ThonburiUI.ttc` #0 |
| Emoji | Apple Color Emoji | `Apple Color Emoji.ttc` #0 |

RUI's fallback picks these faces by itself (see
`devanagari_and_thai_take_the_system_faces`). egui gets them as an explicit chain, and
is built with `epaint/color_fonts` so emoji are drawn in color. egui reads the 180 MB
emoji file into memory, which is why its footprint is about 310 MB. RUI maps the file.

Line height differs between the two. RUI uses 1.4 × 14 px. egui uses the row height
from the fonts' own metrics.

### Correctness

Checked by eye against a CoreText rendering of the same strings (`NSAttributedString`,
Arial with system fallback).

| Check | RUI `main` | RUI this branch | egui 0.37 |
|---|---|---|---|
| Arabic joined | yes | yes | yes |
| Arabic right to left, digits left to right, brackets mirrored | yes, matches CoreText | yes | **no**: there is no bidi. A font run is shaped right to left as a whole, so `2024` is drawn `4202` and the space before a run moves to its end |
| RTL run inside LTR, first line of a paragraph | correct | correct | wrong (as above) |
| RTL run inside LTR, a wrapped line that starts with Arabic | **wrong**: the line is reordered as a right-to-left paragraph | **wrong**, not fixed | wrong |
| Devanagari conjuncts | **partly**: falls back to Arial Unicode, which leaves न्द as न् + द | yes (Kohinoor) | yes |
| Thai | drawn in Arial Unicode | Thonburi, as CoreText | Thonburi |
| Thai line breaks | only at spaces; a Thai run with no spaces is one unbreakable word (no dictionary breaking, no UAX #14) | same | same (spaces, CJK, then any character) |
| Fallback faces at the right size | **no**: sizes are descent-to-ascent heights, so Chinese is 12% larger than Latin and Devanagari 20% smaller | yes: fallback faces get the requested face's em | yes |
| Color emoji, ZWJ family as one glyph, flag, skin tone | yes | yes | yes, with `color_fonts` |
| Tofu | none | none | none |

The wrapped-line bidi bug: each wrapped line is shaped as text on its own, and
`line_levels` finds that text's base direction from its first strong character. A
line of an LTR paragraph that starts with an Arabic word is laid out right to left.
Fixing it means passing the paragraph's base level to each line's shaping request. That
touches `TextRequest` and the shape caches, so it is not in this change.

### Results

Apple M2 Max, macOS 26.5. Three interleaved rounds, each case in its own process.
Load average was 20–44 (1 min) and 76–153 (5 min) while other jobs ran. The
instruction and allocation counts varied by under 1% between rounds.

| `text_complex` | instructions (M) | allocations | GPU ms p50 | wall ms p50 | footprint (MB) |
|---|---|---|---|---|---|
| egui 0.37 | 15.29–15.42 | 3 078 | 0.133–0.139 | 1.18–1.25 | 308–315 |
| RUI `main` + fixture | 15.74 | 3 013 | 0.152–0.153 | 1.13–1.22 | 72–75 |
| RUI after the font fixes, before the classify fix | 17.45–17.47 | 3 157 | 0.117–0.158 | 1.42–1.79 | 76 |
| RUI this branch | 15.57–15.58 | 3 157 | 0.157 | 1.20–1.23 | 73–76 |

`text_paragraphs` did not move: RUI 4.96–4.97 M instructions and 908 allocations,
against 4.99–5.00 M before the change. egui was 5.36–5.39 M.

On complex text, RUI and egui cost about the same. RUI uses about 1% more instructions
and 2.5% more allocations, and does bidi, which egui does not.

The profile (`sample`, 30 000 frames) shows where RUI's frame goes. 63% is the changed
paragraph being shaped again for layout, and 16% is the same paragraph's lines being
shaped for drawing. Two hotspots were found:

- `char::is_alphabetic` took 14% of the frame. Cluster classification called it up to
  three times per character. It now reads the bidi class first, which saves 1.9 M
  instructions.
- ThonburiUI is a variable font without `HVAR`. For every glyph advance, ttf-parser
  decodes the outline and applies `gvar`, even at the default coordinates. That cost
  is about 7% of the frame. This is in ttf-parser, not RUI. The static faces in
  `Supplemental/Thonburi.ttc` are AAT (`morx`) only and cost far more under rustybuzz:
  26 M instructions. Sukhumvit Set is cheaper, at 14.2 M, but its Thai is loopless
  and does not look like the system's.

GPUI was not run for this fixture. `run.sh` builds Zed at a pinned revision, a long
build. The Zed checkout under `mooring/.deps` is at a different revision (1.24.3) and
has local changes. The machine's load was also above 100 for most of the session.
