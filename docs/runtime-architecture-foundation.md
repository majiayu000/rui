# RUI Runtime Architecture Foundation

This document is the source-of-truth for RUI's runtime architecture direction.
It captures the boundary design that future runtime, platform, renderer, and
testing work should preserve.

## Decision

RUI should remain a native Rust UI runtime with app-owned state, a long-lived
root view, transient element trees, backend-neutral scenes, native platform
adapters, and renderer adapters.

The architecture is already pointing in the right direction. The next
architecture action is not to replace it with Clean Architecture, a virtual DOM,
or a full ECS. The missing boundary is an explicit per-window `Presenter` plus a
shared `FramePipeline` that both native and headless runners use.

The chosen shape is:

- `AppContext` is the framework state root.
- `RuntimeView<V>` owns the long-lived root `View` value.
- `View::render` builds a fresh element tree through `ViewContext`.
- `Element` instances are lifecycle objects for the current tree and frame.
- `Scene` is the per-frame primitive output and hit-test surface.
- `Renderer` consumes `Scene + target + viewport_size` and stays native-window
  free.
- Platform code owns native windows, native event streams, renderer
  attachments, redraw wakeups, clipboard, DPI, focus, and lifecycle signals.
- The future `Presenter` owns per-window UI runtime state.
- The future `FramePipeline` owns the shared frame order and state transitions.

## Document Source Of Truth

| Document | Owns | Status |
| --- | --- | --- |
| `docs/runtime-architecture-foundation.md` | Runtime architecture direction, boundaries, migration order, and source-of-truth map. | Canonical foundation |
| `docs/ARCHITECTURE.md` | Current architecture overview, existing layer responsibilities, render scheduling, and module map. | Current overview |
| `docs/API.md` | Drift-checked public API section and user-facing API reference. | Public API truth where covered by `docs_api_drift` |
| `docs/platform-window-boundary.md` | Platform window feature contract, renderer attachment ownership, and unsupported-platform behavior. | Platform boundary truth |
| `docs/advanced-ui-runtime-roadmap.md` | Runtime maturity gates and workstream ordering. | Roadmap |
| `docs/warpui-runtime-gap-analysis.md` | Comparison with public WarpUI signals and readiness gaps. | Planning and comparison |
| `docs/warpui-runtime-implementation-spec.md` | Coordination notes for WarpUI-aligned runtime work. | Planning |
| `docs/performance-benchmarks.md` | Benchmark categories, baseline maintenance, and threshold policy. | Benchmark policy |
| `docs/design-review.md` | Historical review notes. | Advisory, may be stale |

WarpUI remains a useful comparison anchor, not RUI's source of truth. RUI should
borrow runtime principles only when they improve its own contracts.

## Current Ownership Model

| Surface | Current Owner | Contract |
| --- | --- | --- |
| `AppContext` | Framework runtime | Owns `EntityStore`, windows, pending updates, dirty/rebuild flags, redraw source counters, keymap, app action handlers, and the root runtime view notifier. |
| `EntityStore` | `AppContext` | Stores type-erased state. `Entity<T>` is an opaque, context-bound handle; type validity is proven only by `AppContext::get` and `AppContext::get_mut`. |
| `RuntimeView<V>` | App runner or headless mount | Owns the long-lived `View` value. The actual `V` value is not stored in `EntityStore`; a marker entity gives the view a stable identity. |
| `ViewContext` | Render phase | Grants temporary access to `AppContext`, current view identity, and `ViewNotifier` during `View::render`. |
| `ViewNotifier` | Callback-safe scheduling handle | Schedules the owning view to rebuild on the next runtime pass. It does not expose or mutate app state directly. |
| `Element` tree | Current rebuild | Owns layout, paint, event, accessibility, and child traversal behavior for the current tree. Persistent data must live in caller state or `AppContext` entities. |
| `EventContext` | Event dispatch phase | Carries focus, hit target, previous hit target, cursor, accessibility announcements, and redraw requests. It does not carry `AppContext`. |
| `Scene` | Current or last painted frame | Records primitives, render layers, hit regions, and draw order. It is backend-neutral and renderer-independent. |
| `Renderer` | Rendering backend | Renders a `Scene` into a platform-provided target and returns explicit renderer errors or diagnostics. |
| Platform window | Platform backend | Owns native APIs, event streams, lifecycle, clipboard, DPI, focus, renderer attachment, and drawable targets. |
| Headless session | Testing runtime | Mirrors runtime state with a recording renderer and should exercise the same frame contract as native runners. |

## Target Boundaries

### App Runtime

`AppContext` remains the framework state root. It owns framework state, not every
piece of application data. Caller-owned state is valid when callbacks mutate it
and then notify the runtime to rebuild.

Rules:

- `notify` and `ViewNotifier::notify` schedule rebuild work; they do not
  synchronously rebuild.
- `request_rebuild` rebuilds the element tree on the next frame.
- `request_redraw` re-lays out and repaints the current tree without requiring a
  new `View::render` pass.
- `complete_redraw_frame` may clear dirty state only when no rebuild or pending
  update remains.
- Borrowed `Ref` and `RefMut` values from entities must not escape the current
  render or callback scope.

### Presenter

