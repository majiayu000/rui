# GH118 Tasks: Post-Foundation Runtime Readiness

## T0 Spec And Queue Mapping

- [x] Create `specs/GH118/product.md`.
- [x] Create `specs/GH118/tech.md`.
- [x] Create `specs/GH118/tasks.md`.
- [x] Map child issues `#119` through `#124` back to umbrella issue `#118`.

## T1 GH121 No-Default-Features Build

- [x] Reproduce `cargo check --no-default-features`.
- [x] Gate macOS/Metal modules behind the `metal` feature.
- [x] Keep unsupported no-default runtime behavior explicit.
- [x] Verify `cargo check --no-default-features`.
- [x] Verify `cargo check --examples --no-default-features`.
- [x] Verify `cargo check --examples`.
- [x] Verify `cargo test`.

## T2 GH119 Backend Readiness Gates

- [x] Add or update backend readiness matrix documentation.
- [x] Ensure unsupported backend behavior is explicit error/diagnostic.
- [x] Verify `cargo test platform_window`.
- [x] Verify `cargo test headless`.
- [x] Verify `cargo test docs_api_drift`.

## T3 GH122 Dogfood And Benchmark Gates

- [x] Ensure dogfood uses repository-owned data for at least one real workflow.
- [x] Document smoke, dogfood, benchmark, and docs-drift validation matrix.
- [x] Document advisory vs blocking benchmark policy and noise handling.
- [x] Verify `cargo test dogfood`.
- [x] Verify `cargo test example_smoke`.
- [x] Verify `cargo test benchmark_config`.

## T4 GH123 Native Accessibility Bridge

- [x] Map roles, labels, values, actions, focus, and announcements through the
  native macOS bridge or explicit unsupported errors.
- [x] Fail closed for missing required accessibility metadata.
- [x] Verify `cargo test accessibility`.
- [x] Verify `cargo test platform_window`.

## T5 GH124 Shared FramePipeline

- [x] Extract or explicitly model shared native/headless frame order.
- [x] Cover notification preservation and dirty clearing.
- [x] Cover redraw source classification.
- [x] Cover pointer hit testing against the previous completed scene.
- [x] Verify `cargo test headless`.
- [x] Verify `cargo test platform_window`.
- [x] Verify `cargo test event_targeting`.
- [x] Verify `cargo test renderer_resource`.
- [x] Verify `cargo test`.

## T6 GH120 Presenter

- [x] Move per-window presentation state into `Presenter`.
- [x] Keep native handles and renderer backend resources outside `Presenter`.
- [x] Route native and headless runners through presenter-owned state.
- [x] Verify resize, focus, pointer capture, rebuild, and redraw behavior.
- [x] Verify `cargo test headless`.
- [x] Verify `cargo test event_targeting`.
- [x] Verify `cargo test platform_window`.
- [x] Verify `cargo test`.
