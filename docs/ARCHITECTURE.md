# RUI Architecture

RUI is a retained, GPU-rendered UI framework for Rust, with a Metal backend
on macOS. Application state lives in an `AppContext`. Views build element
trees. Each frame keeps every part of the tree, its layout and its paint that
did not change, and the renderer draws again only the pixels that changed.

Related documents:

- [`performance-architecture.md`](performance-architecture.md): the cost of each
  stage and the algorithm behind it.
- [`gpui-comparison.md`](gpui-comparison.md): the same fixtures measured
  against Zed's GPUI.
- [`API.md`](API.md): the public API, checked by `tests/docs_api_drift.rs`.
- [`adr/`](adr/): architecture decisions, starting with
  [ADR 0001](adr/0001-element-capabilities-and-defaults.md) on element
  capabilities and the default fast path.
- [`platform-window-boundary.md`](platform-window-boundary.md): what the platform
  window layer must provide.
- [`devtools.md`](devtools.md): the profiling overlay and the damage and replay
  visualizer.

## Layers

| Layer | Module | Owns |
|---|---|---|
| Runtime | `core` | `AppContext`, entities, views and rows, windows, tasks, actions and keymaps, clipboard, drag and drop, menus, image assets, accessibility tree, `Presenter`, `FramePipeline` |
| Elements | `elements`, `advanced_ui` | the `Element` trait, `AnyElement`, layout, paint and event contexts, built-in and themed widgets, SVG icons |
| Renderer | `renderer` | `Scene` and primitives, text measurement, shaping, fallback and glyph atlas, the Metal backend |
| Platform | `platform` | the window contract (`platform::window`) and the macOS runner (`platform::mac`) |
| Devtools | `devtools` | the profiling overlay and the damage and replay visualizer, drawn into each window's scene |
| Testing | `testing` | headless sessions, frame capture and snapshots, driven by the same frame pipeline |

Dependencies point down the table. Renderers never touch native windows: the
platform layer hands them a renderer target.

## Module layout

The files that used to pass 800 lines are split into child modules. Each child
starts with `use super::*;`, and items shared between siblings are
`pub(super)`.

```
src/
├── core/
│   ├── app.rs, app/          # AppContext; app/{rows, release, images, clipboard, menus}.rs
│   ├── presenter.rs, presenter/  # one window's frame; presenter/{drag, menus}.rs
│   ├── frame_pipeline.rs     # FrameStage::ORDER, run by every runner
│   ├── drag.rs, menu.rs      # drag and drop, native menu declarations
│   ├── action/, accessibility/, text_editing/
│   └── entity.rs, view.rs, windows.rs, executor.rs, foreground.rs, ...
├── elements/
│   ├── element.rs            # Element, Capabilities, StableId, IntoElement, Render
│   ├── element/              # layout, paint, event, any (AnyElement), reconcile
│   ├── view_element.rs       # view(entity) in a tree
│   ├── div.rs, text/, input.rs, text_area.rs, scroll_view.rs,
│   │   virtual_list.rs, image.rs, svg.rs, canvas.rs, list.rs, table.rs, ...
├── advanced_ui/              # themed controls built from elements
├── renderer/
│   ├── scene.rs, primitives.rs   # Scene, Primitive, Reach, damage
│   ├── text/                 # measure cache, wrap, rich text, glyph atlas
│   └── metal/
│       ├── renderer.rs       # MetalRenderer
│       ├── renderer/         # planning, encoding, resources, offscreen
│       └── retained.rs, shaders.rs, path.rs, glyph.rs, order.rs, ...
├── platform/
│   ├── window.rs             # the window contract
│   └── mac/
│       ├── app.rs            # run_app, window opening, the loop
│       ├── app/              # window_driver, event_loop, dogfood_automation
│       └── window.rs, events.rs, clipboard.rs, file_drop.rs, menus.rs,
│           accessibility.rs, frame_ticker.rs, ime_state.rs, ...
├── devtools/                 # profiler, damage, window
└── testing/
benchmarks/perf_budgets/      # one budget file per perf_gate case, plus shared.json
benches/perf_budgets/         # the loader perf_gate uses
```

## Runtime

- `AppContext` is the state root. Entities (`Entity<T>`, stored in a `SlotMap`)
  hold application state. `cx.notify(entity)` marks the views whose last render
  read that entity.
- A view (`AppContext::new_view`) is an entity that renders. `view(entity)`
  places it in a tree as a `ViewElement`. It renders when it is first laid out
  and again only when it is notified, without rebuilding the tree around it.
- `cx.rows(&entities, |item, cx| ..)` places one view per entity without a
  view type (`core::app::rows`). Each row records its entity as read, so
  notifying the entity renders that row and nothing else.
