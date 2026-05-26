# RUI Advanced UI Runtime Spec

Status: Draft
Owner: RUI maintainers
Last verified: 2026-05-26
Local RUI baseline: `65960d3`
Reference public baseline: `fc11033` (`2026-05-25`, `Tab group feature flag and entry points (#11486)`)

## Summary

This spec defines how RUI should evolve toward the useful parts of a
production-grade Rust UI runtime without becoming a direct fork of an external
runtime.

The recommended path is to implement an advanced UI runtime capability layer
inside RUI. RUI should keep its current lightweight architecture:

```text
App -> Element tree -> Taffy layout -> Scene primitives -> Metal renderer
```

The project should not vendor the external runtime as the main strategy. The
reference repository has a small MIT-licensed UI framework boundary, but those
crates are still a full application UI runtime with additional workspace-local
dependencies, platform glue, renderers, font systems, and build resources. A
direct extraction is feasible, but it is a separate project with higher
maintenance and license-audit cost.

## Decision

Build RUI into an advanced UI runtime by porting concepts, API ergonomics, and
selected component semantics.

Do not make RUI API-compatible with the external runtime in the first phase.
Instead, provide a small `advanced_ui` module that exposes familiar names and
composition patterns while mapping to RUI's native layout, rendering, and
platform loop.

## Source Facts

### RUI

RUI is a single MIT crate with these current layers:

- `src/core`: app context, entities, view abstraction, geometry, color, style,
  events, animation.
- `src/elements`: `Div`, `Text`, `Button`, `Input`, `Image`, `Table`, `List`,
  `Progress`, `Spinner`, and `ScrollView`.
- `src/renderer`: `Scene` and `Primitive`, plus macOS Metal renderer.
- `src/platform/mac`: AppKit window and event loop.

The current runtime path is:

1. `App::run` builds a root element from `FnMut(&mut AppContext) -> impl Element`.
2. The macOS loop collects pointer, scroll, key, focus, resize, and close events.
3. Elements build a Taffy tree in `layout`.
4. Elements paint `Primitive` values into `Scene`.
5. `MetalRenderer` renders the scene.

Current strengths:

- Small codebase with clear conceptual layers.
- Existing macOS native window and Metal pipeline.
- Existing Taffy-based flex layout.
- Simple scene primitive boundary.
- Existing event path into elements.

Current gaps:

- `View` exists but is not the primary runtime entry point.
- `Element` owns layout, paint, and event handling, making it a large adapter
  surface.
- Platform loop directly constructs and owns the Metal renderer.
- Text measurement is approximate.
- Scene has no z-index, hit map, retained layer model, or glyph-level cache.
- API documentation currently drifts from implementation in some places.

### Reference Runtime

The reference repository separates UI framework crates:

- core UI crate: MIT.
- platform UI crate: MIT.
- The wider application code is AGPL v3.

Relevant public references:

- <https://github.com/warpdotdev/warp>
- <https://github.com/warpdotdev/warp#licensing>

The reference runtime is not just a component library. It includes:

- `App`, `AppContext`, entity, model, view, handles, actions.
- Elements, UI components, presenter, scene, text layout, keymap, accessibility.
- Platform traits in core.
- Platform implementations, renderers, windowing, font handling, shaders, and
  asset plumbing in its platform crate.

Extracting the reference runtime as-is requires at least:

- the core UI crate
- the platform UI crate
- dependency versions from the root `Cargo.toml`
- `Cargo.lock`
- `crates/sum_tree`
- `crates/markdown_parser`
- `crates/string-offset`
- `crates/warp_util`
- `crates/command` for non-macOS paths
- optional `settings_value` crates when enabling those features
- Metal and wgpu shader resources
- ObjC/platform resources
- MIT license notices

## Goals

1. Make RUI capable of building complex application UIs in a production-grade
   runtime style.
2. Preserve RUI's simple single-crate development experience during early work.
3. Keep RUI's rendering path based on its own `Scene` and `Primitive` model.
4. Improve the framework in slices that can be tested independently.
5. Avoid importing AGPL code or workspace-local dependencies without explicit
   audit.
