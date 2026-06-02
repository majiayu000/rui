# WarpUI-Aligned Runtime Implementation Spec

Date: 2026-06-02

This spec turns `docs/warpui-runtime-gap-analysis.md` into a staged execution
plan. It uses public WarpUI concepts as comparison anchors, but keeps RUI's
implementation native to this repository.

## Goal

Evolve RUI from an advanced pre-1.0 macOS UI foundation into a production-grade
application runtime with explicit contracts for state, layout, rendering,
platform lifecycle, accessibility, components, performance, and verification.

## Non-Goals

- Do not import WarpUI as a dependency or copy its implementation.
- Do not broaden scope to terminal Blocks or ADE features unless a separate
  product decision makes that RUI's target domain.
- Do not add new public APIs without tests and at least one real caller or
  example.
- Do not claim cross-platform, accessibility, or performance readiness without
  fresh verification.

## Architecture Contract

RUI should preserve its existing layering while tightening the contracts:

| Layer | Target Contract |
| --- | --- |
| App/runtime | Long-lived app state, view/model ownership, handle/context access rules, multi-window lifecycle, notifier-driven rebuilds. |
| Elements | Deterministic layout, paint, event, action, focus, and accessibility behavior. |
| Layout | First-class dimensions, responsive constraints, persistent or diffable layout state where needed, resize-safe surfaces. |
| Renderer | Scene primitives, batching, bounded resources, renderer diagnostics, explicit errors. |
| Platform | Native lifecycle, input, IME, focus, DPI, clipboard, accessibility host, renderer attachment. |
| Actions | Typed keymaps/actions routed through focus, component, and app scopes. |
| Text | TextField/TextArea APIs, shaped caret/selection geometry, full IME lifecycle, clipboard errors. |
| Advanced UI | Themeable components with shared size, state, validation, accessibility, and keyboard contracts. |
| Testing | Headless contracts, primitive snapshots, native smoke, structured profiling, benchmark gates, docs/API sync. |

## Workstreams

### 1. Runtime Ownership, View Handles, And State Contracts

Scope:

- Document and implement the canonical app/view/model ownership model.
- Define when app state lives in `AppContext` entities, component state, or
  caller-owned controlled props.
- Add public handle/context APIs only where a real view/model relationship needs
  them.
- Define controlled vs uncontrolled behavior for every stateful advanced
  component.

Acceptance criteria:

- Runtime docs explain state ownership and view/model references.
- Stateful controls document controlled/uncontrolled modes.
- Tests prove external state updates propagate through rebuilds.
- Tests prove internal state cannot mutate disabled/read-only controls.

Validation:

- `cargo test headless`
- `cargo test advanced_ui`
- `cargo test action_keymap`
- `cargo test`

### 2. Platform Lifecycle, Redraw, And Multi-Window

Scope:

- Replace close/minimize/focus inference with native delegate-backed lifecycle
  events where AppKit provides them.
- Wire `MacWindow::request_redraw()` into redraw scheduling instead of using
  dirty flags alone.
- Add redraw-source counters and coalescing behavior.
- Define single-window vs multi-window runtime contracts.
- Add a finite macOS dogfood smoke for type/minimize/reopen/Cmd+Q.

Acceptance criteria:

- Minimize, reopen, activation, focus, close, resize, and Cmd+Q have native or
  explicitly documented event paths.
- Repeated redraw requests coalesce without losing final paint.
- Multi-window APIs either work end to end or return explicit unsupported
  errors.
- Native dogfood captures `RUI_PROFILE` and exits cleanly.

Validation:

- `cargo test platform_window`
- `cargo test headless`
- finite macOS native dogfood command
- `cargo test`

### 3. Responsive Layout And Persistent Layout Scaling

Scope:

- Replace raw `Option<f32>`/sentinel sizing with a structured dimension API.
- Support at least `px`, percent, auto, fill, min/max constraints, and
  viewport-aware layout helpers.
- Add resize-safe tests for small, medium, and large viewports.
- Investigate persistent Taffy nodes or layout diffing for complex trees.

Acceptance criteria:

- `w_full()` no longer depends on an infinity sentinel.
- Examples and dogfood surfaces avoid fixed-only root layouts.
- Headless resize tests prove no primary controls overlap or clip.
- Benchmark cases track layout cost for large trees before and after persistent
  layout work.

