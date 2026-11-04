# Ahead of GPUI: Roadmap

This document lists the work behind #179: making RUI faster by default than GPUI,
easier to use, and checkable by automation. It comes from reading GPUI, gpui_apple,
gpui_macos, scheduler and sum_tree at Zed `ead5672` (2026-11-04) next to RUI at
`5690b8f`.

Each item is marked **catch up** (GPUI already does it) or **ahead** (GPUI does
not, or cannot with its current design).

## What GPUI already does

Read the source before assuming GPUI is naive:

- View-level caching. `Entity::cached(style)` skips request_layout, prepaint and
  paint for a view that was not notified and whose bounds and text style did not
  change, and replays the previous frame's scene range, hitboxes, dispatch
  subtree and line layouts (`reuse_prepaint`, `reuse_paint`, `Scene::replay`).
- Automatic dependency tracking at view level: entity reads during render are
  recorded, and `notify` dirties the views that read it.
- R-tree draw order (`BoundsTree`): order only goes up when a primitive overlaps
  an earlier one, so batches span many elements.
- One pooled instance buffer per frame, returned on command buffer completion.
- Glyph atlas with subpixel variants, erf Gaussian shadows, linear blending and
  oklab gradients.
- Tooling: hdrhistogram profiler, frame journal, hang detection, debug overlay,
  inspector, seeded random element trees, counting allocator, hot reload.

## Where GPUI leaves room

- `layout_engine.clear()` every frame. Cached views skip layout only because the
  user gives them a fixed style.
- Caching is opt-in per view, and replays absolute coordinates.
- Full-window clear and redraw; no damage region.
- `hit_test` scans hitboxes linearly; the R-tree is used only for draw order.
- Paths go through an MSAA intermediate texture, with an encoder switch per batch.
- `LineLayoutCache` keeps only the previous frame.
- Foreground tasks are FIFO; priorities exist only on the background executor.
- 99 dependencies, an unstable API, and docs that amount to reading Zed.

## Work

### Usability first

Without these, no one can write a real app on RUI.

| Item | Issue |
|---|---|
| Handlers receive a context to update state | #184 |
| Background work wakes the UI (`AppHandle`, `spawn`) | #185 |
| Entity observe, subscribe, events, weak handles | #186 |
| More than one window | #187 |
| One widget per role; drop the terminal `hooks` module | #188 |
| Inherited text style and paragraph wrapping | #189 |

### Incremental core (ahead)

These share one data model and should be designed together.

| Item | Issue |
|---|---|
| Kept layout tree, default memo, relative-coordinate replay, element-level dependencies, animations dirty only themselves | #173 |
| Damage region redraw from the same replay data | #175 |
| Hit testing through a spatial index | #190 |
| Skip offscreen work, virtualized list (prefix sums for variable heights) | #174 |

### Renderer

| Item | Kind | Issue |
|---|---|---|
| No per-primitive buffers; pooled instance buffer | catch up | #170, PR #181 |
| Paths render at all | fix | #182, PR #183 |
| R-tree draw order and one shader for all instance kinds | catch up, then ahead | #192 |
| Paths without an intermediate texture | ahead | #193 |
| Linear blending, oklab gradients, Gaussian shadows | catch up | #194 |
| Precompiled shaders | catch up | #177 |
| Custom paint element | catch up | #176 |

### Text

| Item | Kind | Issue |
|---|---|---|
| Memory-mapped fonts, lazy table parsing | catch up | #171 |
| Glyph atlas, vertical subpixel variants, MSDF for large text | ahead | #172 |
| LRU line layouts, wrap results by width interval, intrinsic size cache | ahead | #195 |
| Fallback per script run, one implementation for all platforms | ahead | #196 |

### Scheduling

| Item | Kind | Issue |
|---|---|---|
| Display-link pacing, prioritized foreground queue, per-frame budget | ahead | #191 |

### Proof

| Item | Issue |
|---|---|
| Long-list stress benchmark | #180 |
| Budgets, incremental oracle, random trees, allocation counts, GPU time | #178 |
| Head-to-head benchmark against GPUI on the same fixtures | #197 |

### Later

Not scheduled: hot reload, inspector, container queries, accesskit, momentum
scrolling, and a wgpu backend for other platforms.

## Order

1. Land PR #183, PR #180 and PR #181.
2. #197, so every later change is measured against GPUI.
3. #184 and #185.
4. #173, #175 and #190 designed together.
5. The rest in the order of #179.
