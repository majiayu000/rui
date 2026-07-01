# GH118 Technical Spec: Post-Foundation Runtime Readiness

## Source Of Truth

- `docs/runtime-architecture-foundation.md` owns the runtime boundary.
- `docs/advanced-ui-runtime-roadmap.md` owns maturity gates and verification
  language.
- `docs/platform-window-boundary.md` owns platform backend readiness behavior.
- `docs/performance-benchmarks.md` owns benchmark policy.

## Architecture Constraints

- Keep renderer modules free of native window API ownership.
- Keep platform adapters responsible for native lifecycle, DPI, focus,
  clipboard, renderer attachment, and unsupported errors.
- Keep headless and native frame behavior shared where possible; when they
  differ, model the difference explicitly.
- Return typed errors or panic according to the existing native policy. Do not
  add warning-only fallback paths for missing runtime features.
- Keep public API changes out of child issues unless the issue explicitly
  requires them and tests cover the change.

## Child Issue Plan

### GH119 Backend Readiness Gates

Update documentation and deterministic platform tests so unsupported backends
return explicit errors and future backend issues can reuse a gate matrix.

### GH120 Presenter

Introduce a per-window owner for root element, layout tree, scene, focus,
pointer state, root bounds, and frame diagnostics. It must not own native
window handles, Metal layers, or renderer backend resources.

### GH121 No-Default-Features Build

Gate macOS/Metal implementation modules behind `all(target_os = "macos",
feature = "metal")`. Keep no-default behavior explicit rather than silently
falling back to a fake native backend.

### GH122 Dogfood And Benchmark Gates

Separate finite smoke, repository-owned dogfood, benchmark policy, and docs
drift checks. Benchmark enforcement remains advisory until noise policy and CI
conditions are documented.

### GH123 Native Accessibility Bridge

Map semantic accessibility nodes to native bridge roles, labels, values,
actions, focus, and announcements. Unsupported bridge features must be explicit
errors, not missing data.

### GH124 Shared FramePipeline

Extract shared frame ordering for rebuild, layout, event dispatch, paint, scene
finish, render, diagnostics, and redraw completion. Pointer hit testing must
continue to use the previous completed scene until a pre-dispatch hit-region
pass exists.

## Verification Matrix

- `cargo check --no-default-features`
- `cargo check --examples --no-default-features`
- `cargo check --examples`
- `cargo test headless`
- `cargo test event_targeting`
- `cargo test platform_window`
- `cargo test accessibility`
- `cargo test dogfood`
- `cargo test example_smoke`
- `cargo test benchmark_config`
- `cargo test docs_api_drift`
- `cargo test renderer_resource`
- `cargo test`
