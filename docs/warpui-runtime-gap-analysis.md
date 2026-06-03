# WarpUI Runtime Gap Analysis

This document tracks the runtime gaps behind issue #87. It uses public Warp
repository signals as comparison anchors, but it does not make WarpUI the source
of truth for RUI. RUI should keep its own architecture and add explicit runtime
contracts where the current implementation is still too demo-oriented or
component-local.

## Public Comparison Anchors

Warp publishes the `warpui_core` and `warpui` crates in `warpdotdev/warp`. The
public `warpui_core` source exposes modules for app/entity ownership,
accessibility, actions, clipboard, fonts, image cache, keymap, modals,
presenter, rendering, scene, text, UI components, windowing, and zoom.

The useful comparison point is not visual parity. It is the runtime shape:
long-lived application state, handle-like references to models/views,
render-time context, focus-aware actions, platform lifecycle ownership, and
renderer/resource observability.

## Current RUI Baseline

RUI already has a native foundation:

- `core::app`, `core::view`, and `core::entity` provide stateful view rebuilds.
- `core::action` provides typed actions and keymaps.
- `core::text_editing` and `elements::input` cover text input primitives.
- `core::accessibility` and `platform::mac::accessibility` cover semantic nodes
  and macOS bridge groundwork.
- `renderer` and `renderer::resources` expose primitive rendering and resource
  ownership.
- `advanced_ui` contains shared controls and token helpers.
- `tests/*` and `benches/*` cover headless behavior, platform contracts,
  renderer resources, action/keymap behavior, accessibility, and benchmark
  configuration.

That baseline is stronger than a visual demo, but issue #87 is about the parts
that still need product-runtime contracts before RUI can claim mature runtime
readiness.

## Gap Matrix

| Area | Current Risk | Owning Issue |
| --- | --- | --- |
| Runtime ownership | App, view, model, caller-owned, and component-local state boundaries are not fully documented as one ownership model. | #88 |
| Platform lifecycle | macOS close, minimize, reopen, activation, redraw, focus, and multi-window behavior need explicit delegate-backed or unsupported paths. | #89 |
| Responsive layout | Public sizing still needs structured dimension semantics instead of pixel-first and sentinel-value behavior. | #90 |
| Renderer telemetry | Frame pacing, allocation behavior, cache pressure, and batching are not observable enough for readiness claims. | #91 |
| Action routing | Typed keymaps need to flow through runtime focus scopes rather than living beside raw key forwarding. | #92 |
| Text and IME | TextField work does not cover the full IME begin/update/cancel/commit path or multiline TextArea contracts. | #93 |
| Native accessibility | Semantic nodes need complete native macOS bridge routing for roles, values, actions, focus, and announcements. | #40, #83 |
| Theme and tokens | Advanced UI token helpers are not yet an app-owned theme contract with deterministic rebuild behavior. | #94 |
| Data surfaces | Lists, tables, trees, and virtualized surfaces need stable identity and accessibility semantics. | #81 |
| Component primitives | Advanced components need shared size, state, focus, and accessibility contracts. | #44, #79, #80, #82 |
| Dogfood and docs drift | Native dogfood, docs/API drift checks, and finite examples need automated gates. | #95 |

## Cross-Cutting Acceptance Gates

Every runtime workstream should satisfy these gates before its readiness claim is
accepted:

- The affected runtime boundary is documented in `docs/`.
- Public APIs have a real caller, example, or focused test.
- Unsupported platform, accessibility, renderer, or data paths return explicit
  errors or diagnostics.
- Tests cover the changed behavior at the narrowest useful layer.
- The workstream updates the roadmap or API docs when public behavior changes.

## Validation Matrix

Use focused commands for each workstream and keep `cargo check` plus
`cargo test` as the umbrella gates.

| Workstream | Focused Validation |
| --- | --- |
| Runtime ownership and state | `cargo test headless`, `cargo test advanced_ui`, `cargo test action_keymap` |
| Platform lifecycle | `cargo test platform_window`, `cargo test headless` |
| Responsive layout | `cargo test headless`, `cargo test advanced_ui_layout_tests` |
| Renderer telemetry | `cargo test renderer_resource`, `cargo test renderer_backend`, `cargo test benchmark_config` |
| Action routing | `cargo test action_keymap`, `cargo test event_targeting`, `cargo test text_editing` |
| Text and IME | `cargo test text_editing`, `cargo test accessibility` |
| Accessibility bridge | `cargo test accessibility`, `cargo test platform_window` |
| Theme and tokens | `cargo test advanced_ui`, `cargo test primitive_snapshot`, `cargo test accessibility` |
| Data surfaces and components | `cargo test advanced_ui`, `cargo test headless`, `cargo test accessibility` |
| Dogfood and docs drift | `cargo test example_smoke`, `cargo test dogfood`, `cargo check` |

## Readiness Rule

Do not describe RUI as WarpUI-equivalent or production-ready because the surface
looks similar. Runtime readiness requires explicit ownership, lifecycle,
layout, renderer, action, text, accessibility, theme, data-surface, dogfood, and
documentation gates to be verified with fresh local or CI output.