6. Give users a familiar builder-style API for containers, flex layouts, text,
   hoverable regions, scrollable content, and common controls.
7. Establish verification gates for layout, events, text, rendering primitives,
   and examples.

## Non-Goals

1. Do not provide full reference runtime API compatibility in the first implementation.
2. Do not vendor the external application layer.
3. Do not copy AGPL code from application, extras, or product modules.
4. Do not replace Taffy with the reference runtime's custom layout system.
5. Do not introduce a second full UI runtime inside RUI.
6. Do not add automation before the manual migration path is validated.
7. Do not ship platform expansion before macOS remains stable.

## Design Principles

- Prefer native RUI concepts over compatibility shims when behavior diverges.
- Add abstractions only when they unlock a concrete slice of functionality.
- Keep public APIs small until behavior is verified by examples and tests.
- Make render output inspectable through primitives before relying on screenshots.
- Treat text as a first-class subsystem, not as a convenience primitive.
- Keep license provenance explicit for any copied MIT source.
- Keep all new APIs in snake_case.
- No silent fallback for missing critical render, layout, or asset data.

## Target Architecture

```text
Application
  App
  AppContext
  ViewRuntime              future stateful view entry point

Element Runtime
  Element
  AnyElement
  LayoutContext
  PaintContext
  EventContext

Advanced UI Layer
  advanced_ui::Container
  advanced_ui::Flex
  advanced_ui::Text
  advanced_ui::Hoverable
  advanced_ui::Scrollable
  advanced_ui::controls

Layout
  Style
  Taffy adapter
  measured text
  scroll constraints

Scene
  Primitive
  Layer
  ZIndex
  Clip
  HitMap
  Resource refs

Renderer
  Renderer trait
  MetalRenderer
  resource caches

Platform
  PlatformApp or WindowBackend trait
  macOS AppKit backend
```

## Public API Direction

The first API should be familiar to advanced UI users but intentionally RUI-native:

```rust
use rui::prelude::*;
use rui::advanced_ui::{Container, Flex, Hoverable};

fn main() {
    App::new().run(|_cx| {
        Container::new(
            Flex::column()
                .spacing(12.0)
                .child(advanced_text("Project status").size(18.0))
                .child(
                    Hoverable::new(
                        Container::new(advanced_text("Run checks"))
                            .padding(10.0)
                            .background(Color::hex(0x2d3436))
                    )
                    .cursor(Cursor::Pointer)
                )
        )
        .background(Color::hex(0x101114))
        .into_element()
    });
}
```

API requirements:

- `advanced_ui` APIs must map to existing RUI `Element` implementations.
- Methods use RUI naming conventions, not the reference runtime's exact names when that would
  conflict with local style.
- The compatibility layer may be experimental and feature-gated at first.
- The compatibility layer must not require external runtime crates as
  dependencies.

## Workstream 1: Scene and Rendering Model

### Problem

RUI's current scene is a flat primitive list with a clip stack. The reference runtime's scene
supports richer layering, z-ordering, hit information, glyph/image/icon
resources, and visibility checks.

### Requirements

1. Add `ZIndex` as an explicit primitive ordering concept.
2. Add `Layer` as a scene grouping unit.
3. Add hit rectangles for event targeting.
4. Add visible-rect queries for clipped descendants.
5. Preserve current primitive rendering behavior.
6. Keep `Scene::primitives()` available for tests.
7. Avoid changing the Metal renderer in the same patch that introduces the data
   model, except where required to preserve behavior.

### Proposed Types

```rust
pub struct ZIndex(pub i32);

pub struct Layer {
    pub z_index: ZIndex,
    pub clip_bounds: Option<Bounds>,
    pub primitives: Vec<Primitive>,
    pub hit_regions: Vec<HitRegion>,
}

pub struct HitRegion {
    pub id: ElementId,
    pub bounds: Bounds,
    pub z_index: ZIndex,
}
```

