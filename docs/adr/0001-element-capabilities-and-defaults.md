# ADR 0001: Element capabilities, the default fast path, and file splits

Status: accepted. Date: 2026-11-05. Base: `main` at c2e3596.

## Context

The retained frame grew one optional `Element` method per feature: `memoizable`
and `ignores_pointer` (#173 step 2, #174), `reconcile` (#173), `adopt_stable_id`
(#173 step 1), and `children_mut` and `refresh_views` (#186). A new container has
to know which of them to implement, and forgetting one fails quietly:

- `children` without `children_mut`: views placed inside never render.
- `ignores_pointer` without `memoizable`: never read.
- `memoizable` without `reconcile`: memo works only while the instance survives
  (view content); on a root rebuild nothing is kept.
- Each container that reconciles has to pair its children, compare, and swap
  them back, as `Div::reconcile` does today.
- `adopt_stable_id` has five identical bodies (`Input`, `ScrollView`, `Button`,
  `Canvas`, `VirtualList`), each with its own `explicit_id: bool`.
- `refresh_views` has one override, `ViewElement`, which is the frame's own type.

Cost of one changed row in the 2 000-row list (perf_gate on c2e3596, M2 Max):

| case | instructions | allocations | of a full frame |
|---|---|---|---|
| `list.full_frame` | 67.5 M | 36 069 | 1 |
| `list.one_row_changed` (rebuilt from the root) | 17.1 M | 14 012 | 0.254 |
| `list.one_row_changed_views` (rows as views, #234) | 2.0 M | 23 | 0.029 |

A probe split the rebuilt case up: building and dropping the 2 000-row tree costs
11.7 M and 14 001 allocations, `reconcile` adds 3.8 M, and layout plus paint of
the kept tree add 1.6 M. The same bench allocates and frees 14 000 small boxes
for 6.0 M. So the allocator is about a third of the rebuilt frame. Most of the
rest is the walk, which is O(rows) however cheap the allocation.

## Decision

### 1. One capability declaration; the frame owns the walks

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// Layout and paint depend only on what `same_props` compares, the
    /// inherited text style, the children and the size layout gives it.
    /// (was `memoizable`)
    pub pure: bool,
    /// Pointer moves, releases and scrolls change nothing it shows or calls.
    /// Read only when `pure`. (was `ignores_pointer`)
    pub pointer_inert: bool,
}

pub trait Element: Any {
    fn capabilities(&self) -> Capabilities { Capabilities::default() }
    /// For pure elements: whether `self` lays out and paints as `previous`,
    /// children aside. (was `reconcile`, without the child pairing)
    fn same_props(&self, _previous: &dyn Element) -> bool { false }
    /// Where an element that needs an id keeps it. (was `adopt_stable_id`)
    fn stable_id_mut(&mut self) -> Option<&mut StableId> { None }
    fn children(&self) -> &[AnyElement] { &[] }
    fn children_mut(&mut self) -> &mut [AnyElement] { &mut [] }
    // style, layout, paint, events, accessibility: unchanged
}
```

- `AnyElement::reconcile` pairs the children of a pure element by position,
  calls `same_props`, and swaps the kept children back. `Div` keeps only its
  property comparison. Non-pure containers are not reconciled, as today, so the
  refactor is behavior-identical; reconciling the children of non-pure containers
  such as `ScrollView` is a separate, measured change.
- `refresh_views` leaves the trait. `AnyElement` walks `children_mut` and
  handles `ViewElement` itself, as it already does when it computes
  `contains_views`.
- `StableId { id: ElementId, explicit: bool }` replaces the five copies:
  `StableId::new()` makes an ephemeral id, `StableId::explicit(id)` is what
  `.id()` sets, and the frame calls `adopt` in layout. `Element::id()` stays.
- `AnyElement::new` asserts in debug builds that `children()` and
  `children_mut()` have the same length, naming the type, so a container that
  forgets `children_mut` fails in tests.
- A new container implements `children`/`children_mut` and, if pure,
  `capabilities` and `same_props`. Nothing else.
- Old names are not kept: all implementers are in this crate and move in the
  same PR.

### 2. View boundaries are the default fast path; no element arena

An arena makes the allocations cheap but leaves the walk. The probe gives an
upper bound: at best the rebuilt frame drops the 6.0 M of allocator cost, from
17.1 M to about 11 M, or 0.16 of a full frame. That misses #173's 0.10 target.
An arena also conflicts with the retained frame:

- `reconcile` keeps the previous frame's instances. Arena-backed elements would
  have to be copied out, or the arena would need two generations with kept
  subtrees pinned.
- It adds a lifetime to `AnyElement` and to every element, which breaks every
  lane's API at once.

A view boundary removes the walk: the row's parent does not rebuild, so cost
follows what changed (0.029). Rows as views should be the normal way to write a
list, without hand-written view types:

```rust
// A view per entity, keyed by (row fn type, entity id), made on first use.
// Notifying one entity renders that row again and nothing else.
div().flex_col().children(cx.rows(&tasks, |task: &Task, cx| task_row(task, cx)))
```

Migration:

1. Add `rows` on top of the existing `ViewElement` and `render_child_view`. Its
   views are released when no frame places them, which needs #236 (view release,
   lane E). Add a perf_gate case `list.one_row_changed_rows`, gated at ≤ 0.05 of
   a full frame.
2. Move `DataList`, the examples and the docs to it. Rebuilding from the root
   stays correct through `reconcile`, but it is the fallback.
3. Virtual lists build rows during layout, so views cannot sit in them today.
   They stay as they are (0.4 M per frame at 100 k rows) until lane E gives rows
   a pre-layout build.

### 3. File splits (pure moves, one PR each)

- `src/elements/element.rs` (1 733 lines) becomes `src/elements/element/`:
  `mod.rs` (the trait, `Capabilities`, `StableId`, `IntoElement`, `Render`),
  `layout.rs` (`LayoutContext`, `LayoutNodes`, measured text, `style_to_taffy`),
  `paint.rs` (`PaintContext`), `event.rs` (`EventContext`, `PointerEvent`,
  `EventResult`), and `any.rs` (`AnyElement`, reconcile, layout keep, paint
  replay, action dispatch).
- `src/renderer/metal/renderer.rs` (1 605 lines): `renderer.rs` keeps
  `MetalRenderer`, construction, pipelines and the `Renderer` impl.
  `planning.rs` gets `FrameData` and `build_frame` with its path, image and text
  builders. `encoding.rs` gets `encode_frame` and the scissors. `resources.rs`
  gets the textures and the offscreen target. The existing `capture.rs` gets the
  capture and offscreen entry points.
- `src/platform/mac/app.rs` (956 lines): `app.rs` keeps `run_app*`, which is
  the runner. `window_driver.rs` gets `NativeWindowDriver`, `NativeWindow` and
  `run_iteration`. `event_loop.rs` gets `run_between_frames`, redraw scheduling
  and the viewport sync. `dogfood_automation.rs` gets the automation.
- `benchmarks/perf_budgets.json` becomes `benchmarks/perf_budgets/<case>.json`,
  with `{instructions_m, allocations}` and a note for each case, plus
  `benchmarks/perf_budgets/shared.json` for headroom, the calibration note, the
  footprint, the damage, the scaling and the checksum. The numbers stay the same.

## Consequences

- Lanes B to F write new elements against the trait in section 1. Element code
  in open PRs needs a mechanical rename when it rebases.
- The arena is off lane F's list. Lane F's work on the list path is the `rows`
  perf case and keeping it at or under 0.05.
- Text allocations at paint (`Primitive::Text` copies, about 900 per frame in
  `text.paragraphs`) are not affected by this ADR.
