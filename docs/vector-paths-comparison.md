# Vector Paths: RUI vs egui vs Vello

RUI's other comparison fixtures have no vector paths. These two do, and they are
where Vello, which flattens and rasterizes paths in GPU compute shaders, should be
strong.

```sh
cargo bench --bench compare_frame -- paths paths_icons
cargo run --release --manifest-path benchmarks/egui_compare/Cargo.toml -- paths paths_icons
cargo run --release --manifest-path benchmarks/vello_compare/Cargo.toml -- paths paths_icons
```

`COMPARE_PNG=<dir>` makes each side also save frame 0 as `<dir>/<side>_<case>.png`.

## Fixtures

Both scenes come from `benchmarks/paths_scene.rs`, which every side includes, so
the shapes are generated from one seed and are the same everywhere. The window is
1440x900 at scale 2 and every side clears to `#101418`.

- **`paths`**: 2 000 translucent fills (stars, self-crossing pentagrams, cubic
  blobs, two-cubic leaves), then 500 polylines of 20–50 segments, 0.75–6 px wide,
  miter joins (limit 4) and butt caps, then 200 circles. Each frame one fill turns.
- **`paths_icons`**: 300 icons of 48 px in a 20x15 grid, each one of 12 designs of
  20 paths (fills, stars, strokes, circles) in a 24 px view box. Each frame one
  icon moves.

Each side uses its own idiomatic API:

| | RUI | egui 0.37 | Vello 0.12.0 |
|---|---|---|---|
| `paths` | one `canvas` painting `Primitive::Path`s every frame from vertices flattened to 0.125 px once and kept in `Arc`s (the turning fill is flattened again each frame); circles are round `Quad`s | `Painter` with `PathShape`s, curves flattened by `CubicBezierShape::flatten`; `circle_filled` | a `Scene` of kurbo `BezPath`s and `Circle`s, encoded again each frame |
| `paths_icons` | 300 `svg` elements of the designs' markup, each painting one `Picture` of its paths | the same paths through `Painter`, every frame | each design encoded once into a `Scene`, `append`ed 300 times with a transform |
| Target | Metal, offscreen | `egui_wgpu` into `Bgra8UnormSrgb` | `Renderer::render_to_texture`, `Rgba8Unorm`, area antialiasing |

Instructions are the whole process's, allocations are counted by each binary's
allocator, GPU time is `benchmarks/gpui_compare/gpu_time.rs` summed over a frame's
command buffers. No side waits for its frames.

## Correctness

Frame 0 of each side was saved and compared side by side and pixel by pixel.

- **Fill rule.** RUI and Vello fill by the nonzero rule: the pentagrams' centers
  are filled, concave stars are right. egui fills only convex polygons
  (`PathShape::fill` says so): it fan-triangulates every star, blob and leaf, so
  concave shapes spill over their outlines. That is egui's documented limit, not a
  harness bug, and it makes egui's picture visibly wrong.
- **Stroke joins.** RUI drew `Primitive::Path` strokes as butt-ended segment
  rectangles with nothing at the vertices, which left a notch on the outside of
  every turn (visible on the 4–6 px polylines). **Fixed**: strokes now get miter
  joins, beveled past a miter limit of 4, as SVG's default does. Vello (miter,
  limit 4) and RUI now match at the joins. egui's joins were not checked as closely. RUI's `svg` strokes already had
  joins and caps.
- **Antialiasing.** RUI's analytic coverage and Vello's area coverage agree to a
  pixel at edges; egui feathers each polygon by one device pixel and looks slightly
  softer.
- **Alpha.** RUI blends in linear light (it renders to an sRGB target); Vello
  blends the sRGB-encoded values. Overlapping translucent shapes are therefore
  brighter in RUI: a 50% `#c86432` over `#101418` is `#934927` in RUI and
  `#6c3c25` in Vello. That is a choice, not a bug, and it is why 48% of pixels
  differ from Vello by more than 16/255 while the geometry matches. egui into its
  sRGB target lands between the two.

## Results

Apple M2 Max, macOS 26.5, load average 13–44 (1 min). Three runs per side,
interleaved; ranges are over the runs. RUI before is this document's first
version, measured the same way.

| Case | | RUI before | RUI | egui | Vello |
|---|---|---|---|---|---|
| `paths` | instructions (M) | 30.4 | 8.71–8.73 | 54.6 | 16.5–17.7 |
| | allocations | 2 522 | 21 | 24 791 | 7 555 |
| | GPU ms p50 | 1.66–2.09 | 0.937–0.942 | 1.28 | 1.00–1.02 |
| | CPU ms p50 | 3.2–3.3 | 1.17–1.31 | 5.2 | 1.07–1.20 |
| `paths_icons` | instructions (M) | 31.3 | 5.04–5.05 | 65.7 | 5.70–5.99 |
| | allocations | 6 081 | 98 | 41 476 | 567 |
| | GPU ms p50 | 0.37–0.64 | 0.20–0.22 | 0.62 | 0.30–0.32 |
| | CPU ms p50 | 2.3 | 0.36–0.39 | 5.8 | 0.49–0.52 |

RUI draws `paths` in 197 draw calls and 55 084 instances, `paths_icons` in 102 and
6 001.

