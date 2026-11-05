# Performance Architecture

How RUI turns one input into one presented frame, what each stage must cost, the
algorithm and data structure behind that cost, and how each claim is checked.
It is the design behind #179 and the lanes in `docs/ahead-of-gpui-roadmap.md`.

## Principles

1. **Cost follows change.** A frame where one row changes costs in proportion to
   that row, not to the tree. Every stage needs an answer to "what didn't change,
   and how do we skip it?"
2. **Cost follows what is visible.** Rows, glyphs and primitives that cannot reach
   the screen are never built, rasterized or drawn.
3. **Idle is free.** No frames, timers or wakeups unless something is dirty or
   animating.
4. **Memory follows use.** Resident memory grows with the glyphs, images and rows
   in use, never with font files or list length.
5. **Fast by default.** Application code does not opt into caching. GPUI makes the
   user write `.cached(style)` with a fixed size; RUI must get there without it.
6. **Every claim has a check.** A stage is not faster until `perf_gate` budgets it
   and an oracle shows it still produces the same output.

## Where the time goes today

2000-row long list, 1440x900 at scale 2, one row's selection changing per frame,
M2 Max (`benches/perf_gate.rs`, `benches/compare_frame.rs`):

| Stage | Instructions | Allocations | Scales with |
|---|---|---|---|
| Build (user `render` + element construction) | ~8 M | ~14 K | whole tree |
| Layout (retained taffy tree) | ~12.6 M | ~10 K | whole tree (visits), changed nodes (computes) |
| Paint | ~3 M | small | whole tree |
| Metal (after culling) | ~1 M | small | visible primitives |
| **Frame** | **~24 M, ~1.5 ms** | **~24 K** | |
| GPUI, same fixture, full redraw | 157 M, 11.7 ms | 50 K | whole tree |
| GPUI, rows as `.cached()` views, one row changed | 346 M, 31.7 ms | 6 K | number of views |

With `virtual_list` the same rows cost ~0.4 M instructions at 1 000 or 100 000
rows. What is left in the plain list is build, layout and paint visiting every
element. The incremental core removes that.

## The frame, stage by stage

```
input/effects -> build -> layout -> paint -> hit index -> render -> present
     |            |         |         |          |           |         |
  priority     memo by   retained  replay by   lazy grid   damage    display
   queue      identity    taffy    relative    on demand   scissor     link
                                    ranges
```

### 1. Effects and scheduling (lane 5, #191)

- **Queue:** one foreground queue with four priorities: input > animation > layout >
  idle. Each frame has a time budget from the display refresh, and idle work yields
  when the budget is spent. GPUI's foreground queue is FIFO.
- **Wakeups:** `AppHandle` posts one application event per burst of updates, and a
  display link runs only while something is dirty or animating.
