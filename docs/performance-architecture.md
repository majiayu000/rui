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
- **Rebuilding only what changed (done: #186 remainder).** A view made with
  `AppContext::new_view` is placed in any tree with `view(entity)`, which
  renders nothing. Entities read through `AppContext::get`/`get_mut` during a
  view's render are recorded. A frame that notified only entities some child
  view read, or the views themselves, does not build the root: it renders the
  notified views in place, reconciles each new subtree against the old one, and
  drops the layout and paint records on the path to them. The root is still
  built for an explicit rebuild, an entity it read itself, or an entity no view
  read, as before. GPUI tracks reads per view too; RUI's view keeps the
  per-element memo, replay and damage below it.
- **Lists without view types (ADR 0001).** `cx.rows(&entities, |item, cx| ..)`
  places a view per entity, keyed by the row closure's type and the entity, made
  on first use and released once no tree places it. Notifying an entity renders
  only its row. Rows render with the newest closure of their call site.
- **Target:** one-row change: build cost of one row plus the path to it. With
  rows as views, the 2000-row list's one-row frame costs 2.0 M instructions and
  23 allocations, 0.029 of a full frame. Written with `cx.rows` it costs the same,
  2.1 M and 23 (`list.one_row_changed_rows`, gated at 0.05 of a full frame).

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
- **Clip culling (done: #174).** A memoizable subtree that is unchanged since
  it was last painted, and whose recorded reach, moved to its new origin, misses
  the window and the active clip, is not painted at all. A subtree is painted
  once before it can be left out, because only painting it gives its reach.
  Damage counts only what the window can show, so content leaving the view
  costs nothing.
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
- **Pointer dispatch (#190 remainder).** Layout marks memoizable subtrees with
  no pointer handlers. Moves, releases and scrolls pass them by without
  computing their bounds, so a move over the 2000-row list costs 0.03 M
  instructions instead of 1.56 M. Presses still walk the tree, because a press
  clears focus in any div it misses. Routing handlers through the hit index
  instead would need every div with a handler to register a region.
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
- **Damage (done: #175).**
  - `Scene::damage` is everything painted rather than replayed in place,
    everything of the previous frame that was not replayed, and both places of
    what moved or changed order, unioned into one rectangle.
  - Each window keeps a `RetainedTarget`: a texture holding its last frame and
    the id of the scene frame it shows. Only primitives touching the damage are
    drawn into it, with every clip cut to the damage and a Load action, after
    the damage is filled with the clear color. A render pass then copies the
    texture to the rotating drawable; the normal frame path has no blit encoder,
    which on an M3 Max makes the driver allocate about 160 MB more.
  - A target that does not show the scene's previous frame (first frame, a
    frame that was painted but not presented), a new size or scale, and an
    image or other unbounded change are full damage.
  - One changed row on the 2000-row list draws 0.087 of the window again: 0.18
    ms of GPU time against 0.50 ms for a full redraw at 2880x1800 (M2 Max). The
    copy costs about 0.1 ms, so a nearly empty window gains nothing.
  - GPUI clears and redraws the whole window every frame.
- **Startup (#177).** Shader libraries compile on a background thread in parallel
  with font setup. Pipelines are cached in an `MTLBinaryArchive` keyed by the
  shader source hash; a mismatched or corrupt cache is rebuilt, never trusted.
  Build-time `metallib` is an option where the Xcode toolchain exists.
- **Color (done).** sRGB targets with linear blending, oklab gradients and erf
  Gaussian shadows, with text coverage remapped so weight does not change with
  color.
- **Sprite shader cost (done).** Measured by drawing a frame's sprite pass 100
  times in one command buffer: a single frame's GPU time is mostly clock
  ramp-up, which hides shader changes.
  - Aligned plain quads skip antialiasing. A solid, square-cornered, borderless
    quad whose device-pixel edges are whole numbers covers only pixels that lie
    wholly inside it. The vertex shader tests its edges and sets an
    `ALIGNED_QUAD` flag, and the fragment returns the color. GPUI does the same
    for unrounded, borderless quads. Rounded, bordered, gradient and fractional
    quads keep the antialiased paths. A flag fits the existing kind bits, where
    a second pipeline would need CPU-side classification and a pipeline switch
    per batch. The pixels that change are aligned edges, which drew at about 84%
    alpha before and left a light seam between adjacent list rows.
  - Vertex outputs carry 13 values instead of 45. The tiler writes every output
    of each of an instance's six vertices to memory, which dominated small
    sprites such as glyphs. Fragments read quad, shadow and clip fields by
    instance index, as GPUI does, and convert border and gradient colors per
    fragment. Pixels are unchanged.
  - Branch order matters in the one fragment function. Aligned quads come first
    and glyphs second; glyphs first would cost quads 14%.
  - Per pass at 2880x1800 (M-series, load 15–20), main → now:
    `list_virtual_1k` 0.064 → 0.037 ms, `quad_grid` 0.093 → 0.052 ms,
    `text_paragraphs` 0.036 → 0.023 ms.
  - What text still pays: glyph sampling and the coverage remap are about 2% of
    its pass. A fragment function that holds only the glyph path draws the pass
    in about 0.012 ms, so the remainder is the size of the shared fragment
    function. Splitting it needs a second pipeline.

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
| Startup shaders | precompiled metallib | renderer created off the main thread, in parallel with the first layout (#226); build-time metallib still open (#177) |

Every row is implemented on main as of `c2e3596`, except a build-time metallib (#177,
needs the Xcode `metal` tool). `MTLBinaryArchive` was measured and dropped: the cold
cost is the source compile, which runs before any archive lookup (#226).

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
   - `tests/damage_frames.rs`: frames through a `RetainedTarget` vs full
     redraws, byte for byte, over random edits, resizes, moved rows and a target
     that missed a frame. Injected bugs (not damaging removed content, moved
     replays, or skipping the clear) fail it.
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

| Fixture | Target | Result on `c2e3596` |
|---|---|---|
| One row changed, 2000-row list | < 10% of a full frame | rows as views: 0.029 (2.0 M instructions, 23 allocations); list rebuilt from the root: 0.25 |
| One row changed vs GPUI's `.cached()` case | 10x fewer instructions | 10.0x, with the list rebuilt from the root |
| Full redraw, 2000-row list | ≤ 50% of GPUI's instructions | 16% (25.1 M vs 159.7 M) |
| `virtual_list` 100 000 vs 1 000 rows | within 20% | equal; 4.2x fewer instructions than `uniform_list` |
| Text page resize within a wrap interval | zero reshapes, zero rewraps | met (`benches/text_cache.rs` fails otherwise) |
| Idle window | zero frames | met (#215, verified natively) |
| Startup to first frame, warm | below GPUI's | 53 ms vs 183–191 ms |
| Footprint | below GPUI's | lower on every fixture in `docs/gpui-comparison.md` |

## Stop conditions

From #179: if the incremental core is complete and a full redraw still cannot
match GPUI's cost, position RUI as a framework for small tools. The incremental
core is complete and a full redraw costs 16% of GPUI's, so this condition is not
met. The remaining risk is correctness, which the oracles above exist to catch,
and scope: RUI is macOS only.
