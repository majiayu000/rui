# WarpUI-Aligned Runtime Implementation Spec

This spec turns the issue #87 gap analysis into implementation rules. It is a
coordination document for child issues rather than a request to copy WarpUI. RUI
should stay native to this repository and use public WarpUI signals only as a
runtime maturity benchmark.

## Design Principles

- Keep ownership explicit: the runtime should state whether data belongs to the
  app, a view/model handle, a caller-controlled value, or a component-local
  state cell.
- Keep failures visible: unsupported platform, renderer, accessibility, and
  data-surface paths should return errors or diagnostics instead of silently
  falling back.
- Keep public APIs justified: a new public API needs a caller, example, or test
  that proves the contract.
- Keep validation close to the behavior: unit, integration, headless, native,
  and benchmark checks should be selected by the changed boundary.

## Workstream Specs

### Runtime Ownership and State

Owner: #88

Define the runtime state model in terms of app-owned state, view/model handles,
caller-controlled component values, and component-local state. Document access
rules for render-time context and mutation paths. Stateful controls must specify
controlled and uncontrolled behavior, including disabled and read-only mutation
rules.

Required proof:

- External state updates trigger deterministic rebuilds.
- Component-local state survives rebuilds only where the contract allows it.
- Disabled and read-only controls reject internal mutation explicitly.

### macOS Lifecycle, Redraw, and Multi-Window

Owner: #89

Move platform behavior toward explicit lifecycle callbacks for close, minimize,
reopen, activation, focus, resize, and Cmd+Q. Redraw requests should have named
sources and coalescing behavior. Multi-window APIs must either work end to end
or return explicit unsupported errors.

Required proof:

- Lifecycle helpers are covered by tests.
- Repeated redraw requests coalesce without dropping the final paint.
- Unsupported multi-window behavior is explicit.

### Responsive Dimensions

Owner: #90

Introduce structured dimension values for pixels, percentages, auto, fill, and
min/max constraints. Style-to-layout conversion should preserve those semantics
directly rather than relying on raw float sentinels.

Required proof:

- Public sizing APIs avoid sentinel values.
- Headless resize tests cover small, medium, and large viewports.
- Example and dogfood roots avoid fixed-only assumptions.

### Renderer Telemetry and Resource Limits

Owner: #91

Make frame and resource behavior observable through structured telemetry. The
runtime should be able to report frame interval, event-to-render latency, jank
counts, layout/dispatch/paint/render timing, draw counts, buffer allocations,
cache hits, evictions, and resource pressure.

Required proof:

- `RUI_PROFILE` can emit structured data usable by tests or dogfood tooling.
- Repeated text and image scenes stay bounded under configured cache limits.
- Benchmarks can warn or fail when allocations scale with primitive count.

### Focus, Keymap, and Action Routing

Owner: #92

Route key events through focus, component, and app action scopes before falling
back to raw key delivery. Component handlers should run before app-level
fallbacks when focus semantics require it. Disabled and read-only controls must
ignore activation actions explicitly.

Required proof:

- Conflict detection remains deterministic.
- Focused component actions and app fallback actions are ordered by contract.
- Ignored actions produce a testable handled/ignored result.

### Text, IME, and TextArea

Owner: #93

Represent IME begin, update, cancel, and commit from platform events through
text editing state and paint output. Add multiline TextArea behavior with
selection, caret geometry, shaped runs, password/placeholder/validation states,
clipboard behavior, and accessibility semantics.

Required proof:

- IME composition is not reduced to commit-only behavior.
- TextArea covers vertical navigation and multiline selection.
- Accessibility exposes value, selection, composition, disabled/read-only, and
  error state.

### Native Accessibility Bridge

Owners: #40, #83

Route roles, labels, values, focus, actions, announcements, and scroll semantics
through the native macOS bridge. Unsupported bridge features should return
explicit errors so missing data is not mistaken for working accessibility.

Required proof:

- Semantic node tests cover roles and required values.
- Native bridge tests cover supported actions and unsupported errors.
- ScrollView semantics expose viewport and position information.

### Theme and Tokens

Owner: #94

Replace hard-coded advanced UI token helpers with an app-owned theme contract.
The contract should cover color, radius, spacing, typography, density, and
shared state tokens for disabled, read-only, invalid, focused, hovered, pressed,
loading, and error states.

Required proof:

- Theme changes rebuild controls deterministically.
- Advanced controls consume tokens without source edits for each app theme.
- Snapshot or headless tests prove token values affect rendered primitives.

### Data Surfaces and Advanced Components

Owners: #44, #79, #80, #81, #82

Advanced components should share sizing, interaction state, validation, focus,
and accessibility rules. Data surfaces should support stable row identity,
virtualization-ready structure, keyboard navigation, selection, and accessible
list/table/tree semantics.

Required proof:

- Component state machines follow shared contracts unless a distinct interaction
  model is documented.
- Data rows have stable identity independent of visual order.
- Accessibility tests cover list/table/tree state.

### Native Dogfood and Docs/API Drift

Owner: #95

Add finite native macOS smoke coverage and docs/API drift checks. Examples must
stay CI-safe by default. Public docs should distinguish implemented behavior
from roadmap work and include concrete validation commands.

Required proof:

- Native dogfood launches, interacts, captures profiling output, and exits.
- Public builders and enum variants are compile-checked or drift-checked.
- README and docs avoid claiming unsupported behavior as complete.

## Implementation Order

1. Stabilize ownership and state rules (#88).
2. Stabilize platform lifecycle and redraw semantics (#89).
3. Replace sentinel sizing with structured dimensions (#90).
4. Add renderer telemetry and bounded-resource checks (#91).
5. Route actions through runtime focus scopes (#92).
6. Complete IME and TextArea contracts (#93).
7. Finish native accessibility and data-surface semantics (#40, #81, #83).
8. Add app-owned theme tokens and component contract cleanup (#44, #79, #80,
   #82, #94).
9. Add native dogfood and docs/API drift gates (#95).

## Validation Policy

Each child issue should run its focused validation command. The umbrella runtime
claim should not advance unless these commands are fresh in local or CI output:

- `cargo check`
- `cargo test`
- `cargo test platform_window`
- `cargo test headless`
- `cargo test accessibility`
- `cargo test action_keymap`
- `cargo test renderer_resource`
- `cargo test benchmark_config`

Native macOS dogfood and calibrated benchmark enforcement are required before
performance or platform readiness claims, but they may remain local-only until
the repository has stable CI hardware for them.