- **Dependencies:** `observe`, `subscribe` and `notify` (#186) are flushed in
  `prepare_frame` and again after listener updates, so derived state shows in the
  same frame. A flush is bounded to 64 rounds.
- **Target:** zero frames while idle; input handled before queued idle work;
  animations at display rate.

### 2. Build (lane 2, #173 step 1–2; #186 remainder)

- **Stable identity.** An element's id is its explicit key, or else its parent's
  identity plus its child index and type. It survives rebuilds, so it can key
  caches, focus, hit regions and accessibility.
- **Fingerprints.** Each identified element hashes its inputs (style, content,
  children's fingerprints) during construction, which costs O(1) per element on
  top of building it.
- **Rebuilding only what changed.** Today a notify rebuilds from the root view.
  Making views entities (#186 remainder) lets a notify rebuild just the views that
  read the changed entity, with entity reads tracked during `render`. GPUI tracks
  this per view; RUI can track per element with an id.
- **Target:** one-row change: build cost of one row plus the path to it.

### 3. Layout (done: retained tree; lane 4: caches; lane 2: skip unchanged subtrees)

- **Done:** the retained taffy tree (`LayoutNodes`) reuses nodes in creation order
  and restyles only changed ones, so taffy's per-node cache survives frames. GPUI
  calls `layout_engine.clear()` every frame. Layout fell from 40 M to 12.6 M.
- **Remaining cost:** visiting every element to compare styles. With identity and
  fingerprints, an unchanged subtree reuses its nodes without being visited, which
  gives O(changed + ancestors).
- **Text measurement (#195):**
  - Shaped lines live in an LRU with generations; GPUI keeps only the last frame.
  - A paragraph's wrap is the same for every width in an interval `[w1, w2)`, found
    from prefix sums over break opportunities. A resize that stays inside the
    interval rewraps nothing, whereas GPUI treats every width as a new key.
  - Intrinsic sizes are cached by (text, style, width bucket).
- **Lists:** `virtual_list` lays out only visible rows plus overscan, giving
  O(visible). Rows still in view keep their element instances, and with them their
  state.

### 4. Paint (lane 2, #173 step 2; #174 remainder)

- **Replay.** Each identified element records the range of scene primitives it
  produced, relative to its own origin. If its fingerprint and size are unchanged,
  paint copies the range and adds the new origin. GPUI replays absolute
  coordinates, so any move invalidates the cache; RUI's relative ranges survive a
  resized sidebar or a scroll.
- **Clip culling.** A child entirely outside the active clip is not painted.
- **Scene storage.** Primitives go into one `Vec` reused across frames. Ranges are
  index pairs and replay is a `memcpy` plus an offset pass, so a frame allocates
  nothing proportional to the tree.
- **Target:** one-row change paints one row; a scroll paints the newly visible
  rows and replays the rest.

### 5. Hit testing (done: #190)

- **On demand.** Regions register as a plain push during paint. A grid index is
  built only after 32 queries in a frame with at least 64 regions. That is the
  break-even point, so frames with a few queries pay nothing extra, and frames
  with many get queries of about 35 ns regardless of size.
- **Future:** with replay, a replayed subtree's regions can be reused with an
  offset too.

### 6. Render (lane 3, #192, #193, #177, atlas; lane 2, #175)

- **Culling (done).** `Primitive::may_touch` skips anything that cannot reach the
  visible clip, using conservative margins for antialiasing, blur, stroke and text
  overflow.
- **Draw order by overlap (#192).** A bounds tree assigns each primitive an order
  that rises only when it overlaps an earlier one. Non-overlapping primitives of
  one kind then batch across the whole scene instead of only when adjacent. On
  top of that, one instance layout and one shader for quads, glyphs, underlines
  and shadows means a list draws in a constant number of draws. GPUI still
  alternates quad and sprite batches per row.
- **Glyphs (done, with follow-ups).** One shared A8 atlas at device scale with
  subpixel variants, and a shaping cache that keeps the strings the previous frame
  drew. Follow-ups: per-page in-flight tracking for eviction, and blit uploads.
- **Paths (#193).** Static paths are rasterized once into the atlas. Dynamic paths
  get analytic coverage in the fragment shader. No intermediate MSAA texture and no
  encoder switch per path batch, which GPUI pays.
- **Damage (#175).**
  - Paint replay marks which ranges changed; their bounds, unioned, are the
    damage region.
  - Each window keeps a persistent offscreen color texture. Only primitives
    touching the damage are drawn into it, scissored and with a Load action, and
    the texture is then blitted to the rotating drawable.
  - Resize, scale change and the first frame are full damage.
  - GPUI clears and redraws the whole window every frame.
- **Startup (#177).** Shader libraries compile on a background thread in parallel
  with font setup. Pipelines are cached in an `MTLBinaryArchive` keyed by the
  shader source hash; a mismatched or corrupt cache is rebuilt, never trusted.
  Build-time `metallib` is an option where the Xcode toolchain exists.
- **Color (done).** sRGB targets with linear blending, oklab gradients and erf
  Gaussian shadows, with text coverage remapped so weight does not change with
  color.

### 7. Present (lane 5, #191)

The display link paces presentation and the drawable is acquired as late as
possible, after layout and paint, to keep input-to-photon latency low. Telemetry
(`RUI_PROFILE=1`) reports per-frame stage times, drawable wait and
event-to-render latency.

## Memory model

| Resource | Policy | Today |
|---|---|---|
| Fonts | `mmap`ed, faces share one mapping; clean file pages, not footprint | 1150 → ~30–90 MB process total |
| Glyph atlas | 1024² A8 pages, grow on demand; eviction by frames-in-flight (lane 3) | grows only |
| Shaping, wrap, measure caches | bounded LRU with generations (lane 4) | bounded |
| Layout tree | one node per laid-out element, reused | retained |
| Scene | one primitive buffer reused across frames | reused |
| Lists | `virtual_list` keeps only built rows | O(visible) |

Allocations per frame are budgeted by `perf_gate` because they are exact on any
machine. The target for a one-row change is allocations proportional to that row.

## Algorithmic advantages over GPUI

| Area | GPUI (Zed 784374c) | RUI design |
|---|---|---|
| Layout tree | cleared every frame | retained; restyle only changed nodes |
| Caching | opt-in `.cached(style)` per view, fixed size | default, by identity and fingerprint |
| Replay | absolute coordinates; any move invalidates | relative ranges; moves replay with an offset |
| Dependency tracking | per view | per identified element |
| Redraw | full window | damage region on a persistent texture |
| Hit testing | linear scan over hitboxes | lazy grid after break-even |
| Draw batching | R-tree order, separate quad/sprite batches | R-tree order + one shader for all instance kinds |
| Paths | MSAA intermediate texture, encoder switch per batch | atlas sprites / analytic coverage in one encoder |
| Line layout cache | previous frame only | LRU with generations |
| Wrap cache | keyed by exact width | keyed by width interval |
| Font fallback | Core Text, per-platform code | script runs over rustybuzz, one implementation |
| Foreground scheduling | FIFO | priority queue with frame budget |
| Startup shaders | precompiled metallib | binary-archive cache + parallel init (metallib where available) |

Rows that are done today: layout tree, hit testing, color, glyph atlas, culling.
The rest are in lanes 2–5.

## Verification

Each optimization ships with three things.

1. **An oracle.** The optimized path must produce the same output as the plain
   one, over seeded random inputs.
   - `tests/retained_layout.rs`: retained vs fresh layout, 24 seeds × 30 frames of
     reshaping trees, wrapped text included.
   - `tests/incremental_frames.rs`: reconciled, kept and replayed frames vs fresh
     full frames, 32 seeds × 40 frames of random edits and resizes. Injected bugs
     (skipping the inherited text style check, the unrounded size check, the
     exact-translation check, or comparing div styles) fail it.
   - `RUI_VERIFY_REPLAY=1` repaints every frame in full and panics at the first
     primitive that differs from the replayed one.
   - Lane 2 adds damaged vs full-redraw pixels.
   - Hit index vs linear scan: 300 random scenes.
   - Injected bugs (skip a restyle, skip a rewire) must fail the oracle; this has
     been shown for the layout oracle.
2. **A budget.** `cargo bench --bench perf_gate` gates instructions where the
   counter exists, and allocations, virtual-list scaling and footprint
   everywhere. It runs in CI. Deliberately slowed builds have been shown to fail
   it.
3. **A head-to-head number.** `benchmarks/gpui_compare/run.sh` runs the same
   fixtures on RUI and on GPUI at a pinned Zed revision. Lane 1 extends it to
   virtual lists vs `uniform_list`, text-heavy pages, many quads, resize and
   startup. Results and every known unfairness live in `docs/gpui-comparison.md`.

"Faster than GPUI" is claimed per fixture, only from that table, measured
serially on one quiet machine.

## Targets

| Fixture | Target |
|---|---|
| One row changed, 2000-row list | < 10% of a full frame; below GPUI's `.cached()` case by 10x |
| Full redraw, 2000-row list | ≤ 50% of GPUI's instructions (met: ~15%) |
| `virtual_list` 100 000 vs 1 000 rows | within 20% (met: equal) |
| Text page resize within a wrap interval | zero reshapes, zero rewraps |
| Idle window | zero frames |
| Startup to first frame, warm | below GPUI's |
| Footprint, long list with renderer | below GPUI's (met on this fixture) |

## Stop conditions

From #179: if the incremental core is complete and a full redraw still cannot
match GPUI's cost, position RUI as a framework for small tools. On the current
numbers the full redraw already costs less, so the remaining risk is
correctness, which the oracles above exist to catch.
