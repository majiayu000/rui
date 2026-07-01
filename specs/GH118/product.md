# GH118 Product Spec: Post-Foundation Runtime Readiness

## Summary

RUI has a healthy macOS/Metal foundation, headless tests, example smoke tests,
and a finite native dogfood path. It is not yet a production-grade runtime
claim. This spec tracks the remaining post-foundation readiness work and keeps
each child issue tied to testable gates instead of visual similarity or broad
runtime claims.

## Goals

- Define the product-facing readiness contract for post-foundation runtime work.
- Keep `Presenter` and `FramePipeline` work aligned with
  `docs/runtime-architecture-foundation.md`.
- Keep unsupported platform, renderer, accessibility, and dogfood gaps explicit.
- Link every child issue back to a concrete validation command and acceptance
  surface.

## Non-Goals

- Do not claim WarpUI parity.
- Do not implement Windows, Linux, or Web backends in this umbrella issue.
- Do not treat finite smoke tests or one benchmark run as production proof.
- Do not introduce a virtual DOM or replace RUI's current app-owned state model.

## User-Facing Acceptance

- `#119` defines backend readiness gates and unsupported backend behavior.
- `#120` extracts per-window presentation state into a `Presenter`.
- `#121` makes `cargo check --no-default-features` pass without compiling
  Metal/AppKit modules.
- `#122` defines dogfood and benchmark enforcement gates beyond finite smoke.
- `#123` completes native macOS accessibility bridge routing or explicit
  unsupported errors.
- `#124` extracts a shared `FramePipeline` or explicitly models native/headless
  differences.
- Runtime readiness language remains consistent with
  `docs/runtime-architecture-foundation.md` and
  `docs/advanced-ui-runtime-roadmap.md`.

## Done When

- This spec has `product.md`, `tech.md`, and `tasks.md`.
- Open child issues link back to this umbrella issue.
- Every child issue has testable done-when criteria.
- Implementation PRs use closing keywords only when the linked acceptance
  criteria are satisfied.
