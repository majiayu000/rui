# Developer tools

RUI's developer tools live in `src/devtools/`. They are off by default, and a
frame with them off pays one branch per stage and allocates nothing for them.

## Profiling overlay

A panel in the top-right corner of every window with the last complete frame's
numbers. It is drawn straight into the scene after paint, outside layout, so it
never moves the app's elements and is not hit-tested.

Turn it on:

- `RUI_DEVTOOLS=1 cargo run --example animation_demo` starts with it on.
- `cx.toggle_profiler()` (on `AppContext` or `ViewContext`) shows or hides it in
  every window; `cx.profiler_enabled()` reads the state.

What it shows, for the frame before the one on screen:

| Line | Meaning |
|------|---------|
| `frame N` | Frames this window drew with the profiler on. |
| `interval`, `fps` | Time since the previous drawn frame began. Idle windows draw no frames, so this is only a rate while something redraws. |
| `dropped` | Display refreshes missed while an animation asked for every refresh: a frame more than 1.5 refresh intervals after one that asked for the next refresh dropped the refreshes in between. |
| `prepare`, `rebuild`, `layout`, `paint` | Wall time of those `FrameStage`s. |
| `present`, `drawable` | The whole present stage, and of it the wait for a drawable. Headless runners show `(no backend)`. |
| `encode`, `scheduled` | Encoding and committing the frame, and waiting for its commands to be scheduled before the drawable is presented with the Core Animation transaction. |
| `gpu` | GPU time of the latest frame the GPU finished (a frame or two behind), from the command buffer's `GPUStartTime`/`GPUEndTime`. |
| `draws`, `instances` | `MetalRenderer::last_frame_stats()`. |
| `damage` | Share of the window `Scene::damage` reported changed, the overlay itself left out. |
| `redrawn` | Device pixels the renderer drew again, overlay included. |
| `prims`, `replayed` | Primitives in the scene and how many were replayed from the previous frame. |
| `allocs` | Heap allocations from rebuild through present, the overlay's own left out. |

Allocations are counted only by `rui::devtools::CountingAllocator`, which the app
has to install, since only a binary chooses its global allocator:

```rust
#[global_allocator]
static ALLOCATOR: rui::devtools::CountingAllocator = rui::devtools::CountingAllocator;
```

Without it the line reads `allocs n/a`. Installed, it counts only while a
profiler is on; off, each allocation costs one relaxed atomic load.

The same counters can be read in code: `Presenter::profiler()` (and
`HeadlessSession::profiler()` in tests) returns the window's `Profiler`, whose
`last_frame()` is a `FrameSample`.

### What it costs when on

- Ten primitives at the end of the scene, and their strings each frame.
- Text may reach past its bounds horizontally (see `Primitive::reach`), so the
  renderer redraws a full-width band about 140 px tall at the top of the window
  every frame, in addition to the app's own damage, which it joins into one
  rectangle. Render times with the overlay on include that.
- The renderer reads the GPU time of each finished command buffer and times the wait for scheduling.

### Relation to `RUI_PROFILE`

`RUI_PROFILE=1` prints one `rui.renderer.profile.v1` JSON line per frame to stderr,
for scripts. The overlay shows the same stage times on screen, and the numbers
`RUI_PROFILE` does not have: prepare and rebuild time, GPU time, damage and
replay, dropped frames and allocations. The two are independent.