- Views are released once no tree places them (`core::app::release`): the
  rebuild drops the view, its read set and its queued notifications, unless the
  same rebuild placed it again. `AppContext::release` releases one explicitly.
- Windows share entities. Each window has its own frame state, presenter,
  scene, focus and renderer target (`core::windows`). A runner activates a
  window before running its frame.
- Work on the UI thread is prioritized (`core::foreground`: input, animation,
  layout, idle). Animations request frames. `cx.spawn` runs futures on the UI
  thread and `cx.background_spawn` runs them on worker threads
  (`core::executor`). Foreground tasks are polled at the start of a frame.
- Keys go through a keymap to typed actions, which are dispatched to the
  focused element first (`core::action`).

## Frame pipeline

`FramePipeline` (in `core::frame_pipeline`) runs the stages in
`FrameStage::ORDER`, for the native and headless runners alike:

1. **Prepare.** Apply handle updates, poll foreground tasks, run frame tasks,
   flush effects and queue view notifications.
2. **Rebuild.** If the root read a notified entity, or a rebuild was asked for,
   build the root again and reconcile it against the previous tree. Then render
   the views that are new or notified.
3. **Layout.** Taffy computes layout on a tree kept across frames.
4. **Dispatch events.** Events reach the laid-out tree: pointer events through
   the scene's hit index, keys and actions through focus.
5. **Paint.** Elements push primitives into the `Scene`.
6. **Present.** The renderer draws the scene.
7. **Finish.** Complete the redraw and clear the frame's requests.

`Presenter` (in `core::presenter`) holds one window's root, layout tree, scene
and focus, and runs these stages for it.

## Incremental core

What a frame keeps from the previous one:

- **Identity.** In layout, every `AnyElement` gets a stable id: the id the app
  set, or else one derived from its parent's id, its position and its type
  (`ElementId::derived`). Elements that need an id for focus, hit regions or
  accessibility keep it in a `StableId` and receive this identity. Composites
  derive the ids of their parts from it.
- **Capabilities.** An element declares `Capabilities { pure, pointer_inert }`
  (ADR 0001). A pure element's layout and paint depend only on what
  `same_props` compares, the inherited text style, its children and its size.
- **Reconcile.** `elements::element::reconcile` pairs a rebuilt pure element
  with the previous one at the same position. It keeps the previous instance,
  with its layout and paint, when the props and all the children match, and
  otherwise keeps the children that do.
- **Layout keep.** A pure subtree records the range of layout nodes it owns. The
  next retained pass hands those nodes back without visiting the subtree, as
  long as the inherited text style is the same.
- **Paint replay.** A pure subtree records its range of primitives. The next
  frame moves that range out of the previous scene, translated, when the
  subtree moved by whole pixels and is otherwise unchanged. Subtrees outside the
  viewport and clip are not painted at all.
- **Pointer inertness.** Subtrees without pointer handlers are skipped when
  moves, releases and scrolls are dispatched.
- **Damage.** The scene knows which primitives are new. A window draws into a
  `RetainedTarget` kept across frames: only the primitives that touch the
  damage are drawn again, scissored to it, and a render pass copies the result
  to the drawable.

- **Keyed pairing.** Children of pure elements are paired in place while
  their keys agree. From the first mismatch on they are paired by key: an
  explicit id, or the entity a view shows (`element/reconcile.rs`). A list that
  moved keeps its rows.
- **Bounded reach.** Every primitive declares how far it can draw
  (`Reach::{Nothing, Within, Unbounded}`). Images reach only their bounds, so an
  image changes the damage only where it is, and is culled outside the
  viewport. Clips stay unbounded.

Rows as views (`cx.rows`) are the fast path for lists. Rebuilding from the root
stays correct through reconcile, but its cost grows with the size of the tree
(ADR 0001).

## Interaction

- **Clipboard.** `cx.write_to_clipboard` and `cx.read_from_clipboard` reach the
  app clipboard (`core::app::clipboard`): in memory by default, so headless
  runs are deterministic, and the general pasteboard on macOS. Copy, Cut and
  Paste are standard actions bound to Cmd+C/X/V; `Input` and `TextArea` handle
  them through the event context.
- **Drag and drop.** `Div::on_drag` makes a typed payload and a preview,
  `on_drop::<T>` and `on_drag_over::<T>` receive it, and `on_drop_files` takes
  files dropped from Finder (`core::drag`, `core::presenter::drag`,
  `platform::mac::file_drop`). A drag remembers its targets by stable identity,
  so a target rebuilt mid-drag still hears that the drag left.
- **Menus.** `cx.set_menus` declares the menu bar and `cx.show_context_menu`
  opens a context menu (`core::menu`). Choosing an item routes its action as a
  key binding would: to the focused element, then the app's handlers. Each item
  shows the chord the keymap binds to its action. `platform::mac::menus` builds
  the `NSMenu`s.