Validation:

- `cargo test headless`
- `cargo test advanced_ui_layout_tests`
- `cargo test benchmark_config`
- `cargo bench --bench runtime_baselines` after calibration

### 4. Renderer Batching, Resources, And Frame Telemetry

Scope:

- Add renderer diagnostics for draw count, buffer allocations, texture/cache
  hits, evictions, drawable wait, and render time.
- Introduce configurable resource cache limits for production renderer paths.
- Add Metal render-after-eviction tests.
- Batch or reuse buffers where primitive count currently causes linear
  allocation growth.
- Convert `RUI_PROFILE` from stderr-only summaries into structured optional
  output.

Acceptance criteria:

- Renderer diagnostics expose allocation and resource behavior.
- Repeated image/text frames stay bounded under configured cache limits.
- Benchmarks detect primitive-count allocation regressions.
- Frame telemetry includes frame interval, event-to-render latency, jank count,
  p95/p99, drawable wait, layout, dispatch, paint, and render.

Validation:

- `cargo test renderer_resource`
- `cargo test renderer_backend`
- `cargo test benchmark_config`
- structured `RUI_PROFILE` native smoke
- `cargo bench --bench runtime_baselines` after CI calibration

### 5. Actions, Focus, Keyboard, And Command Routing

Scope:

- Make `Keymap` and `ActionRouter` part of the runtime key path.
- Define focus traversal, focused component scope, app scope, and ignored vs
  handled action results.
- Add default action contracts for text input, buttons, tabs, menus, dialogs,
  scroll views, and data components.

Acceptance criteria:

- Runtime key dispatch can route typed actions before or alongside raw key
  events.
- App-level shortcuts do not bypass focused component handlers incorrectly.
- Components expose default key bindings as typed actions.
- Disabled/read-only components ignore activation actions explicitly.

Validation:

- `cargo test action_keymap`
- `cargo test event_targeting`
- `cargo test advanced_ui`
- `cargo test text_editing`

### 6. TextField, TextArea, And Full IME

Scope:

- Add advanced `TextField` and `TextArea` wrappers over core text editing.
- Expose full IME lifecycle: begin/update/cancel/commit marked text.
- Use shaped text geometry for caret, selection, and horizontal/vertical
  movement.
- Add password, validation, placeholder, disabled, read-only, clipboard, and
  accessibility semantics.

Acceptance criteria:

- Single-line and multiline editing are first-class public APIs.
- IME composition is represented from platform event through input state and
  paint.
- Text selection and caret geometry follow grapheme clusters and shaped runs.
- TextField/TextArea expose complete accessibility value, selection,
  composition, disabled/read-only, and error semantics.

Validation:

- `cargo test text_editing`
- `cargo test accessibility`
- `cargo test advanced_ui`
- native macOS IME smoke
- `cargo test`

### 7. Native Accessibility Bridge

Scope:

- Attach RUI accessibility nodes to a native macOS accessibility host.
- Map roles, labels, values, state, focus, selection, scroll position, actions,
  and announcements.
- Keep unsupported native features as explicit errors.
- Complete existing child work such as #40 and #83.

Acceptance criteria:

- Native AX tree can be inspected for a running macOS example.
- Accessibility actions route back to RUI elements.
- Focus and announcements sync with native host behavior.
- ScrollView exposes position and max values.

Validation:

- `cargo test accessibility`
- native macOS accessibility smoke
- `cargo test advanced_ui`
- `cargo test headless`

### 8. Theme And Advanced Component System

Scope:

- Introduce app-owned theme/tokens with light, dark, and high-contrast sets.
- Make advanced controls consume theme tokens rather than fixed color
  functions.
- Complete the component inventory tracked by #44 and child issues #79-#82.
- Add shared state contracts for size, density, invalid, disabled, read-only,
  loading, error, focus, hover, press, and selected states.

Acceptance criteria:

- Theme changes can rebuild controls without app-specific widget forks.
- Advanced controls share sizing, state, validation, action, and accessibility
  contracts.
- Raw elements and advanced components are documented separately.
- Examples demonstrate real composition, not placeholder-only surfaces.

Validation:

