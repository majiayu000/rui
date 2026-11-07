# ADR 0002: Renderer backends: one frame planner, one encoder per GPU API

Status: accepted. Date: 2026-11-07. Base: `main` at d76ae8d.

## Context

RUI renders only through Metal, so it runs only on macOS. Everything between the
`Scene` and the Metal calls lived in `src/renderer/metal/`, but most of it never
touched Metal:

| file | what it does | Metal types it used |
|---|---|---|
| `metal/order.rs` | overlap order (GPUI's `BoundsTree`) | none |
| `metal/frame.rs` | `FramePlan`: items → sprite refs and draws | none |
| `metal/path.rs` | path tiles, path sprites, CPU coverage | none |
| `metal/renderer/planning.rs` | scene → instance data, clips, text runs, images | `MTLScissorRect`, `Texture` for images |
| `metal/retained.rs` | damage of a kept texture, damage scissor | `Texture`, `MTLScissorRect` |
| `metal/glyph.rs`, `image_primitive.rs` | `GpuGlyph`, `GpuImage` instance layouts | none |

A second backend that re-derived this would drift from Metal in draw order,
clipping and damage, which are exactly what the pixel and perf gates check.

## Decision

### 1. `src/renderer/plan/`: everything a frame needs before a GPU API

Moved out of `metal/` unchanged in behavior, compiled when any GPU backend is:

- `order.rs`, `frame.rs`, `path.rs`: as before.
- `instances.rs`: the `#[repr(C)]` instance layouts `GpuGlyph`, `GpuImage`
  (with `GpuQuad`, `GpuShadow` in `primitives.rs`, `GpuPathTile` in `path.rs`).
  Their byte layout is the contract every shader set reads; the MSL and WGSL
  structs mirror them field for field.
- `mod.rs`: `FramePlanner<T>` owns the backend-neutral state: the text glyph
  cache and its atlases (CPU side), the image cache, path sprite and tile caches,
  and the image texture bookkeeping (`ImageTextures<T>`: resource limits and
  eviction, generic over the backend's texture handle `T`). `FrameData<T>` is the
  frame it fills: instance arrays, `FramePlan` refs and draws, clip rectangles
  (`ScissorRect` in device pixels, and the same as edges for the shader's
  per-fragment test), and the damage scissor.
- `build.rs` (was `planning.rs`): `FramePlanner::build` turns a `Scene` into a
  `FrameData`, including the rebuild when a font load empties the glyph atlas.
- `retained.rs`: `RetainedFrame` remembers which `Scene::frame` a kept texture
  shows and answers the damage the next scene must draw; `damage_scissor` rounds
  it to device pixels.

The only thing planning asks of a backend is `TextureUpload`: turn an image's
RGBA8 sRGB pixels into a texture handle. A frame is then:

```text
backend.begin_frame:  retire finished GPU work → planner.begin_frame(oldest frame in flight)
planner.build:        Scene → FrameData<T>            (shared, CPU only)
backend.encode:       atlas uploads → one pass of planned draws → copy/present
```

### 2. What each backend owns

| | Metal (`renderer/metal/`) | wgpu (`renderer/wgpu/`, feature `wgpu`) |
|---|---|---|
| shaders | MSL in `shaders.rs`, compiled at startup | WGSL ports in `wgpu/shaders.rs`, compiled by naga |
| instance data | pooled shared `MTLBuffer`, one per frame in flight | one storage buffer, grown as needed, written with `Queue::write_buffer` |
| per-draw values | `set_vertex_bytes` | the same frame buffer (images as an array, uniforms at its head) |
| atlas pages | private textures, cleared and filled by a blit from a staging buffer | textures filled with `Queue::write_texture` (the queue orders it after earlier submissions) |
| page binding | `set_fragment_texture` per draw | a bind group per (coverage page, color page) pair, cached until the atlas epoch changes |
| image textures | `MTLTexture` | `wgpu::Texture` plus its bind group |
| frames in flight | command buffer status | `Queue::on_submitted_work_done` counter |
| present | `CAMetalLayer` drawable, after `wait_until_scheduled` | caller's surface texture (`Renderer::Target = wgpu::Texture`); offscreen captures in this step |
| GPU time | `GPUStartTime`/`GPUEndTime` | not yet (timestamp queries are optional in wgpu) |

Both draw sprites (quads, shadows, glyphs, color glyphs, path tiles, path
sprites) through one instanced pipeline indexed by `SpriteRef` (kind in the top
three bits), and images through a second pipeline, in the order `FramePlan`
gives. Both render into an 8-bit BGRA sRGB target (`BGRA8Unorm_sRGB`,
`Bgra8UnormSrgb`) with `src_alpha, 1 - src_alpha` color blending and
`one, 1 - src_alpha` alpha blending, so blending happens in linear light and the
shaders convert scene colors from sRGB to linear exactly as the Metal ones do.
Coverage pages are `R8Unorm`, color pages and images `RGBA8 sRGB`.

### 3. Retained target and damage on wgpu

The planner already cuts every clip to the damage scissor and skips primitives
that cannot touch it, so a backend only has to:

1. keep a texture of the drawn size (`RENDER_ATTACHMENT | TEXTURE_BINDING`),
   and ask `RetainedFrame` for the damage;
2. with damage, begin the pass with `LoadOp::Load`, set clip 0 (the damage) as
   the scissor and draw the clear color with a full-screen triangle, as a
   cleared target would start; without, `LoadOp::Clear`;
3. draw the planned frame, then copy the kept texture to the output with a
   full-screen triangle that `textureLoad`s each pixel (a render pass, as on
   Metal, because a surface texture need not allow `COPY_DST`).

### 4. Scope of the first wgpu step

Feature `wgpu` (off by default) adds two dependencies:

- `wgpu` 29 (MSRV 1.87), default features off; `std`, `parking_lot`, `wgsl`,
  `metal`, `vulkan`, `dx12`. GLES and WebGPU are left out: the sprite pipeline
  reads six storage buffers in the vertex stage, which downlevel GL does not
  offer. `raw-window-handle` comes with it and is not used directly yet.
- `pollster` 0.4 (already a dev-dependency), to block on adapter, device,
  error-scope and readback futures. It is a single small crate with no
  dependencies.

The backend renders offscreen and to any `Bgra8UnormSrgb` texture a caller
supplies. A windowed platform layer for Windows and Linux is the next step and
needs no renderer change. GPU timing and the offscreen benchmark entry points
(`render_offscreen*`) stay Metal-only for now.

## Consequences

- Metal output does not change: the extraction moves code, and the Metal
  encoder reads the same `FrameData`. The pixel-checked perf gate
  (`PERF_GATE_PIXELS=1 cargo bench --bench perf_gate`) gives the same checksum
  and the instruction and allocation budgets hold with the feature off.
- `renderer::wgpu::tests` draws the 48 seeded random scenes of the Metal
  planning oracle (quads, gradients, shadows, text, images, paths, nested
  clips, at 1x and 2x), a fixture with one of each primitive (radial gradient,
  emoji, rounded image, tiled and atlas paths), and a retained full frame plus
  a damaged one, through both backends, and compares the pixels and draw
  counts. Different shader compilers round `pow`, `exp` and `fwidth`
  differently, so the test allows 8 levels per channel and 1% of pixels past 2
  levels. Measured with wgpu over Metal: the fixture and retained frames are
  identical, 44 of 48 random scenes are identical, three differ by 1 level, one
  by 4 levels on 0.17% of its pixels.
- A new primitive or instance field is added once in `plan/` and once per
  shader set; the cross-backend test fails if a shader set misses it.
- Shader edits to the Metal sprite shader (for example dropping quad
  antialiasing) must be mirrored in WGSL, or the cross-backend test fails.
