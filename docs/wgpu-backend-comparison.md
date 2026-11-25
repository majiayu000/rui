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
The wgpu side draws into a kept offscreen texture and redraws all of it each frame, as
the Metal side's `render_offscreen` does. wgpu commits several command buffers a
frame; its GPU time is their sum, as on egui's side.

## Results (RUI `main` 29544b1, 2026-11-26)

Apple M2 Max, macOS 27, three runs per case, each in its own process. The machine was
compiling at the same time (load average 130–185), so compare instructions and
allocations; GPU time and startup moved by up to 2x between runs.

| Case | Metal instructions (M) | wgpu instructions (M) | wgpu / Metal | Metal allocations | wgpu allocations | Metal footprint (MB) | wgpu footprint (MB) |
|---|---|---|---|---|---|---|---|
| list_full | 17.07–17.12 | 17.51–17.61 | 1.03x | 4 077 | 4 148 | 57–62 | 60–76 |
| list_virtual_1k | 0.76–0.78 | 1.20–2.38 | 1.6–3.1x | 151 | 222–229 | 49 | 143–164 |
| text_paragraphs | 3.75–3.77 | 4.42–4.49 | 1.18x | 898 | 969 | 55 | 92–96 |
| quad_grid | 5.88–5.92 | 7.83–8.72 | 1.33–1.47x | 16 | 87–88 | 58–59 | 101–183 |
| editor_scroll | 2.46–2.52 | 2.90–3.08 | 1.18–1.22x | 677 | 749–751 | 60 | 105–158 |
| terminal_rows | 3.16–3.17 | 3.80–4.12 | 1.20–1.30x | 885 | 956–957 | 55–56 | 117–135 |
| workbench_pane | 3.42–3.46 | 4.01–4.11 | 1.17–1.19x | 893 | 964 | 55–56 | 69–90 |
| workbench_full | 3.77–3.79 | 4.40–4.59 | 1.17–1.21x | 1 082 | 1 153 | 55–57 | 64–116 |
| paths | 6.81–6.88 | 17.58–18.24 | 2.6x | 21 | 104 | 180–196 | 197–218 |

Startup, process start to the first `list_full` frame: Metal 40–125 ms, wgpu 43–69 ms,
footprint 21–23 MB against 24 MB. Too noisy under this load to call.

## What it shows

- **wgpu adds 17–33% to most frames** and about 70 allocations a frame. `list_full` is
  the exception at 3%: its cost is layout, not drawing.
- **Paths cost 2.6x more on wgpu.** The Metal backend's path caching has no wgpu
  equivalent yet.
- **The wgpu footprint is 1.5–3x Metal's and varies run to run** (up to 183 MB on
  `quad_grid`). That points at memory wgpu or RUI's wgpu encoder keeps, and is worth
  its own look.
- **Against the other libraries, with RUI on wgpu as well:**
  - `workbench_pane` and `workbench_full` are 4.0–4.6 M, still under half of
    GPUI's 8.27 M and 10.34 M.
  - egui leads `list_full` by about 5% (16.6 M against 17.5 M).
  - RUI still leads `text_paragraphs` (4.4 M against 5.4 M) and `quad_grid`
    (7.9 M against 23.1 M).
- **So drawing through Metal directly accounts for part of RUI's lead, not most of it,**
  except on paths and virtual lists, where the wgpu backend loses most of the margin.