`Presenter` should be the per-window runtime object that owns UI state needed to
present one window.

It should own:

- `AppContext`
- root builder or `RuntimeView`
- current root element
- `taffy::TaffyTree`
- root bounds
- current or last completed `Scene`
- focused element
- pointer capture target
- last pointer hit target
- frame timing and redraw diagnostics

It should not own:

- native `NSWindow`, `CAMetalLayer`, or platform-specific window handles
- renderer backend resources
- global process lifecycle beyond the window it presents
- hidden fallback data for unsupported platform features

### FramePipeline

`FramePipeline` should be the shared state machine used by native and headless
drivers. The macOS runner and `testing::HeadlessSession` currently duplicate
this ordering; future work should extract the common contract instead of adding
another runner-specific copy.

Canonical frame order:

1. Drain or import platform/runtime events into backend-neutral events.
2. Consume runtime view notifications.
3. Rebuild the root element tree when `needs_rebuild` or pending updates exist.
4. Preserve notifications raised during render by setting rebuild state again.
5. Compute layout through Taffy.
6. Dispatch backend-neutral pointer, scroll, key, text, focus, resize, and close
   events against the current root element.
7. Use the last completed `Scene` for pointer hit testing until a dedicated
   pre-dispatch hit-region pass exists.
8. Paint the current root element into a cleared `Scene`.
9. Finish the scene.
10. Render through the active `Renderer` when a platform target is available.
11. Record diagnostics and complete the redraw frame.

Frame invariants:

- Native and headless frame behavior should be semantically identical for
  rebuild, layout, event dispatch, paint, hit testing, and redraw completion.
- Platform lifecycle events may request redraw, quit, or resize, but they should
  not own layout or paint logic.
- Renderer errors are explicit. Native apps may treat some renderer failures as
  fatal, while tests should surface typed errors such as `HeadlessError`.
- Close, quit, minimize, and redraw classification should be centralized so
  platform and app runners do not drift.

### Elements

Elements are not durable state stores. They are frame/tree lifecycle objects.
They own:

- layout node creation
- paint output
- input handlers
- accessibility extraction
- child traversal

They may request redraw or stop event propagation. Durable application state
belongs in caller-owned data or `AppContext` entities, then the runtime is
notified to rebuild when visible output changes.

### Renderer Adapter

The renderer contract should stay backend-independent:

```rust
Scene + Renderer::Target + viewport_size -> Result<(), RendererError>
```

Renderer modules own backend resources, primitive support validation,
diagnostics, resource pressure behavior, batching, and frame telemetry. They
must not import native window APIs directly.

### Platform Adapter

The platform adapter owns native behavior:

- window creation and close handling
- native lifecycle callbacks
- event polling or callback draining
- DPI and content size
- focus
- clipboard
- renderer attachment and drawable target acquisition
- redraw wakeups
- explicit unsupported errors for missing features

It must not own element layout, event propagation, scene painting, or renderer
resource policy.

## Migration Plan

### P0: Documentation Foundation

- Establish this document as the architecture source-of-truth.
- Align README and crate-level architecture language with the runtime boundary.
- Keep WarpUI docs framed as comparison and planning docs.

### P1: Shared FramePipeline

- Add a shared frame pipeline module for rebuild, layout, event dispatch, paint,
  scene finish, render, diagnostics, and redraw completion.
- Route headless tests through the shared pipeline first, because headless
  errors are typed and deterministic.
- Add focused tests for notification preservation, dirty clearing, redraw source
  classification, and pointer hit testing against the previous scene.

### P2: Presenter Extraction

- Move per-window state out of runner-local variables into a `Presenter`.
- Make native macOS and headless sessions use the same presenter-owned state.
- Keep platform adapters responsible for native windows and renderer targets.

### P3: Platform Runner Cleanup

- Replace duplicated native/headless event and redraw classification with shared
  frame inputs.
- Keep macOS-specific code in `src/platform/mac`.
- Return explicit unsupported errors for non-macOS paths until shared platform
  tests and real targets exist.

### P4: Readiness Gates

- Use `cargo check` and `cargo test` as umbrella gates.
- Keep `cargo test --test docs_api_drift` for public API documentation drift.
- Use focused commands from `docs/advanced-ui-runtime-roadmap.md` for affected
  workstreams.
- Treat benchmark enforcement as pending unless `docs/performance-benchmarks.md`
  thresholds are enabled and validated in CI.

## Non-Goals

- Do not replace RUI with WarpUI internals or module names.
- Do not introduce a virtual DOM unless a concrete RUI problem requires it.
- Do not convert `EntityStore` into a full ECS without a specific state
  ownership problem and tests.
- Do not make renderer code depend on native window APIs.
- Do not hide unsupported platform features behind placeholder success paths.
- Do not claim WarpUI equivalence or production readiness from visual parity.

## Acceptance Criteria For Runtime Architecture Work

Future architecture work should satisfy these checks:

- The source-of-truth document for the changed boundary is updated.
- Public API behavior is covered by `docs/API.md` only when drift tests verify it.
- Native and headless runners share behavior or document the reason they differ.
- Unsupported platform, renderer, accessibility, and data paths return explicit
  errors or diagnostics.
- Tests prove the smallest changed contract before readiness claims are made.
