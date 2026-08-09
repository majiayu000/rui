# Changelog

All notable project changes should be recorded here.

## Unreleased

### Performance

- System fonts are now parsed once per process and text measurement metrics
  survive across frames, instead of both being rebuilt every frame.

### Runtime

- Added `LayoutContext::with_text_measurer` for callers that hold a text
  measurement cache across frames. The cache-aware pipeline entry points are
  `FramePipeline::layout_root_with_text_measurer` and
  `FramePipeline::build_frame_with_text_measurer`; the original signatures
  remain available for source compatibility.

## 0.2.5 - 2026-05-31

### Accessibility

- Added accessibility node contract fields for read-only, invalid, text input ranges, and scroll positions.
- Exposed text input accessibility semantics for value, caret, selection, and composition state.
- Added progress indicator accessibility semantics with label overrides and value reporting.

## 0.2.4 - 2026-05-31

### Documentation

- Added a README build/run clarification for local example workflows.

## 0.2.3 - 2026-05-31

### Launch Readiness

- Documented current limitations and caveats in the README.
- Documented release status for the current pre-release period.
- Added GitHub issue and pull request templates for outside contributors.

### Release Status

- Current repository version is `0.2.3`.
- GitHub release tags are published for recent changes.
- crates.io publishing is blocked until the `rui` package name ownership or package naming is resolved; use the Git dependency and pin a commit or release tag for reproducible builds.
