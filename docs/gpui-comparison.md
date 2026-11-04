# RUI vs GPUI: Long-List Comparison

Same fixture, same machine, both frameworks drawing through Metal. Tracks #197.

Run it with:

```sh
benchmarks/gpui_compare/run.sh
```

## Fixture

2000 rows of 28 px in one column, each a background, a title and a time, in a
1440x900 window at scale 2. Nothing is culled. Each measured frame rebuilds, lays
out, paints and submits to Metal without waiting for the GPU.

- RUI: `benches/compare_frame.rs`, through `MetalRenderer::render_offscreen`.
- GPUI: `benchmarks/gpui_compare/long_list.rs`, run inside a Zed checkout with
  `#[gpui::bench]` and its headless Metal renderer, `BENCH_MEASUREMENT=instructions`.

Instructions are whole-process retired instructions on both sides.

## Results

Apple M2 Max, macOS 26.5.2. RUI `5690b8f` (main) and `da6778d` (PR #181). GPUI at
Zed `784374c`, built with `runtime_shaders`.

| Case | Instructions / frame | Wall time / frame | Footprint |
|---|---|---|---|
| RUI main, full redraw | 873 M | 63.2 ms | 1150 MB |
| RUI with PR #181, full redraw | 91 M | 4.6 ms | 1134 MB |
| RUI main with #171 and a retained layout tree (#173) | 64 M | 3.8 ms | 117 MB |
| RUI with offscreen culling in the renderer (#174) | 24 M | 1.5 ms | 57 MB |
| GPUI, full redraw | 157 M | 11.7 ms | 75 to 90 MB |
| GPUI, one row changed, rows as `.cached()` views | 346 M | 31.7 ms | 90 to 110 MB |

RUI with PR #181 draws the same pixels as main (stress benchmark checksum
`ecf5a44a7e3b5308` on both).

GPUI's footprint is read after the first measured run; it rises by about 7 MB on
each later run of the bench function in the same process. GPUI reports 50 K heap
allocations per full-redraw frame and 6 K per cached frame.

## Reading the numbers

- With PR #181, RUI's full redraw costs about 58% of GPUI's instructions and 40%
  of its wall time.
- GPUI's own incremental path is slower than its full redraw at this size:
  replaying 2000 cached views costs more than rebuilding them. Caching per element
  by default (#173) has to avoid that per-view overhead.
- The renderer skips primitives that cannot reach the visible area, so the 1970
  offscreen rows cost build, layout and paint but no Metal work or text
  rasterization. GPUI submits all of them in this fixture too; a GPUI app would
  normally use `uniform_list` to avoid building them at all, which RUI does not
  have yet (#174).
- Memory was RUI's worst gap: about 13 times GPUI, from reading whole font files
  (#171). With #171 and culling it is now below GPUI on this fixture.
- RUI keeps its layout tree across frames, so a one-row change re-lays out only
  that row's ancestors: layout drops from 40 M to 12 M instructions in
  `stress_frame`. Build, paint and Metal still redo the whole list.

## Not equal yet

- RUI rasterizes text at the logical font size, then scales it to the 2x target.
  GPUI rasterizes glyphs at device scale with subpixel variants. RUI does less
  text work and its text is blurrier at 2x.
- RUI renders text as one texture per string; GPUI uses a glyph atlas (#172).
- Time to first frame is measured for RUI only (about 2.3 s, mostly font loading).
- One fixture. #197 also asks for wrapping text, many small quads and resize.
