# RUI

**A GPU-accelerated UI framework for Rust**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Test Coverage](https://img.shields.io/badge/coverage-66.23%25-green.svg)]()
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)]()

RUI is a high-performance UI framework inspired by [GPUI](https://github.com/zed-industries/zed) and [Warp](https://www.warp.dev/). It renders directly to the GPU using Metal (macOS) for smooth 120fps rendering.

## Features

- **GPU-Accelerated Rendering** - Metal-based rendering on macOS for buttery smooth 120fps
- **Declarative API** - Build UIs with a clean, chainable builder pattern
- **Flexbox Layout** - Powered by [Taffy](https://github.com/DioxusLabs/taffy) for familiar CSS-like layouts
- **React-like Hooks** - `use_mouse`, `use_paste`, `use_window_focus` and more
- **Rich Component Library** - Div, Text, Button, Input, Image, Table, List, Progress, Spinner
- **Animation System** - Built-in support for smooth animations with easing functions

## Quick Start

Add RUI to your `Cargo.toml`:

```toml
[dependencies]
rui = { git = "https://github.com/majiayu000/rui" }
```

Create your first app:

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

Run the example:

```bash
cargo run --example hello_world
```

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

```rust
use rui::prelude::*;

fn main() {
    App::new().run(|cx| {
        div()
            .w(400.0)
            .h(300.0)
            .bg(Color::hex(0x2d3436))
            .flex_col()
            .items_center()
            .justify_center()
            .gap(24.0)
            .child(
                text("Counter")
                    .size(32.0)
                    .bold()
                    .color(Color::WHITE)
            )
            .child(
                div()
                    .flex_row()
                    .gap(16.0)
                    .child(button("-").bg(Color::hex(0xd63031)))
                    .child(button("+").bg(Color::hex(0x00b894)))
            )
    });
}

fn button(label: &str) -> Button {
    button(label)
        .w(60.0)
        .h(60.0)
        .rounded(30.0)
}
```

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

RUI follows a three-layer architecture:

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
| `Button` | Interactive button with variants |
| `Input` | Text input field |
| `Image` | Image display with fit modes |
| `Table` | Table layout with rows/cells |
| `List` | Ordered/unordered lists |
| `Progress` | Progress bar |
| `Spinner` | Loading spinner |

### Hooks Module

| Hook | Description |
|------|-------------|
| `use_mouse` | Track mouse events and position |
| `use_paste` | Handle clipboard paste events |
| `use_window_focus` | Detect window focus changes |

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
| Windows | Planned | Vulkan/DX12 |
| Linux | Planned | Vulkan |
| Web | Planned | WebGPU |

## Running Examples

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

## Requirements

- Rust 1.75 or later
- macOS 10.15+ (Catalina or later)
- Metal-compatible GPU

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run tests with coverage
cargo tarpaulin --out Html

# Format code
cargo fmt

# Lint
cargo clippy
```

## Test Coverage

Current coverage: **66.23%** (1208 tests)

| Module | Coverage |
|--------|----------|
| core/color | 100% |
| core/entity | 100% |
| core/geometry | 100% |
| core/style | 100% |
| core/animation | 95% |
| elements/* | 60-80% |

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- [GPUI](https://github.com/zed-industries/zed) - Inspiration for the architecture
- [Taffy](https://github.com/DioxusLabs/taffy) - Flexbox layout engine
- [Warp](https://www.warp.dev/) - Inspiration for GPU-accelerated terminal UI

---

Built with Rust
