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
| `paths` | one `canvas` painting `Primitive::Path`s; the fixture flattens curves to 0.125 px; circles are round `Quad`s | `Painter` with `PathShape`s, curves flattened by `CubicBezierShape::flatten`; `circle_filled` | a `Scene` of kurbo `BezPath`s and `Circle`s, encoded again each frame |
| `paths_icons` | 300 `svg` elements of the designs' markup | the same paths through `Painter`, every frame | each design encoded once into a `Scene`, `append`ed 300 times with a transform |
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

Apple M2 Max, macOS 26.5, load average 17–19 (1 min). Three runs per side,
interleaved; ranges are over the runs.

| Case | | RUI | egui | Vello |
|---|---|---|---|---|
| `paths` | instructions (M) | 30.4 | 54.6–54.7 | 16.4–17.8 |
| | allocations | 2 522 | 24 791 | 7 555 |
| | GPU ms p50 | 1.74–2.09 | 1.27 | 1.03–1.04 |
| | CPU ms p50 | 3.2–3.3 | 5.2–5.3 | 1.0–1.2 |
| `paths_icons` | instructions (M) | 31.3 | 67.2–67.4 | 7.1–8.0 |
| | allocations | 6 081 | 41 476 | 567 |
| | GPU ms p50 | 0.64 | 0.61 | 0.31–0.48 |
| | CPU ms p50 | 2.3 | 6.0–6.2 | 0.6–0.7 |

RUI draws `paths` in 197 draw calls and 55 078 instances, `paths_icons` in 140 and
6 001.

### Where each is ahead

- **Vello is ahead on both cases, on CPU and GPU.** On `paths` it uses 1.8x fewer
  instructions and about half RUI's GPU time; on `paths_icons` 4x fewer
  instructions and about 0.6x the GPU time. Vello's CPU work is encoding: it
  never flattens or tiles on the CPU, and an appended icon costs a copy of its
  encoding. RUI turns every path into device edges, tiles or atlas sprites on the
  CPU each frame.
- **egui's GPU time is lower than RUI's on `paths`** (1.27 against about 1.9 ms):
  it draws plain triangles, while RUI's path tiles test their edges per pixel.
  On the icons the two are even.
- **RUI is ahead of egui on CPU**: 1.8x fewer instructions on `paths`, 2.1x on the
  icons, and 7–10x fewer allocations; egui tessellates every shape each frame.

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

A path whose device edges and visible area are the same as on an earlier frame is
not tiled again: its tiles are copied from the kept ones (`PathTiles`), and a small
path drawn again is drawn from its rasterized atlas sprite (`PathSprites`). What
still runs every frame for an unchanged path is turning its vertices into device
edges, hashing them to find the kept tiles, and copying the tiles into the frame's
instance buffer, which is uploaded whole; the canvas closure also rebuilds every
`Primitive::Path`. Skipping that work needs paths that keep their identity across
frames (shared vertex storage in `Primitive::Path`), which is an API change left
for later.

## Not compared yet

GPUI (its side of the harness has no path cases); egui drawing icons from
`egui_extras`' rasterized SVG textures, which is how egui apps usually show icons
and would be cheaper than vector paths; Vello's MSAA modes.
