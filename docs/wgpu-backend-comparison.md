# RUI on wgpu compared with RUI on Metal

Every other comparison in `docs/` draws RUI through its Metal backend, while egui, iced,
Slint and Vello go through wgpu. This one measures what RUI's own wgpu backend costs on
the same fixtures, so a lead over a wgpu-based library can be split into "the
framework" and "no wgpu".

```sh
cargo bench --bench compare_frame --features wgpu -- list_full
RUI_COMPARE_BACKEND=wgpu cargo bench --bench compare_frame --features wgpu -- list_full
```

Both runs are macOS: wgpu draws through Metal underneath, so the difference is wgpu's
own work plus RUI's wgpu encoder. Linux and Windows (Vulkan, DX12) are not measured.
The wgpu side draws into a kept offscreen texture per size and redraws all of it each
frame, and once three frames are in flight waits for the oldest, as the Metal side's
`render_offscreen` does. `WgpuRenderer::render` keeps its submission index, so the
bench submits an empty batch after each frame and waits on that; it costs the wgpu
side about 0.05 M instructions and one allocation a frame. wgpu commits several command buffers a frame; its GPU time is
their sum, as on egui's side.

## Results (RUI `main` 29544b1, 2026-11-26)

Apple M2 Max, macOS 27, three runs per case, each in its own process, load average
26–39. Instructions and allocations are the numbers to compare.

| Case | Metal instructions (M) | wgpu instructions (M) | wgpu / Metal | Metal allocations | wgpu allocations | Metal footprint (MB) | wgpu footprint (MB) |
|---|---|---|---|---|---|---|---|
| list_full | 17.07 | 17.55 | 1.03x | 4 077 | 4 151 | 60 | 68 |
| list_virtual_1k | 0.77 | 1.12–1.14 | 1.46x | 151 | 225 | 48 | 65–66 |
| text_paragraphs | 3.75–3.76 | 4.46–4.48 | 1.19x | 898 | 972 | 55 | 62 |
| quad_grid | 5.87–5.88 | 7.86–7.89 | 1.34x | 16 | 90 | 58–59 | 76–77 |
| resize_full | 39.05–39.32 | 39.54–39.80 | 1.01x | 16 088 | 16 162 | 58–59 | 84–87 |
| editor_scroll | 2.49–2.50 | 2.93–2.97 | 1.18x | 677 | 752 | 60 | 77 |
| terminal_rows | 3.15–3.16 | 3.66–3.71 | 1.17x | 885 | 959 | 55 | 63 |
| workbench_pane | 3.42–3.43 | 4.04–4.05 | 1.18x | 893 | 967 | 55 | 60–61 |
| workbench_full | 3.75–3.77 | 4.39–4.42 | 1.17x | 1 082 | 1 156 | 55–56 | 58–62 |
| paths | 6.81 | 17.65–18.27 | 2.6x | 21 | 107 | 195 | 198–211 |

Startup, process start to the first `list_full` frame: Metal 34–42 ms (one run of
987 ms left out as an outlier), wgpu 36–39 ms; footprint 22 MB against 24–25 MB.

## What it shows

- **wgpu adds 17–34% to most frames** and about 74 allocations a frame. `list_full`
  and `resize_full` are the exceptions at 1–3%: their cost is layout, not drawing.
- **Paths cost 2.6x more on wgpu.** The Metal backend's path caching has no wgpu
  equivalent yet.
- **The wgpu footprint is 1.1–1.5x Metal's.** An earlier version of this bench let
  wgpu frames queue without the three-frame limit and reported 1.5–3x, varying
  run to run; that was the backlog, not the backend.
- **Startup is level.**
- **Against the other libraries, with RUI on wgpu as well:**
  - `workbench_pane` and `workbench_full` are 4.0 M and 4.4 M, still under half of
    GPUI's 8.27 M and 10.34 M.
  - egui leads `list_full` by about 5% (16.6 M against 17.6 M).
  - RUI still leads `text_paragraphs` (4.5 M against 5.4 M) and `quad_grid`
    (7.9 M against 23.1 M).
- **So drawing through Metal directly accounts for part of RUI's lead, not most of it,**
  except on paths, where the wgpu backend loses the margin.
