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
frame, and waits once three frames are in flight, as the Metal side's
`render_offscreen` does. wgpu commits several command buffers a frame; its GPU time is
their sum, as on egui's side.

## Results (RUI `main` 29544b1, 2026-11-26)

Apple M2 Max, macOS 27, three runs per case, each in its own process, load average
26–39. Instructions and allocations are the numbers to compare.

| Case | Metal instructions (M) | wgpu instructions (M) | wgpu / Metal | Metal allocations | wgpu allocations | Metal footprint (MB) | wgpu footprint (MB) |
|---|---|---|---|---|---|---|---|
| list_full | 17.06–17.07 | 17.51 | 1.03x | 4 077 | 4 151 | 59–60 | 66–67 |
| list_virtual_1k | 0.77 | 1.09 | 1.42x | 151 | 224 | 48 | 65 |
| text_paragraphs | 3.75 | 4.40–4.42 | 1.17x | 898 | 972 | 55 | 61 |
| quad_grid | 5.89 | 7.82–7.83 | 1.33x | 16 | 90 | 57 | 76–80 |
| resize_full | 39.32 | 39.49–39.50 | 1.00x | 16 088 | 16 162 | 58–60 | 86 |
| editor_scroll | 2.49–2.50 | 2.88–2.90 | 1.16x | 677 | 751 | 60 | 76 |
| terminal_rows | 3.15–3.16 | 3.63–3.68 | 1.16x | 885 | 958–959 | 55–56 | 62–63 |
| workbench_pane | 3.41–3.42 | 3.97–4.00 | 1.17x | 893 | 966–967 | 55 | 61 |
| workbench_full | 3.77 | 4.34–4.35 | 1.15x | 1 082 | 1 155 | 52–55 | 60–61 |
| paths | 6.81 | 18.13–18.19 | 2.67x | 21 | 107 | 195–196 | 198–208 |

Startup, process start to the first `list_full` frame: Metal 37–42 ms, wgpu 36–44 ms;
footprint 22–23 MB against 24 MB.

## What it shows

- **wgpu adds 15–33% to most frames** and about 74 allocations a frame. `list_full`
  and `resize_full` are the exceptions at 0–3%: their cost is layout, not drawing.
- **Paths cost 2.7x more on wgpu.** The Metal backend's path caching has no wgpu
  equivalent yet.
- **The wgpu footprint is 1.1–1.5x Metal's.** An earlier version of this bench let
  wgpu frames queue without the three-frame limit and reported 1.5–3x, varying
  run to run; that was the backlog, not the backend.
- **Startup is level.**
- **Against the other libraries, with RUI on wgpu as well:**
  - `workbench_pane` and `workbench_full` are 4.0 M and 4.3 M, still under half of
    GPUI's 8.27 M and 10.34 M.
  - egui leads `list_full` by about 5% (16.6 M against 17.5 M).
  - RUI still leads `text_paragraphs` (4.4 M against 5.4 M) and `quad_grid`
    (7.8 M against 23.1 M).
- **So drawing through Metal directly accounts for part of RUI's lead, not most of it,**
  except on paths, where the wgpu backend loses the margin.
