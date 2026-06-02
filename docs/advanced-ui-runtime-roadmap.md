# Advanced UI Runtime Roadmap

This spec defines the path for evolving RUI into a production-grade
application UI runtime while keeping the implementation native to this
repository. The plan is incremental: each workstream should land with
observable contracts, tests, and examples before higher-level readiness claims
are made.

Related WarpUI-aligned planning:

- `docs/warpui-runtime-gap-analysis.md` documents the current design gaps
  between RUI and the public WarpUI runtime anchors.
- `docs/warpui-runtime-implementation-spec.md` turns those gaps into staged
  runtime workstreams and validation gates.

## Goals

- Preserve the lightweight RUI architecture: `View` builds elements, elements
  produce layout and scene primitives, renderer/platform boundaries present
  explicit contracts.
- Grow advanced UI capabilities inside RUI rather than vendoring an external
  runtime.
- Keep failures explicit. Unsupported platforms, missing renderer resources,
  missing accessibility metadata, unstable snapshots, and unavailable local
  data should return errors or visible diagnostics.
- Tie readiness to verification: tests, benchmarks, headless snapshots,
  accessibility output, and real local dogfooding.

## Non-Goals

- Do not import another runtime's source tree as the UI layer.
- Do not claim production readiness from visual similarity alone.
- Do not treat demo placeholders or seeded sample data as dogfood evidence.
- Do not hide missing platform or renderer features behind silent fallback
  paths.

## Architecture Target

The target runtime keeps the current layering and strengthens the contracts
between layers.

| Layer | Contract |
| --- | --- |
| Application | Stateful `View` rendering, notifier-driven rebuilds, explicit lifecycle state. |
| Elements | Stable layout, event dispatch, accessibility nodes, and primitive output. |
| Advanced UI | Reusable controls with shared interaction state, validation, sizing, and semantics. |
| Text | Measured text, editing, selection, IME composition, clipboard, and caret geometry. |
| Actions | Typed actions and keymaps routed by focus, component handlers, and app handlers. |
| Accessibility | Required labels and values, testable announcements, and platform bridge errors. |
| Renderer | Scene recording, resource cache ownership, pressure handling, and explicit resource failures. |
| Platform | Window lifecycle, input, focus, DPI, clipboard, and renderer attachment per backend. |
| Testing | Headless layout/event/accessibility assertions, primitive snapshots, frame capture hooks, and finite smoke tests. |

## Completed Foundation

These issue lanes have landed and form the current baseline:

- #5 scene foundations and deterministic scene tests.
- #6 advanced UI layout wrappers.
- #7 z-order event targeting and hoverable behavior.
- #8 measured text layout and raster diagnostics.
- #9 stateful view runtime and notifier rebuilds.
- #10 renderer/platform boundary split.
- #11 first advanced UI controls.
- #12 production runtime hardening umbrella.
- #20 complex text editing and IME support.
- #21 keymap and typed action routing.
- #22 accessibility semantics and announcements.
- #23 cross-platform windowing boundary validation.
- #24 renderer resource lifecycle management.
- #25 benchmark baselines and thresholds.
- #26 shared component interaction state boundaries.
- #27 headless and visual testing tools.
- #28 local dogfood app using repository-owned data.

## Remaining Production Gates

The current implementation is stronger than a demo toolkit, but production
readiness still depends on repeated validation under real product pressure.
Use these gates before making stronger claims:

1. Text and IME
   - Cover multi-stage composition, replacement ranges, marked text, grapheme
     motion, selection painting, clipboard failures, and multiline editing.
   - Require fresh tests for each supported editing behavior.

2. Action Routing
   - Keep keymaps typed and conflict-checked.
   - Route actions through focus, component, and app scopes with explicit
     handled/ignored results.
   - Add regression tests when new controls introduce default bindings.

3. Accessibility
   - Require labels for controls and values for stateful widgets.
   - Keep announcements testable before platform bridge wiring is considered
     complete.
   - Treat unsupported bridge features as explicit errors.

4. Platform Windowing
   - Each backend must implement lifecycle, input, DPI, resize, focus,
     clipboard, and renderer attachment contracts.
   - Non-macOS backends should stay explicitly unsupported until they have
     shared tests and real renderer targets.

5. Renderer Resources
   - Resource caches must expose ownership, reuse, disposal, and pressure
     behavior.
   - Missing glyphs, image decode failures, missing textures, or active content
     under pressure must produce explicit renderer errors.

6. Performance
   - Keep benchmark categories stable: layout, text, scene build, pointer
     dispatch, and recording throughput.
   - Turn benchmark enforcement on only after CI noise is measured and baseline
     hardware is stable.

7. Component Boundaries
   - Advanced controls should share sizing, validation, interaction state, and
     accessibility patterns.
   - Avoid one-off component state machines unless a control has a distinct
     interaction model.

8. Testing and Dogfood
   - Headless tests must assert layout, events, accessibility, snapshots, and
     missing-backend diagnostics.
   - Primitive snapshots must reject unstable image sources.
   - Example smoke tests must be finite and CI-safe.
   - Dogfood apps must use application-owned data, not placeholder fixtures.

## Workstream Rules

- Search existing modules and tests before adding a new abstraction.
- Add public APIs only when a real caller or test needs them.
- Prefer structured contracts over ad hoc string parsing when the runtime owns
  the data model.
- Keep new docs and tests tied to explicit commands.
- Close or update tracking issues only after fresh local or CI verification.

## Verification Matrix

| Claim | Required Proof |
| --- | --- |
| Layout/event behavior | `cargo test headless`, relevant component tests. |
| Snapshot stability | `cargo test primitive_snapshot`. |
| Example safety | `cargo test example_smoke`. |
| Dogfood coverage | `cargo test dogfood`, `cargo run --example advanced_ui_controls`. |
| Build health | `cargo check`, `cargo test`. |
| Runtime performance | `cargo test benchmark_config`, `cargo bench --bench runtime_baselines`. |
| Platform contract | `cargo test platform_window`. |
| Accessibility | `cargo test accessibility`. |

## Readiness Language

Use precise language for the current state:

- "Advanced UI foundation" means layout, events, controls, testing, and local
  dogfood are present.
- "Production-grade runtime" requires the production gates above to stay green
  across real applications, supported platforms, and benchmark baselines.
- If a gate is not verified in the current branch, describe it as pending
  validation instead of complete.