### Acceptance

- Existing examples render unchanged.
- Unit tests prove z-index sort order.
- Unit tests prove clipped hit regions do not report hits outside clip bounds.
- `cargo check` passes.

## Workstream 2: Layout and Container Semantics

### Problem

RUI has `Div` with flexbox properties, but advanced UI style composition separates
containers, flex layout, constraints, alignment, and child view composition.

### Requirements

1. Introduce `advanced_ui::Container` as a semantic wrapper over RUI layout and
   paint behavior.
2. Introduce `advanced_ui::Flex` with `row`, `column`, `spacing`, axis alignment,
   cross-axis alignment, and child composition.
3. Preserve Taffy as the layout engine.
4. Avoid duplicate layout systems.
5. Provide focused tests for child bounds, spacing, alignment, and nested
   containers.
6. Keep existing `div()` API working.

### API Sketch

```rust
Container::new(child)
    .padding(8.0)
    .margin(4.0)
    .background(Color::BLACK)
    .border(1.0, Color::WHITE)
    .radius(6.0)

Flex::row()
    .spacing(8.0)
    .main_axis_alignment(MainAxisAlignment::Center)
    .cross_axis_alignment(CrossAxisAlignment::Stretch)
    .child(a)
    .child(b)
```

### Acceptance

- New `examples/advanced_ui_layout.rs` demonstrates nested container, row, and
  column behavior.
- Layout unit tests assert exact child bounds.
- Existing `dashboard`, `counter`, and `hello_world` examples still compile.

## Workstream 3: Event Targeting and Hoverable

### Problem

Events are dispatched through elements, but there is no shared hit-map or
advanced UI runtime hoverable/event handler abstraction.

### Requirements

1. Add event targeting based on computed bounds and z-index.
2. Introduce `Hoverable` with hover state and cursor intent.
3. Introduce event result semantics so handlers can either propagate or stop.
4. Preserve current button/input behavior.
5. Make focus transitions explicit.

### Proposed Event Result

```rust
pub enum DispatchEventResult {
    Propagate,
    Stop,
}
```

### Acceptance

- Hover tests cover enter, move, leave.
- Nested event tests prove topmost element receives pointer events first.
- Button click behavior remains intact.
- `use_mouse` can still observe pointer data.

## Workstream 4: Text System

### Problem

RUI currently estimates text width from character count. That is not sufficient
for complex application UI, terminal-like UI, or reliable layout.

### Requirements

1. Replace rough text measurement with font-backed measurement.
2. Introduce a text layout cache.
3. Distinguish measurement from rasterization.
4. Support single-line text clipping.
5. Support multiline text measurement.
6. Prepare for selection without requiring it in the first patch.

### Proposed Types

```rust
pub struct TextLayoutCache;

pub struct TextMetrics {
    pub size: Size,
    pub baseline: f32,
}

pub struct LaidOutText {
    pub metrics: TextMetrics,
    pub glyph_runs: Vec<GlyphRun>,
}
```

### Acceptance

- Tests compare measured width against rendered glyph bounds within tolerance.
- Text layout affects Taffy size.
- Empty text produces empty layout and no user-visible error.
- Missing system font is an error unless a declared fallback is available.

## Workstream 5: Stateful Views

### Problem

RUI has `View` and `ViewContext`, but the app runner uses an element builder
directly. The reference runtime's power comes from long-lived views and handles.

### Requirements

1. Add an explicit `App::run_view` entry point.
2. Keep `App::run` for simple apps.
3. Connect `View::render` to the rebuild path.
4. Let views notify themselves through `ViewContext::notify`.
5. Keep model/view handles out of the first implementation unless required.

### API Sketch

```rust
App::new().run_view(|cx| CounterView::new(cx));
```

### Acceptance

- Counter example uses `run_view`.
- Calling `notify` causes the view to re-render.
- Existing element-builder examples continue to work.

## Workstream 6: Renderer and Platform Boundaries

### Problem

