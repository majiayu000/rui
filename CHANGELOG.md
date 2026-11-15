# Changelog

All notable project changes should be recorded here.

## Unreleased

### Rendering

- **Breaking:** `Primitive::Path` takes `vertices: Arc<[PathVertex]>` and an
  `offset: Point` added to each vertex. Keep the `Arc` and paint clones of it
  while the shape stays the same: the renderer keeps a path's edges and its atlas
  sprite or tiles by that storage, and moving it only changes the offset.
- `Primitive::Picture` draws a `Picture`, primitives recorded once, at an
  offset. The renderer keeps what it planned for a picture and copies it into
  later frames while the picture is drawn at the same offset, scale and clip.
- `canvas(..).cache_key(key)` records the closure into a `Picture` and paints it
  again without the closure while the key and the canvas bounds stay the same;
  the canvas is then pure, so its paint is replayed. `svg` icons are pictures.
- Path tiles cut by the visible area's left side no longer lose the inside of
  the path left of the first edge in the area.

- A `wgpu` cargo feature adds `renderer::wgpu::WgpuRenderer`, which draws the
  same planned frames as Metal with WGSL ports of its shaders over Vulkan, DX12
  or Metal, offscreen or into a caller's `Bgra8UnormSrgb` texture (ADR 0002).
  Frame planning moved from `renderer/metal` to the backend-neutral
  `renderer/plan`; Metal output is unchanged.

### Performance

- System fonts are now parsed once per process and text measurement metrics
  survive across frames, instead of both being rebuilt every frame.

### Runtime

- macOS native windows now route AppKit text input through `NSTextInputClient`,
  including composition begin, update, commit, and cancel events without
  duplicating the corresponding key text.
- The native macOS dogfood driver now captures and validates renderer frame
  telemetry, including pacing, latency, percentile, phase, and jank metrics.
- Added `LayoutContext::with_text_measurer` for callers that hold a text
  measurement cache across frames. The cache-aware pipeline entry points are
  `FramePipeline::layout_root_with_text_measurer` and
  `FramePipeline::build_frame_with_text_measurer`; the original signatures
  remain available for source compatibility.
- `Presenter` now owns the root element it presents and the renderer diagnostics
  snapshot for its window; native and headless runners drive frames through it
  instead of holding their own copies.
- `AppContext` is the single owner of viewport size. `Presenter` no longer keeps a
  duplicate that had to be synchronized by hand.
- Frames now run through `FramePipeline::run_frame`, driven by `FrameStage::ORDER`.
  Native and headless runners share that order and only supply the two
  platform-dependent stages, `DispatchEvents` and `Present`.
- Added `AppContext::has_frame_work` as the single definition of an idle frame.

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