**GPU time follows the CPU's pace.** No side waits for its frames, so a side that
spends longer on the CPU leaves the GPU idle between frames, and the GPU clocks
down. Busy-waiting 2.2 ms after each RUI `paths` frame raises its GPU p50 from
0.94 to 1.79–2.09 ms with the same draws: the GPU halving from before is the CPU
work going away, not cheaper drawing. Of the 0.94 ms, skipping the per-pixel edge
loop of path tiles saves 0.11 ms and skipping their coverage entirely 0.19 ms;
the other 0.75 ms is rasterizing and blending 55 000 overlapping translucent
instances (5 interleaved runs each, medians 0.940, 0.830, 0.753 ms).

### Where each is ahead

- **RUI is ahead of Vello on CPU for both cases** (1.9x fewer instructions on
  `paths`, 1.1–1.2x on the icons) and level or ahead on GPU. An unchanged path
  costs RUI a lookup by its vertex storage and a copy of the tiles or atlas
  sprite it was drawn with; an icon is one `Picture` whose planned instances are
  copied whole.
- **Vello is ahead when geometry changes every frame.** RUI's caches key on the
  identity of the vertices: a path built anew each frame (a new `Arc`) is
  flattened, tiled or rasterized again, as before; Vello's cost is the same either
  way.
- **RUI is ahead of egui** on CPU by 6–13x and on GPU, and draws concave shapes
  right.

## RUI fixes from this comparison

| Fix | `paths` instructions (M) | `paths_icons` instructions (M) |
|---|---|---|
| before | 24.2 (joins missing) | 41.3 |
| miter joins on stroked paths (correctness) | 33.4 | 41.3 |
| `svg` finds markup passed before by address, not by hashing it | — | 35.4 (with the next) |
| path keys hashed in two lanes; join winding from one cross product | 30.4 | 35.4 |
| an element's paint reach built from its children's records | 30.4 | 31.3 |

- `svg(markup)` hashed the whole markup with SipHash every call, so 300 icons
  hashed about 1 MB a frame. It now keeps markup by address and length and only
  compares it, which is a `memcmp`.
- The atlas sprite and kept-tile caches keyed paths by a byte-serial FNV hash
  of every coordinate; the hash now runs two independent lanes.
- Each memoizable element measured the reach of every primitive below it, so a
  path four elements deep was measured four times. Its reach is now its
  children's recorded reaches plus its own primitives.

The joins cost 25% on `paths`, whose 500 polylines have about 17 000 joints: each
joint adds a miter of four edges.

## Caching of unchanged paths

`Primitive::Path` takes its vertices as `Arc<[PathVertex]>` plus an `offset` added
to each, so a path painted again shares its storage and moving it changes only the
offset. The planner keys a path by that storage, its offset, stroke width and
scale (`plan/path_cache.rs`): once a path comes back on a later frame its device
edges are kept, with the atlas sprite or the tiles it is drawn with, and a frame
only finds it and places them. Paths whose storage is new each frame still go
through the caches keyed by their edges (`PathSprites`, `PathTiles`).

A `Picture` is a list of primitives recorded once; `Primitive::Picture` draws it at
an offset. The planner keeps what it planned for a picture drawn again at the same
offset, scale and clip in a frame of the same size (`plan/picture.rs`): instances,
clips, and items, those that run on in one draw merged, so a grid of quads is one
item. A later frame copies them; a picture's quads are shared with the frame
rather than copied, and written once, into the instance buffer. A kept picture is
dropped when an atlas page its glyphs sample was emptied; pictures with images or
color glyphs, or drawn into a retained frame's damage, are planned every time.
`svg` icons are pictures, and `canvas(..).cache_key(key)` records its closure into
one (see iced's `canvas::Cache`): `quad_grid_cached` costs 0.58–0.59 M
instructions a frame, against 5.88 M before and 0.80–1.57 M for iced 0.14 measured
the same day.

| Change | `paths` (M) | `paths_icons` (M) | `quad_grid_cached` (M) |
|---|---|---|---|
| before | 30.4 | 31.3 | 5.88 |
| `canvas(..).cache_key` (paint replayed) | — | — | 5.61 |
| vertices in an `Arc`, paths kept by identity; fixture keeps its vertices | 8.77 | 13.3 | — |
| `Picture`, planned once and copied | 8.73 | 5.08 | 0.88 |
| a kept picture's quads shared, not copied | 8.72 | 5.05 | 0.59 |

What did not help: inlining `Primitive::reach`, marking the path entry point
`#[inline(never)]`, and finding the old places of moved replays only when damage
is asked; none moved instructions by more than 0.5%. Copying a kept picture's
quads into the frame cost 0.29 M instructions for 12 000 quads, a third of the
frame.

### Correctness

`COMPARE_PNG` draws frame 0 three times and fails when the frames drawn from kept
paths differ from the first by more than 4/255 in a channel: the atlas sprites are
rasterized on the CPU to 8-bit coverage, so they differ from analytic coverage by
up to 3/255. `paths_icons` frame 0 is identical to before. `paths` frame 0 differs
only along the window's left side: tiles cut by the visible area dropped the inside
of a path left of the first edge in the area, which left rectangular gaps in shapes
running off the left side. That is fixed, and the kept frames now agree with the
first, which they did not before (by up to 157/255).

## Not compared yet

GPUI (its side of the harness has no path cases); egui drawing icons from
`egui_extras`' rasterized SVG textures, which is how egui apps usually show icons
and would be cheaper than vector paths; Vello's MSAA modes.