## Assets

- **Images.** `cx.image(asset)` takes a path or encoded bytes and decodes them
  off the UI thread with `background_spawn` (`core::app::images`). Until then
  the image shows its placeholder; a failed decode is an explicit error state.
  When the decode lands, only the view that asked is notified. Decoded images
  are kept while shown and otherwise within a byte budget
  (`set_image_budget`). `Image` is pure, so an unchanged image replays its
  paint.
- **SVG icons.** `svg(markup)` parses a practical subset of SVG once
  (`elements::svg`) and turns it into filled path primitives per size and tint.
  Unsupported features fail with a message that names the element, shown in
  place of the icon.

## Renderer

- **Text.** `TextMeasureCache` measures, shapes (rustybuzz, with a shape plan
  kept per font) and wraps text. It falls back per script run, and keeps results
  in frame-aware LRUs. `TextGlyphCache` rasterizes glyphs once at device scale
  into A8 atlas pages, plus RGBA pages for color emoji. Pages are evicted whole,
  and only once the GPU has finished every frame that drew from them.
- **Metal.** A frame is planned before anything is encoded. The scene becomes
  instance data in one pooled buffer, plus a plan of draws. Quads, shadows,
  glyphs and paths share one sprite pipeline. Draws are ordered by overlap, so a
  frame is usually one draw call. Paths are drawn as analytic tiles, or from an
  atlas when they are small shapes drawn before. Images have their own pipeline.
  The sprite shader skips the per-fragment clip test for draws wholly inside
  their clip, and plain quads take a specialized path. The renderer is created
  off the main thread (`MetalRenderer::spawn`). `renderer.rs` holds
  `MetalRenderer`; planning, encoding, resources and offscreen frames are its
  child modules.
- **Offscreen.** Captures and offscreen frames, used by tests and benchmarks,
  render into a kept texture through the same planning and encoding code.

## Platform (macOS)

`platform::mac` runs the app:

- The runner (`run_app`) opens windows and runs the event loop.
- Each window has a driver that turns AppKit events into platform events, runs
  the frame pipeline, and presents with the Core Animation transaction.
- Window events are held until the next display tick, so input coalesces into
  one frame.
- Animation frames come from a display link while one is requested, or from a
  timer when no display is awake (`frame_ticker`). An idle window gets no ticks.
- IME composition, the accessibility bridge (`NSAccessibility`), the pasteboard,
  file drops and native menus also live here.
- `app.rs` holds the runner. Its child modules are `window_driver` (one window's
  driver), `event_loop` (held events, idle work, viewport sync, redraw
  scheduling) and `dogfood_automation` (scripted input for native dogfood
  runs).

Other platforms fail with explicit unsupported errors until a backend
implements the window contract.

## Devtools

`RUI_DEVTOOLS=1` (or `cx.toggle_profiler()`) draws a profiling overlay after
paint, outside layout: stage times, frame interval, GPU time, draw calls,
damage share, replayed primitives and, with the opt-in `CountingAllocator`,
allocations. `RUI_DEVTOOLS=damage` (or `cx.toggle_damage_visualizer()`) tints
replayed subtrees green, outlines repainted ones red and outlines the frame's
damage in magenta. Both are off unless asked for, and the overlay is left out of
the damage it reports. See [`devtools.md`](devtools.md).

## Testing and gates

- **Headless.** `testing` mounts views and drives the same `FramePipeline`
  without a window. It captures primitives, accessibility trees and, with Metal,
  pixels.
- **Oracles.** These compare the incremental paths with full recomputation over
  random edits:
  - `tests/retained_layout.rs`
  - `tests/incremental_frames.rs`
  - `tests/damage_frames.rs`, pixel-exact
  - `tests/child_views.rs`
  - `tests/pointer_inert.rs`
  - `tests/stable_identity.rs`
- **Replay check.** `RUI_VERIFY_REPLAY=1 cargo test` repaints every painted
  frame in full and panics at the first primitive that differs from the replay.
- **Performance gate.** `cargo bench --bench perf_gate` gates instructions,
  allocations, footprint, damage area and virtual-list scaling against
  `benchmarks/perf_budgets/`. `PERF_GATE_PIXELS=1` also checks a pixel checksum.
  CI runs it without the instruction counter. See
  [`performance-benchmarks.md`](performance-benchmarks.md).
- **Comparison.** `benchmarks/gpui_compare/` runs the GPUI fixtures; the results
  are in [`gpui-comparison.md`](gpui-comparison.md).
- **Dogfood.** `scripts/native_dogfood_macos.sh` drives the native example with
  scripted input and checks its profile output.
