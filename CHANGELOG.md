# Changelog

All notable project changes should be recorded here.

## Unreleased

## 0.3.0-rc.1

### Rendering

- Text can be shaped with OpenType feature settings:
  `TextRequest::with_font_features(&[FontFeature])` and the new
  `font_features: FontFeatures` field of `Primitive::Text`.
  `FontFeatures::no_ligatures()` turns `calt` and `liga` off, so JetBrains Mono
  draws `->` as `-` and `>`. The measure, wrap, glyph and raster caches and each
  face's shaping plans key on the list (#329).
  **Breaking:** `TextRequest` and `Primitive::Text` built with a struct literal
  need the new field (`font_features: &[]` / `Default::default()`).
- `rui::renderer::text::register_font(bytes)` and `register_font_file(path)`
  add the faces of a font file or collection, and text naming their family uses
  them. A family outside the built-in table is looked up among the installed
  fonts (macOS `/System/Library/Fonts`, `/Library/Fonts`, `~/Library/Fonts`;
  Linux `/usr/share/fonts`, `/usr/local/share/fonts`, `~/.local/share/fonts`,
  `~/.fonts`; Windows `Fonts` and the per-user fonts folder), scanned once the
  first time such a family is asked for, so Menlo or a user-installed Nerd Font
  now draw instead of falling back (#325).
- A font family with no loaded face draws in the default family (same weight
  and slant) instead of failing the frame with `RenderFailed` or panicking in
  layout. It is logged once and listed in the new
  `RendererDiagnostics::unknown_font_families` (#326).
  **Breaking:** code that builds `RendererDiagnostics` with a struct literal
  must add the field; `TextError::UnsupportedFontFamily` is no longer returned.
- `Painter::glyph` draws one glyph in a `canvas` at a pen position of the
  caller's choosing, from the text glyph atlas, without shaping. Characters are
  resolved once with `renderer::text::GlyphResolver` (falling back to other faces
  as text does) into `FontGlyph`s, which a grid keeps per cell: editing a cell
  shapes nothing, and wide or fallback glyphs stay on the cells the grid gives
  them instead of pushing later columns off it. `examples/canvas_grid.rs` now
  draws its 200x60 grid this way, and `stress_frame` gains `grid_*` stages for it
  (#176).
- **Breaking:** `Primitive` and `PrimitiveKind` gain a `Glyph` variant, so
  exhaustive matches on them need an arm for it.
- **Breaking:** `font_size` is the em size, as in CSS, Core Text and GPUI. It
  was the ascent-to-descent height, which is 1.0 to 1.4 em depending on the font,
  so the same number drew text up to 29% smaller than other toolkits and changed
  size when the font changed. Text now draws larger at the same `font_size`
  (SF Mono at 18 advances `0` by 11.13 px, was 9.45). The factor is the font's
  ascent-to-descent height in ems: 1.0 for Helvetica, 1.12 for Arial (the macOS
  default), 1.18 for SF Mono, 1.16 for DejaVu Sans, about 1.33 for Segoe UI (the
  Windows default) and 1.36 for Noto Sans. To keep an old look, divide the old
  size by it and multiply the line height by it, since the line box is
  `font_size * line_height`. `TextArea` has no font size setter and draws at a
  fixed 14, so its text grows (#327).
- Underlines and strikethroughs of styled text are placed from the baseline
  each span is drawn on, so they no longer touch the glyphs at the em size.
  `RichPiece` gains a `baseline` field.
- `table()` measures its cells' text for automatic column widths instead of
  guessing from the byte count, so bold headers no longer run past the column
  border.
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

- The `image` dependency leaves out its `avif` feature, which only adds an AVIF
  encoder (`ravif`/`rav1e`) RUI never calls; every format RUI could decode
  before still decodes. A release build of the library compiles 27 fewer
  crates.
- System fonts are now parsed once per process and text measurement metrics
  survive across frames, instead of both being rebuilt every frame.
- A background an ancestor paints again because something inside it changed no
  longer puts its whole box in the frame's damage when it is drawn the same as
  in the frame before: with a root and a panel background, one changed row
  redraws 13% of a 320x600 window at 2x instead of all of it (#343).

### Runtime

- **Breaking:** `App::new()` leaves process-wide logger registration to the
  application. To use `RUST_LOG` with `env_logger`, add it to the binary's
  dependencies and call `env_logger::init()` in `main`.

- The library builds again with `--no-default-features` (no Metal, no wgpu),
  so the update-path tests `docs/getting-started.md` gives run again; since
  `Primitive::Picture` the `picture` module was left out of that build while
  the plan and primitives still used it. CI now runs that command.
- An entity one view's listener updates shows in the other views that read it
  on the next frame, parent to child and child to parent, instead of one frame
  later; child views' queued listener updates run before the frame works out
  what renders (#341).
- The winit runner (Linux, Windows) opens the windows app code opens with
  `AppContext::open_window` and `open_view_window` instead of closing them:
  each gets its own presenter, surface and renderer, events go to the window
  winit names, and closing one leaves the others running. Its window reports
  the `multi-window` feature.
- The winit runner (Linux and Windows) publishes each window's accessibility
  tree through AccessKit: UI Automation on Windows, AT-SPI on Linux. It is the
  tree `Presenter::accessibility_tree` builds for the macOS runner, with roles,
  names, values, checked and selected state, ranges, bounds and focus, and a
  text input's value as text runs with its caret. Press, focus, set value,
  increment, decrement and scroll requests come back through the same dispatch
  the macOS runner uses. The tree is built only while an assistive technology
  reads it. Not yet: announcements of action feedback, and text editing beyond
  replacing the value (#344).
- CI reads the winit runner's accessibility tree back over AT-SPI under
  Wayland (a headless sway) as well as X11, with the same checks
  (`scripts/wayland_atspi_tree.sh`). AT-SPI goes over D-Bus, so the tree,
  focus and actions work the same; positions are reported relative to the
  window, since a Wayland client cannot learn where its window is on screen
  (#344).
- The winit runner (Linux and Windows) copies, cuts and pastes through the
  system clipboard via arboard, X11 or Wayland on Linux, instead of an
  in-memory clipboard only the app could see. Without a system clipboard it
  logs an error and keeps the in-memory one.
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
- `testing::mount`, `mount_view` and `HeadlessApp` windows render the child
  views their first tree places (`view(..)`, `cx.rows(..)`) before laying it
  out, as native windows do. They used to panic with "view ... was laid out
  before it rendered". The first frame still builds the root once, also when
  entities were notified before a headless window opened.
- A window that loses key status (switching windows or apps) now blurs its
  focused element and focuses it again when it becomes key, on macOS and
  winit. Before, focus was dropped: after switching back, Tab started over,
  Enter on a focused button did nothing, and an input kept its ring and took
  keys while nothing owned focus. Focus is not given back when app code moved
  or cleared it meanwhile, or the element was removed, disabled or is behind a
  modal that opened. `HeadlessSession::dispatch_window_event` with
  `Event::Blur` / `Event::Focus` and `HeadlessApp::make_key_window` drive the
  same path in tests.

### Advanced UI

- `Toaster` / `Toast` show transient info, success, warning and error
  notifications stacked in a window corner, closing after a duration that
  pauses while hovered, with close and action buttons. Toasts are
  `AccessibilityRole::Status` live regions announced when they appear.
- `date_picker` is a date field opening a month calendar in an overlay with
  keyboard navigation, min/max and disabled dates; the calendar is exposed as
  an `AccessibilityRole::Grid` of `GridCell`s. `CalendarDate` does the date
  math; "today" is the local date (`localtime_r` on unix).
- `AppContext::request_frame_at` / `ViewContext::request_frame_at` render a
  view once a time has passed; the macOS loop and the winit runner sleep until
  the earliest one instead of drawing every refresh. Toasts use it to close.
- **Breaking:** `ThemeColors` gains a `warning` colour, and
  `AccessibilityRole` gains `Grid`, `GridCell` and `Status`.

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