The macOS platform loop directly creates `MetalRenderer`. That makes it harder
to test and harder to support alternate renderers.

### Requirements

1. Introduce a renderer trait.
2. Keep `MetalRenderer` as the default macOS renderer.
3. Move renderer construction behind an app/window option or backend factory.
4. Avoid changing AppKit event behavior in the same patch.

### Proposed Trait

```rust
pub trait Renderer {
    type Drawable;

    fn render(
        &mut self,
        scene: &Scene,
        drawable: &Self::Drawable,
        viewport_size: Size,
    ) -> Result<(), RenderError>;
}
```

### Acceptance

- `MetalRenderer` implements the trait.
- A test renderer can record scenes without creating a Metal device.
- macOS examples still render.

## Workstream 7: Components

### Initial Components

Implement these after the scene, layout, event, and text foundations:

1. `advanced_ui::Button`
2. `advanced_ui::TextInput`
3. `advanced_ui::Checkbox`
4. `advanced_ui::SegmentedControl`
5. `advanced_ui::Tooltip`
6. `advanced_ui::ProgressBar`
7. `advanced_ui::Scrollable`

### Requirements

- Each component has unit tests for state transitions.
- Each component has an example.
- Components use shared styling tokens.
- Components do not silently degrade when required data is absent.

## Workstream 8: Production Runtime Hardening

### Problem

After the foundational workstreams, RUI will have the shape of a serious UI
runtime, but it still will not have the long-running production qualities of a
runtime used by a complex desktop application.

### Requirements

1. Add complex text editing support: IME composition, selection ranges, cursor
   movement, word boundaries, clipboard integration, and multiline editing.
2. Add a keymap and action system with typed actions, standard actions, command
   routing, and conflict detection.
3. Add accessibility support: roles, labels, focus announcements, action
   feedback, and screen-reader state integration.
4. Add cross-platform windowing strategy after the renderer boundary is stable.
5. Add renderer resource management: texture atlas lifecycle, glyph cache
   eviction, image cache pressure handling, and GPU device reporting.
6. Add performance benchmarks for layout, text, scene building, event dispatch,
   and renderer throughput.
7. Tighten component boundaries so controls share event, focus, disabled, hover,
   active, and styling behavior.
8. Add testing tools: headless app runner, primitive snapshots, frame capture,
   visual snapshots, and example smoke tests.
9. Dogfood with at least one real local application before declaring the runtime
   production-ready.

### Acceptance

- IME and selection behavior are covered by tests and a manual example.
- Keymap/action dispatch is tested independently from platform events.
- Accessibility data can be inspected in tests.
- At least one non-macOS platform plan is validated against the renderer and
  window abstractions before implementation starts.
- Benchmarks establish baseline numbers and regression thresholds.
- A real application uses the advanced UI layer without app-specific hacks.

## License and Provenance Rules

1. Conceptual reimplementation is preferred.
2. If code is copied from the reference runtime's MIT crates, preserve copyright and license
   notices in the copied file or nearest module-level notice.
3. Do not copy code from application, extras, or any AGPL area.
4. Do not copy workspace-local dependencies unless their license has been
   verified and documented.
5. Add a provenance note to any file that contains copied MIT source.
6. Run a license audit before any release that includes copied external code.

## Implementation Plan

### Phase 0: Baseline and Guardrails

- Add this spec.
- Add a short architecture note linking this spec from `docs/ARCHITECTURE.md`.
- Add primitive snapshot tests for existing scenes.
- Add layout tests for current `Div` behavior.

Done when:

- `cargo check` passes.
- Existing examples compile.
- Current architecture limitations are documented.

### Phase 1: Scene Foundations

- Add `ZIndex`, `Layer`, and `HitRegion`.
- Preserve current flat primitive iteration.
- Add hit testing helpers.
- Add tests.

Done when:

- Existing rendering behavior remains unchanged.
- Hit testing and z-order tests pass.

### Phase 2: `advanced_ui` Layout Layer

- Add `src/advanced_ui/mod.rs`.
- Add `Container`.
- Add `Flex`.
- Add `advanced_text` or `advanced_ui::Text`.
- Add example.

