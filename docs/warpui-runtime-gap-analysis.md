# WarpUI-Aligned Runtime Gap Analysis

Date: 2026-06-02

This document compares RUI against the public evidence available for Warp's
WarpUI crates and product architecture. It is not a claim that RUI should copy
Warp's private implementation. The purpose is to make the current distance
explicit enough that future work can be split into reviewable, testable issues.

## Sources

Local RUI evidence:

- `README.md` describes RUI as early-stage, macOS-only, Metal-backed, and not
  yet complete for accessibility or advanced interactions.
- `src/core/app.rs` owns `AppContext`, entities, pending updates, viewport
  state, and platform dispatch.
- `src/elements/element.rs` maps RUI style into Taffy layout and exposes
  `LayoutContext`.
- `src/platform/mac/app.rs` runs the current macOS event/render loop.
- `src/platform/mac/window.rs` owns AppKit event polling and Metal-layer
  attachment.
- `src/renderer/scene.rs`, `src/renderer/primitives.rs`, and
  `src/renderer/metal/renderer.rs` define the scene primitive and Metal
  renderer path.
- `src/core/accessibility` and `src/platform/mac/accessibility.rs` define
  accessibility data and the currently unattached macOS bridge.
- `tests/*`, `benches/*`, and `docs/advanced-ui-runtime-roadmap.md` define the
  existing verification posture.

Public Warp/WarpUI evidence:

- `warpdotdev/warp` publishes `crates/warpui_core` and `crates/warpui`.
- `crates/warpui_core/README.md` describes a global `App` that owns views and
  models as entities, handle-based entity references, render-time app context,
  and view-rendered elements.
- `crates/warpui_core/src/lib.rs` publicly exposes modules including
  `accessibility`, `actions`, `clipboard`, `fonts`, `image_cache`, `keymap`,
  `modals`, `presenter`, `rendering`, `scene`, `text`, `ui_components`,
  `windowing`, and `zoom`.
- Warp's public architecture writing describes GPU rendering through primitive
  scene concepts and a UI framework loosely inspired by Flutter.
- Warp's public product docs describe terminal Blocks, agent context, and Warp
  Drive. Those are useful product anchors for an agentic terminal/ADE, but they
  should not be treated as required RUI scope unless RUI chooses that domain.

## Short Answer

Yes, RUI is still far from WarpUI as a mature application runtime.

RUI has a meaningful foundation: native Rust app state, Taffy layout,
scene-based rendering, AppKit/Metal runtime, text shaping, input editing,
accessibility data, action/keymap modules, examples, headless tests, renderer
resource tests, and recent frame profiling.

The gap is not one missing widget. The remaining distance is product-runtime
work: ownership and view lifecycle, platform lifecycle, multi-window,
responsive layout, persistent layout/diffing, renderer batching and resource
limits, action/focus integration, full IME and text field APIs, native
accessibility bridge, themeable component contracts, virtualized data
components, live dogfood, and docs/API synchronization.

## Current RUI Baseline

RUI is best described as an advanced pre-1.0 native UI framework foundation.
The current architecture is:

- Application layer: `App::new().run(...)` builds a root element tree through
  `AppContext`.
- State layer: `AppContext` owns entity storage, pending updates, dirty/rebuild
  flags, viewport size, and runtime view notification.
- Element layer: elements implement layout, paint, event handling, and optional
  accessibility nodes.
- Layout layer: each render frame builds a Taffy tree from element layout.
- Scene layer: paint emits primitives and hit regions into `Scene`.
- Renderer layer: Metal renders primitives on macOS.
- Platform layer: AppKit events are polled, translated, and dispatched to the
  element tree.
- Testing layer: headless layout/event tests, primitive snapshots, accessibility
  tests, renderer resource tests, example smoke tests, and benchmark config
  tests.

Recent fixes improved fundamentals:

- AppKit key events are translated into RUI input without feeding handled key
  events back into AppKit.
- Minimize/reopen behavior has a platform-level path.
- `RUI_PROFILE=1` exposes frame-stage timing.
- Text measurement cache and system font loading are reused across frames,
  reducing RTodo input-frame layout from roughly 110ms to about 0.05ms in the
  measured release profile.

## Confirmed Gaps

### 1. Runtime Ownership And View Handles

RUI has an `AppContext` and entity storage, but it does not yet expose the same
kind of mature handle/context model that WarpUI documents publicly.

Current evidence:

- `AppContext` owns entities, windows, pending updates, viewport state, dirty
  flags, and notification.