- `cargo test advanced_ui`
- `cargo test accessibility`
- `cargo test action_keymap`
- `cargo test primitive_snapshot`
- `cargo test example_smoke`

### 9. Data Components, Virtualization, And Structured Scroll Surfaces

Scope:

- Add virtualized list, table, and tree primitives with stable item identity.
- Add table row/cell element support, selection, sorting, resizing, and
  keyboard navigation.
- Add scroll commands, paging, scrollbar drag, and programmatic scroll APIs.
- Evaluate whether a block-like structured scroll stream belongs in RUI's
  scope for terminal/ADE applications.

Acceptance criteria:

- Large data sets render visible ranges without full item layout/paint.
- Selection and keyboard navigation are stable under insert/delete/reorder.
- Table/list/tree expose appropriate accessibility roles and values.
- Structured block work is either explicitly scoped in or documented as a
  non-goal.

Validation:

- `cargo test headless`
- `cargo test accessibility`
- `cargo test event_targeting`
- runtime virtualization benchmarks

### 10. Docs, Examples, And API Sync

Scope:

- Split documentation for raw elements, advanced UI, runtime/platform, and
  testing.
- Make docs examples compile-checked or explicitly marked conceptual.
- Add CI or a script to detect docs/API drift for public builders and enum
  variants.
- Update README readiness language only after verified gates land.

Acceptance criteria:

- API docs match current source names and builder methods.
- Docs distinguish what is implemented from roadmap work.
- Examples are finite by default and safe for CI.
- Issue and PR descriptions cite fresh validation, not stale previous runs.

Validation:

- `cargo test example_smoke`
- docs/API drift script or generated docs check
- `cargo check`
- `cargo test`

## Rollout Order

1. Documentation and issue hygiene
   - Land gap analysis and this spec.
   - Open a top-level tracker issue.
   - Link existing child issues instead of duplicating them.

2. Runtime observability
   - Make frame profile output structured.
   - Add redraw-source and renderer allocation counters.
   - Add native macOS dogfood smoke.

3. Platform and action contract
   - Delegate-backed lifecycle.
   - Redraw coalescing.
   - Runtime keymap/action/focus routing.

4. Layout and theme contract
   - Structured dimensions.
   - Responsive examples/tests.
   - App-owned theme tokens.

5. Text, accessibility, and advanced controls
   - TextField/TextArea.
   - Full IME.
   - Native accessibility bridge.
   - Complete #44 child controls.

6. Renderer scale and data surfaces
   - Batching/resource limits.
   - Virtualized list/table/tree.
   - Benchmark enforcement after calibration.

7. Cross-platform expansion
   - Only after macOS contracts are verified and platform boundary tests can be
     shared across backends.

## Validation Matrix

| Claim | Required proof |
| --- | --- |
| Build health | `cargo check` |
| Full regression safety | `cargo test` |
| Platform lifecycle | `cargo test platform_window` plus native macOS smoke |
| Layout/event contract | `cargo test headless`, `cargo test event_targeting` |
| Advanced controls | `cargo test advanced_ui`, `cargo test accessibility` |
| Text/IME | `cargo test text_editing` plus native IME smoke |
| Accessibility bridge | `cargo test accessibility` plus native AX inspection |
| Renderer resources | `cargo test renderer_resource`, `cargo test renderer_backend` |
| Snapshots/examples | `cargo test primitive_snapshot`, `cargo test example_smoke` |
| Performance | `cargo test benchmark_config`, structured `RUI_PROFILE`, calibrated runtime benches |
| Docs/API sync | compile-checked examples or docs drift script |

## Issue Mapping

Existing issues:

- #40: accessibility bridge and semantic coverage.
- #44: advanced component parent tracker.
- #79: TextField wrapper.
- #80: Tabs.
- #81: data list/tree/table row primitives.
- #82: Menu and Popover/Dialog primitives.
- #83: ScrollView accessibility semantics.

New top-level issue should track the cross-cutting runtime plan:

- runtime ownership/state model
- platform lifecycle/redraw/multi-window
- responsive layout/dimensions
- renderer telemetry/resources/batching
- actions/focus integration
- full IME/text APIs
- theme/design-system contract
- data virtualization
- live dogfood and docs sync