Done when:

- A nested advanced UI style layout can be expressed without `div()`.
- Layout tests prove child bounds.

### Phase 3: Events and Hover

- Add event propagation result.
- Add topmost hit targeting.
- Add `Hoverable`.
- Add cursor intent.

Done when:

- Hover example works.
- Nested hit tests pass.

### Phase 4: Text Layout

- Add measured text layout.
- Add cache.
- Integrate measured size with Taffy.
- Keep current text rendering until replacement is proven.

Done when:

- Text no longer uses character-count width estimation.
- Text examples remain visually correct.

### Phase 5: Stateful View Runtime

- Add `run_view`.
- Wire `ViewContext::notify`.
- Add view example.

Done when:

- Stateful counter can be implemented as a `View`.
- Rebuild behavior is tested.

### Phase 6: Component Set

- Build components on top of the new foundations.
- Add examples and tests for each.

Done when:

- Layout, input, hover, and text behavior are shared rather than duplicated per
  component.

### Phase 7: Production Runtime Hardening

- Add complex text editing, IME, and selection support.
- Add keymap and action routing.
- Add accessibility APIs.
- Add renderer resource lifecycle management.
- Add performance benchmarks.
- Add headless and visual testing tools.
- Dogfood in a real app.

Done when:

- The runtime can be evaluated against a production-grade checklist, not just
  example UIs.

## Verification Strategy

Every phase must include fresh verification from the current session.

Required commands before completion of Rust changes:

```bash
cargo check
cargo test
```

For doc-only changes:

```bash
git diff --check
```

Recommended focused commands:

```bash
cargo test scene
cargo test layout
cargo test text
cargo test event
cargo run --example advanced_ui_layout
```

Visual verification should use screenshots for examples once behavior depends on
rendered output rather than primitive snapshots.

## Acceptance Criteria

The project reaches the target state when:

1. RUI can build complex nested UI without using low-level `Div` everywhere.
2. Layout is deterministic and covered by tests.
3. Text measurement and rendered text agree closely enough for application UI.
4. Event targeting respects z-order and clipping.
5. Components share common style and event foundations.
6. macOS Metal examples continue to run.
7. No AGPL code has been imported.
8. Any copied MIT source has explicit provenance.
9. Public docs match implemented API.
10. Production runtime hardening has a tracked follow-up plan with tests,
    benchmarks, and dogfood criteria.

## Risks

### Risk: Building a reference UI runtime fork accidentally

Mitigation:

- Keep RUI API native.
- Import concepts, not modules.
- Avoid direct dependency on external runtime crates.

### Risk: Scene rewrite breaks rendering

Mitigation:

- Add data model first.
- Preserve flat primitive iteration.
- Add primitive snapshot tests before renderer changes.

### Risk: Text system becomes too large

Mitigation:

- Split measurement, layout, cache, and rasterization.
- Land single-line measurement before rich text.

### Risk: Compatibility layer becomes permanent debt

Mitigation:

- Keep `advanced_ui` explicitly experimental until two examples and tests prove
  the surface.
- Promote stable pieces into core elements only after repeated use.

### Risk: License boundary is violated

Mitigation:

- Prefer reimplementation.
- Do license audit before copying any source.
- Keep provenance in source files.

## Open Questions

1. Should `advanced_ui` be a feature flag or always built?
2. Should `View` become the main app entry point after Phase 5?
3. Should RUI expose `Scene` layers publicly or keep them internal?
4. Which text stack should replace the current `rusttype` path long term?
5. Should non-macOS support wait until the renderer trait is stable?

## Recommended Next Task

Start with Phase 0 and Phase 1:

1. Add primitive snapshot tests around the current flat `Scene`.
2. Add `ZIndex`, `Layer`, and `HitRegion` without changing renderer output.
3. Prove the current examples still compile and `cargo test` passes.

This gives RUI the first necessary advanced UI runtime foundation while keeping the
blast radius small.