- `View` and runtime-view notification exist, but the runtime primarily drives a
  single root built by a closure.
- Existing stateful advanced controls also own internal component state.

Gap:

- No clearly documented controlled/uncontrolled component convention.
- No public view/model handle model that explains when long-lived entities can
  reference each other and when context-bound access is valid.
- No runtime-level action/focus integration with the existing `ActionRouter`.

Impact:

- Larger applications will drift into ad hoc state ownership patterns.
- Component state can diverge from app state.
- Multi-pane or multi-document UI patterns are hard to model cleanly.

### 2. Platform Scope And Multi-Window

RUI is macOS-only in practice.

Current evidence:

- README documents macOS as the only supported runtime.
- `App::run` dispatches to `platform::mac::run_app` and panics on non-macOS.
- `AppContext` has window concepts, but the active macOS runtime creates and
  drives one platform window.

Gap:

- No supported Windows/Linux/web backend.
- Multi-window is partially modeled but not implemented end to end.
- Platform lifecycle is still mostly inferred through polling and visibility
  state.

Impact:

- The framework cannot claim cross-platform runtime parity.
- AppKit edge cases such as close/minimize/reopen/focus/Cmd+Q need more native
  delegate coverage and live dogfood.

### 3. Responsive Layout And Dimension Semantics

RUI layout is still pixel-first.

Current evidence:

- Style dimensions are mostly `Option<f32>` mapped to Taffy length or auto.
- `w_full()` uses an `f32::INFINITY` sentinel.
- Examples and dogfood surfaces still commonly use fixed window sizes.

Gap:

- No first-class dimension type for `px`, percent, fill, auto, min/max content,
  viewport-relative sizing, or breakpoint-driven behavior.
- Layout is full-tree and frame-local; the Taffy tree is cleared and rebuilt
  every render frame.

Impact:

- Apps can break under resize, zoom, localization, and density changes.
- Avoiding overlap/clipping relies too much on manual app code.
- Full-tree relayout limits scaling for complex apps.

### 4. Renderer Pipeline And Resource Scaling

RUI has a useful scene primitive model, but the Metal path is immediate and
mostly unbatched.

Current evidence:

- `Scene` stores primitives, layers, clips, and hit regions.
- Metal iterates primitives and issues draw work per primitive.
- Renderer docs already list batching and persistent layout nodes as future
  optimizations.
- Production `MetalRenderer::new()` uses unbounded resource caches.
- Resource pressure behavior is tested at the generic cache layer, not as a
  full Metal render-after-eviction path.

Gap:

- No batching/material sorting contract.
- No stable allocation counters in renderer diagnostics.
- No bounded production cache policy by default.
- No p95/p99 frame-budget enforcement for common scenes.

Impact:

- Rendering cost can scale linearly with primitive count and per-draw buffer
  allocation.
- Resource leaks or pressure regressions may only show up during manual use.

### 5. Events, Focus, Actions, And Keymaps

RUI has action/keymap infrastructure, but the platform loop still dispatches raw
key events directly into elements.

Current evidence:

- `Keymap` and `ActionRouter` exist and are tested.
- The macOS runtime forwards key-down events to the element tree.
- Pointer hit-testing and pointer capture are scene-based.

Gap:

- Runtime-level keymap/action dispatch is not the primary key path.
- Focus traversal and action scope rules are not complete.
- Default command behavior for controls is not consistently expressed through
  typed actions.

Impact:

- App-level shortcuts, component shortcuts, focus routing, and command palettes
  will become inconsistent unless the runtime owns this contract.

### 6. Text, Input, And IME

RUI has strong text-editing internals but incomplete high-level text field
contracts.

Current evidence:

- `TextEditBuffer` supports selection, cursor movement, composition state,
  clipboard, submit, cancel, and multiline internals.
- `Input` exposes a single-line field API and does not expose a textarea-style
  builder.
- Platform IME currently exposes commit events, not a full begin/update/cancel
  marked-text lifecycle.
- Some controls still estimate text size by character count.

Gap:

- No first-class `TextField`/`TextArea` advanced components.
- No full IME composition route from AppKit through platform events to input
  controls.
- Caret/selection geometry is not consistently based on shaped text in every
  control.

Impact:

- Real text entry, CJK IME, multiline editing, keyboard selection, password
  fields, and validation UX remain below production expectations.

### 7. Native Accessibility Bridge

RUI has semantic accessibility data, but native macOS accessibility is not
attached.

Current evidence:

- Accessibility roles, nodes, actions, validation, and announcements exist.
- Tests cover semantic output and unsupported bridge errors.
- `MacAccessibilityBridge` returns an explicit bridge failure because no native
  AppKit host is attached.

