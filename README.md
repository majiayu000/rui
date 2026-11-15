# RUI

**An early-stage GPU-accelerated Rust UI framework for macOS**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](#requirements)

RUI helps you build native macOS interfaces in Rust with declarative components, flexbox layout, and Metal GPU rendering. Its design is inspired by [GPUI](https://github.com/zed-industries/zed) and [Warp](https://www.warp.dev/).

[Install and quick start](#quick-start) · [API reference](docs/API.md) · [Examples guide](docs/EXAMPLES.md) · [Status and limitations](#status-and-limitations)

## Features

- **GPU-Accelerated Rendering** - Metal-based rendering on macOS
- **Declarative API** - Build UIs with a clean, chainable builder pattern
- **Flexbox Layout** - Powered by [Taffy](https://github.com/DioxusLabs/taffy) for familiar CSS-like layouts
- **One Widget Per Role** - Div, Text, Button, Input, ScrollView, Image, Table, List, Spinner, plus themed controls in `advanced_ui`
- **Animation System** - Built-in support for smooth animations with easing functions

## Status and Limitations

RUI is an early-stage, pre-1.0 framework. APIs, component behavior, and renderer internals may change before the first stable release.

- macOS is the supported runtime platform today, using the Metal renderer.
- Linux and Windows have an experimental runner (feature `winit`: a winit window drawn by the wgpu backend). It opens a single window with pointer, keyboard, scroll, resize and DPI input; IME composition, accessibility, the system clipboard, menus, file drops and extra windows are macOS-only so far. Web is a roadmap item.
- Accessibility semantics and advanced component interactions are still active development areas.
- Visual behavior should be verified on a local macOS desktop session; CI covers compile and automated tests, but not full end-to-end visual QA.
- The recommended install path is the Git dependency shown below until crates.io package ownership or naming is resolved.

## Quick Start

### Build your own macOS app

Start with a Rust binary project:

```bash
cargo new rui-hello
cd rui-hello
```

Add this repository as a Git dependency in `Cargo.toml`:

```toml
[dependencies]
rui = { git = "https://github.com/majiayu000/rui", rev = "1311a1d6b98a601de603d215ebc1d21f408fd7a3" }
```

This pins the source baseline checked by both installation guides, including the listener and native input changes; it is not a new release. This is `majiayu000/rui`. The [separate `audulus/rui` project](https://github.com/audulus/rui) also uses the name `rui`; installing `rui` from crates.io does not select this repository. Use the Git URL above for the API shown here.

Replace `src/main.rs` with:

```rust
use rui::prelude::*;

fn main() {
    App::new().run(|_cx| {
        div()
            .w(400.0)
            .h(300.0)
            .bg(Color::hex(0x1a1a2e))
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                text("Hello, RUI!")
                    .size(32.0)
                    .bold()
                    .color(Color::WHITE)
            )
    });
}
```

Run your application from the `rui-hello` directory:

```bash
cargo run
```

### Try the repository examples

The example targets belong to the RUI repository, so clone it first:

```bash
git clone https://github.com/majiayu000/rui.git
cd rui
cargo run --example hello_world
```

Continue with the [examples guide](docs/EXAMPLES.md) for component demos and the [API reference](docs/API.md) for builders.

### First-run questions

- **`no example target named hello_world`?** Run the example command inside the cloned RUI repository. In your own project, use `cargo run` for `src/main.rs`.
- **Can this run on Linux, Windows, or in a headless job?** Experimentally: `cargo run --example hello_world --no-default-features --features winit` uses the winit runner and the wgpu backend (Vulkan or DX12). The same command on macOS runs it on Metal through winit, for testing the runner. CI opens `hello_world` and `counter` this way on Linux under Xvfb and lavapipe, clicks, presses keys and resizes them with xdotool, and compares screenshots of the windows with `tests/linux_window/` (`scripts/linux_window_pixels.sh`; `RUI_PIXELS_UPDATE=1` rewrites the references). On X11, winit loads `libxkbcommon-x11` and the Xcursor, Xrandr and Xi libraries at runtime.
- **Does a successful compile prove the UI works?** It checks types and dependencies. Inspect the actual window and interactions separately; [status and limitations](#status-and-limitations) explains the current scope.

## Examples

### Hello World

```rust
use rui::prelude::*;

fn main() {
    App::new().run(|_cx| {
        div()
            .w(800.0)
            .h(600.0)
            .bg(Color::hex(0x1a1a2e))
            .flex_col()
            .items_center()
            .justify_center()
            .gap(20.0)
            .child(
                div()
                    .bg(Color::hex(0x16213e))
                    .rounded(16.0)
                    .p(32.0)
                    .shadow_lg()
                    .child(
                        text("Hello, RUI!")
                            .size(48.0)
                            .bold()
                            .color(Color::hex(0xe94560))
                    )
            )
    });
}
```

### Counter with Buttons

Run the stateful [counter example](examples/counter.rs):

```bash
cargo run --example counter
```

It displays the current count and attaches `on_click` handlers to both buttons.
Each handler uses `cx.listener` to update ordinary view state; the owning view
then rebuilds automatically. See [Getting started with state and events](docs/getting-started.md)
for a complete application and text/entity update examples.

### Feature Cards

```rust
fn feature_card(title: &str, description: &str, color: u32) -> Div {
    div()
        .w(180.0)
        .bg(Color::hex(color))
        .rounded(12.0)
        .p(20.0)
        .flex_col()
        .gap(8.0)
        .child(
            text(title)
                .size(20.0)
                .bold()
                .color(Color::WHITE)
        )
        .child(
            text(description)
                .size(14.0)
                .color(Color::rgba(1.0, 1.0, 1.0, 0.8))
        )
}
```

## Architecture

RUI follows a native runtime architecture: `AppContext` owns framework state,
`View` values build transient element trees, elements produce layout and scene
output, renderers consume backend-neutral scenes, and platform adapters own
native windows and renderer attachments. See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for how the
frame pipeline, incremental core, renderer and platform fit together.

```
┌─────────────────────────────────────────────────────────────┐
│                      Application Layer                       │
│  ┌─────────────────────────────────────────────────────┐    │
│  │   App::new().run(|cx| { ... })                      │    │
│  └─────────────────────────────────────────────────────┘    │
├─────────────────────────────────────────────────────────────┤
│                       Element Layer                          │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐           │
│  │   Div   │ │  Text   │ │ Button  │ │  Image  │  ...      │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘           │
├─────────────────────────────────────────────────────────────┤
│                        Core Layer                            │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐       │
│  │ Geometry │ │  Color   │ │  Style   │ │Animation │       │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘       │
├─────────────────────────────────────────────────────────────┤
│                      Renderer Layer                          │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              Metal GPU Renderer (macOS)              │    │
│  └─────────────────────────────────────────────────────┘    │
├─────────────────────────────────────────────────────────────┤
│                      Platform Layer                          │
│  Native window, events, clipboard, DPI, renderer target       │
└─────────────────────────────────────────────────────────────┘
```

### Core Module

- **Geometry** - `Point`, `Size`, `Bounds`, `Rect`, `Edges`
- **Color** - `Color`, `Rgba`, `Hsla` with hex, RGB, HSL support
- **Style** - Flexbox styles, borders, shadows, corners
- **Animation** - Easing functions, transitions, keyframes
- **Entity** - Entity system for managing UI elements

### Element Module

| Element | Description |
|---------|-------------|
| `Div` | Flexbox container element |
| `Text` | Text rendering with styling |
| `Button` | Themed, accessible button (`advanced_ui::Button`, in the prelude) |
| `Input` | Text input field, optionally themed |
| `ScrollView` | Clipped scroll area with accessibility |
| `Image` | Image display with fit modes |
| `Table` | Table layout with rows/cells |
| `List` | Ordered/unordered lists |
| `Spinner` | Loading spinner |

Themed controls (`Checkbox`, `ProgressBar`, `Tabs`, `Menu`, `Dialog` and
more) live in `rui::advanced_ui`. See [One Widget Per Role](docs/API.md#one-widget-per-role)
for which widget covers each role.

## Styling

RUI uses a chainable builder pattern for styling:

```rust
div()
    // Size
    .w(200.0)           // width
    .h(100.0)           // height
    .size(Size::new(200.0, 100.0))

    // Background
    .bg(Color::RED)
    .bg(Color::hex(0xFF5733))
    .bg(Color::rgba(1.0, 0.0, 0.0, 0.5))

    // Layout
    .flex_row()         // horizontal flex
    .flex_col()         // vertical flex
    .items_center()     // align items center
    .justify_center()   // justify content center
    .gap(16.0)          // gap between children

    // Spacing
    .p(16.0)            // padding all sides
    .px(8.0)            // horizontal padding
    .py(12.0)           // vertical padding
    .m(8.0)             // margin all sides

    // Border
    .rounded(8.0)       // border radius
    .rounded_full()     // circular
    .border(1.0, Color::GRAY)

    // Effects
    .shadow_sm()
    .shadow_md()
    .shadow_lg()
    .opacity(0.8)
```

## Color System

```rust
// Predefined colors
Color::WHITE
Color::BLACK
Color::RED
Color::GREEN
Color::BLUE
Color::TRANSPARENT

// Hex colors
Color::hex(0xFF5733)
Color::hex(0x1a1a2e)

// RGB/RGBA
Color::rgb(1.0, 0.5, 0.0)
Color::rgba(1.0, 0.5, 0.0, 0.8)

// HSL/HSLA
Color::hsl(180.0, 0.5, 0.5)
Color::hsla(180.0, 0.5, 0.5, 0.8)
```

## Platform Support

| Platform | Status | Renderer |
|----------|--------|----------|
| macOS | Supported | Metal |
| Windows | Experimental (feature `winit`) | wgpu: DX12/Vulkan |
| Linux | Experimental (feature `winit`) | wgpu: Vulkan |
| Web | Planned | WebGPU |

## Release Status

- The manifest still says `0.2.5`; that number alone does not identify the current source snapshot.
- The `v0.2.5` tag predates the native IME and font-cache changes listed under Unreleased. Use the pinned Git revision above for those APIs, not the old tag.
- Local changes on top of that revision are review candidates until committed and published; this README does not announce a new release.
- The crates.io package named `rui` belongs to the separate `audulus/rui` project. Install this project from its Git URL and pin the intended commit.
- Release history and unreleased launch-readiness changes are tracked in [CHANGELOG.md](CHANGELOG.md).

## Running Examples

Build the project with `cargo build`, then run examples with `cargo run --example <name>` from the repository root on macOS.

```bash
# Hello World - Basic layout
cargo run --example hello_world

# Counter - Interactive buttons
cargo run --example counter

# Dashboard - Complex layout
cargo run --example dashboard

# Animation Demo - Animations
cargo run --example animation_demo
```

### Local Native Dogfood

Most validation is CI-safe and finite: `cargo check`, `cargo test`, `cargo test example_smoke`, `cargo test dogfood`, and `cargo test --test docs_api_drift`.

The native macOS smoke is local-only because it opens a real window and injects framework key events into an input nested inside a dialog, requests and confirms AppKit minimize/restore states, writes `RUI_NATIVE_DOGFOOD_PROFILE`, captures and validates renderer frame telemetry from `RUI_PROFILE`, and exits:

```bash
scripts/native_dogfood_macos.sh
```

The command writes the interaction result to
`target/rui-native-dogfood-profile.json`, renderer JSONL to
`target/rui-native-dogfood-renderer-profile.jsonl`, and the combined process
log to `target/rui-native-dogfood.log`. Override the renderer artifact with
`RUI_NATIVE_DOGFOOD_RENDERER_PROFILE` when another location is needed.
The process log records `native dogfood: AppKit minimized=true` and
`native dogfood: AppKit restored=true` only after observing those native states.

This is a repository-owned acceptance example, not evidence of an external
application using RUI. The automated ASCII smoke does not exercise a Chinese
input method's candidate window or VoiceOver. For manual acceptance, launch:

```bash
RUI_NATIVE_DOGFOOD_INTERACTIVE=1 \
RUI_NATIVE_DOGFOOD_TEXT='你好世界' \
RUI_NATIVE_DOGFOOD_PROFILE=target/rui-native-manual.json \
cargo run --example native_dogfood
```

Using your configured Chinese input method, compose and cancel once, then commit
`你好世界`. With VoiceOver enabled by the tester, check the input label, focus,
value and editing; minimize/restore the window and press Return to finish.
Record the OS, input method, revision and observed results separately: the JSON
only records the final submitted text and does not certify these manual steps.
The same example nests the input inside a dialog, so its automated run covers
nested input routing in a native window. Automated dialog/popover and AX action
tests provide further protocol coverage; VoiceOver navigation and activation
still require the tester's observations.

## Requirements

- Rust 1.88 or later (checked with the committed Cargo.lock)
- macOS 10.15+ (Catalina or later)
- Metal-compatible GPU

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# API/docs drift check
cargo test --test docs_api_drift

# Headless examples and dogfood
cargo test example_smoke
cargo test dogfood

# Local-only native macOS dogfood
scripts/native_dogfood_macos.sh

# Run tests with coverage
cargo tarpaulin --out Html

# Format code
cargo fmt

# Lint
cargo clippy
```

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- [GPUI](https://github.com/zed-industries/zed) - Inspiration for the architecture
- [Taffy](https://github.com/DioxusLabs/taffy) - Flexbox layout engine
- [Warp](https://www.warp.dev/) - Inspiration for GPU-accelerated terminal UI

---

Built with Rust