Gap:

- No native AX host tree.
- No platform action routing from accessibility to elements.
- No focus/announcement synchronization with AppKit.

Impact:

- RUI can test semantics but cannot yet claim production native accessibility.

### 8. Theme And Design-System Contracts

Advanced UI tokens are hard-coded.

Current evidence:

- `advanced_ui::tokens` defines fixed constants and fixed color functions.
- Advanced controls call token functions directly.
- The advanced component set is limited to layout/text wrappers and a handful
  of controls.

Gap:

- No app-owned `Theme`.
- No light/dark/high-contrast token sets.
- No consistent density, size, invalid, disabled, read-only, loading, or error
  state contract across all advanced components.

Impact:

- Apps cannot build a coherent branded or accessibility-aware design system on
  top of RUI without forking control internals.

### 9. Data Components, Virtualization, And Structured Output

RUI has list/table primitives but not app-scale data components.

Current evidence:

- `ScrollView` owns internal scroll state and paints scrollbars.
- `Table` is string-cell oriented and lacks element cells, sorting, selection,
  resizing, virtualization, and table accessibility semantics.
- `List` paints markers and children but has no stable item identity or
  navigation contract.

Warp product caveat:

- Warp's terminal Blocks are a product-level model, not automatically a RUI
  requirement.

Gap:

- No virtualized list/table/tree.
- No structured row/item identity contract.
- No block-like structured scroll-stream abstraction for agentic or terminal
  UIs, if RUI chooses to target that domain.

Impact:

- Large app surfaces will hit layout/render scale limits and inconsistent
  keyboard/accessibility behavior.

### 10. Testing, Profiling, And Live Dogfood

RUI has many good automated tests but lacks live runtime coverage.

Current evidence:

- Headless, snapshot, accessibility, platform boundary, renderer resource,
  benchmark config, and example smoke tests exist.
- README explicitly says CI does not cover full visual QA.
- `RUI_PROFILE` prints stderr summaries, but structured performance artifacts
  and threshold enforcement are not present.
- Benchmark enforcement is disabled pending calibration.

Gap:

- No finite macOS dogfood that launches a real AppKit app, types text,
  minimizes/reopens, captures profiler data, and exits cleanly.
- No structured frame telemetry with event-to-render latency, frame intervals,
  jank counts, p95/p99, and drawable wait classification.
- No CI policy for docs/API drift.

Impact:

- Regressions like input lag, minimize/reopen, frame pacing, native
  accessibility, and renderer pressure can escape unit tests.

### 11. Documentation And API Drift

Public docs have already drifted from source.

Current evidence:

- API docs list some enum variants that do not match source names.
- Input docs describe builders that do not match the current public builder
  surface.
- Existing docs are useful but not compile-checked.

Gap:

- No generated or compile-checked API reference.
- Raw elements and advanced UI components are not clearly separated for users.

Impact:

- Users may build against non-existent APIs, and contributor work may target
  stale contracts.

## Existing Issue Overlap

The current GitHub issue tracker already contains important child work:

- #40 tracks native accessibility bridge and semantic coverage.
- #44 tracks expanded advanced component set.
- #79 tracks TextField wrapper.
- #80 tracks Tabs.
- #81 tracks data list/tree/table row primitives.
- #82 tracks Menu and Popover/Dialog primitives.
- #83 tracks ScrollView accessibility and file split.

This analysis should not duplicate those issues. The missing piece is a
top-level runtime alignment issue that connects them to the broader WarpUI
comparison and adds the platform, performance, responsive layout, action/focus,
theme, virtualization, dogfood, and docs-sync workstreams.

## Non-Goals

- Do not vendor WarpUI or copy Warp source into RUI.
- Do not claim WarpUI parity from visual similarity or a single dogfood app.
- Do not treat Warp's terminal Blocks or agent-product features as mandatory
  RUI scope unless RUI explicitly targets terminal/ADE applications.
- Do not paper over missing runtime features with RTodo-specific or
  app-specific workarounds.
- Do not mark platform or accessibility paths complete until native behavior is
  wired and freshly verified.

## Recommended Next Step

Create a top-level GitHub issue for "WarpUI-aligned runtime gaps" and link it
to this document plus `docs/warpui-runtime-implementation-spec.md`. Keep
existing component/accessibility issues as children. Use fresh `cargo check`,
`cargo test`, targeted benchmarks, and live macOS dogfood as gates before
closing runtime readiness claims.
