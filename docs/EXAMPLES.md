# RUI Examples Guide

This guide walks through the example applications included with RUI.

## Running Examples

```bash
# Clone the repository
git clone https://github.com/majiayu000/rui
cd rui

# Run an example
cargo run --example hello_world
cargo run --example counter
cargo run --example dashboard
cargo run --example animation_demo
```

---

## Hello World

**File:** `examples/hello_world.rs`

The simplest RUI application demonstrating basic layout and styling.

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
                    .flex_col()
                    .items_center()
                    .gap(16.0)
                    .child(
                        text("Hello, RUI!")
                            .size(48.0)
                            .bold()
                            .color(Color::hex(0xe94560))
                    )
                    .child(
                        text("A GPU-accelerated UI framework for Rust")
                            .size(18.0)
                            .color(Color::hex(0xeaeaea))
                    )
            )
            .child(
                div()
                    .flex_row()
                    .gap(16.0)
                    .child(feature_card("Fast", "Metal GPU rendering", 0x0f3460))
                    .child(feature_card("Flexible", "Declarative API", 0x533483))
                    .child(feature_card("Modern", "Rust powered", 0xe94560))
            )
    });
}

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

### Key Concepts Demonstrated

1. **App Creation** - `App::new().run(|cx| { ... })`
2. **Container Layout** - `div()` with flexbox
3. **Text Rendering** - `text()` with styling
4. **Reusable Components** - `feature_card()` function
5. **Color System** - Hex colors, RGBA
6. **Shadows** - `.shadow_lg()`
7. **Rounded Corners** - `.rounded(16.0)`

---

## Counter

**File:** `examples/counter.rs`

Interactive counter demonstrating buttons and layout.

```rust
use rui::prelude::*;

fn main() {
    App::new().run(|cx| {
        let count = 0i32;

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
                    .w(200.0)
                    .h(80.0)
                    .bg(Color::hex(0x636e72))
                    .rounded(12.0)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        text(format!("{}", count))
                            .size(48.0)
                            .bold()
                            .color(Color::WHITE)
                    )
            )
            .child(
                div()
                    .flex_row()
                    .gap(16.0)
                    .child(button("-", Color::hex(0xd63031)))
                    .child(button("+", Color::hex(0x00b894)))
            )
    });
}

fn button(label: &str, bg_color: Color) -> Div {
    div()
        .w(60.0)
        .h(60.0)
        .bg(bg_color)
        .rounded(30.0)
        .flex()
        .items_center()
        .justify_center()
        .shadow_md()
        .child(
            text(label)
                .size(32.0)
                .bold()
                .color(Color::WHITE)
        )
}
```

### Key Concepts Demonstrated

1. **Circular Buttons** - `.rounded(30.0)` on 60x60
2. **Dynamic Text** - `format!("{}", count)`
3. **Flex Row Layout** - Horizontal button arrangement
4. **Shadows** - `.shadow_md()`
5. **Color Theming** - Consistent color scheme

---

## Dashboard (Advanced Layout)

**File:** `examples/dashboard.rs`

Complex dashboard layout with multiple panels.

### Structure

```
┌─────────────────────────────────────────────────┐
│                    Header                        │
├───────────────┬─────────────────────────────────┤
│               │                                  │
│    Sidebar    │           Main Content           │
│               │                                  │
│   - Menu 1    │   ┌───────────┐ ┌───────────┐   │
│   - Menu 2    │   │  Card 1   │ │  Card 2   │   │
│   - Menu 3    │   └───────────┘ └───────────┘   │
│               │                                  │
└───────────────┴─────────────────────────────────┘
```

### Code Pattern

```rust
fn dashboard() -> Div {
    div()
        .w(1200.0)
        .h(800.0)
        .flex_col()
        // Header
        .child(header())
        // Main content area
        .child(
            div()
                .flex_row()
                .flex_grow(1.0)
                // Sidebar
                .child(sidebar())
                // Content
                .child(main_content())
        )
}

fn header() -> Div {
    div()
        .h(60.0)
        .bg(Color::hex(0x2d3436))
        .flex_row()
        .items_center()
        .px(24.0)
        .child(text("Dashboard").size(24.0).bold().color(Color::WHITE))
}

fn sidebar() -> Div {
    div()
        .w(240.0)
        .bg(Color::hex(0x636e72))
        .flex_col()
        .p(16.0)
        .gap(8.0)
        .child(menu_item("Overview"))
        .child(menu_item("Analytics"))
        .child(menu_item("Settings"))
}

fn main_content() -> Div {
    div()
        .flex_grow(1.0)
        .bg(Color::hex(0xdfe6e9))
        .p(24.0)
        .flex_row()
        .flex_wrap()
        .gap(16.0)
        .child(stat_card("Users", "1,234", Color::hex(0x0984e3)))
        .child(stat_card("Revenue", "$12,345", Color::hex(0x00b894)))
        .child(stat_card("Orders", "567", Color::hex(0xe17055)))
}
```

### Key Concepts Demonstrated

1. **Complex Layouts** - Header + Sidebar + Content
2. **Flex Grow** - `.flex_grow(1.0)` for responsive sizing
3. **Nested Components** - Functions returning `Div`
4. **Cards** - Stat cards with icons and values
5. **Flex Wrap** - Grid of cards that wrap

---

## Animation Demo

**File:** `examples/animation_demo.rs`

Demonstrates RUI's animation capabilities.

### Animation Types

```rust
// Fade animation
div()
    .opacity(animation.value())  // 0.0 to 1.0

// Scale animation
div()
    .w(100.0 * animation.value())
    .h(100.0 * animation.value())

// Position animation
div()
    .m(animation.value() * 100.0)

// Color animation
let color = Color::rgba(
    animation.value(),
    0.0,
    1.0 - animation.value(),
    1.0
);
```

### Easing Examples

```rust
// Linear - constant speed
Animation::new(Duration::from_millis(300))
    .easing(Easing::Linear)

// Ease Out - fast start, slow end
Animation::new(Duration::from_millis(300))
    .easing(Easing::EaseOutCubic)

// Bounce - elastic bounce effect
Animation::new(Duration::from_millis(500))
    .easing(Easing::EaseOutBounce)
```

---

## Creating Your Own App

### Step 1: Basic Structure

```rust
use rui::prelude::*;

fn main() {
    App::new().run(|cx| {
        app_root()
    });
}

fn app_root() -> Div {
    div()
        .w(800.0)
        .h(600.0)
        .bg(Color::hex(0x1a1a2e))
        .flex_col()
        .child(header())
        .child(content())
}
```

### Step 2: Add Components

```rust
fn header() -> Div {
    div()
        .h(60.0)
        .bg(Color::hex(0x16213e))
        .flex_row()
        .items_center()
        .px(20.0)
        .child(
            text("My App")
                .size(24.0)
                .bold()
                .color(Color::WHITE)
        )
}

fn content() -> Div {
    div()
        .flex_grow(1.0)
        .p(20.0)
        .child(
            text("Welcome to my app!")
                .size(18.0)
                .color(Color::hex(0xeaeaea))
        )
}
```

### Step 3: Add Interactivity

```rust
fn button_row() -> Div {
    div()
        .flex_row()
        .gap(12.0)
        .child(
            button("Primary")
                .variant(ButtonVariant::Primary)
                .on_click(|_| println!("Primary clicked"))
        )
        .child(
            button("Secondary")
                .variant(ButtonVariant::Secondary)
                .on_click(|_| println!("Secondary clicked"))
        )
}
```

### Step 4: Use Hooks

```rust
use rui::hooks::*;

fn interactive_area() -> Div {
    let mouse = UseMouse::new();

    mouse.on_click(|event| {
        println!("Clicked at: ({}, {})", event.column, event.row);
    });

    div()
        .w(400.0)
        .h(300.0)
        .bg(Color::hex(0x2d3436))
        .child(text("Click anywhere!"))
}
```

---

## Design Patterns

### Component Functions

Create reusable components as functions:

```rust
fn card(title: &str, content: &str) -> Div {
    div()
        .bg(Color::WHITE)
        .rounded(8.0)
        .shadow_md()
        .p(16.0)
        .flex_col()
        .gap(8.0)
        .child(
            text(title)
                .size(18.0)
                .bold()
                .color(Color::hex(0x2d3436))
        )
        .child(
            text(content)
                .size(14.0)
                .color(Color::hex(0x636e72))
        )
}
```

### Theming

Define colors as constants:

```rust
mod theme {
    use rui::prelude::Color;

    pub const BACKGROUND: u32 = 0x1a1a2e;
    pub const SURFACE: u32 = 0x16213e;
    pub const PRIMARY: u32 = 0xe94560;
    pub const TEXT: u32 = 0xeaeaea;
    pub const TEXT_DIM: u32 = 0x9e9e9e;
}

// Usage
.bg(Color::hex(theme::BACKGROUND))
.color(Color::hex(theme::PRIMARY))
```

### Responsive Layouts

Use flex properties for responsive behavior:

```rust
fn responsive_grid() -> Div {
    div()
        .flex_row()
        .flex_wrap()       // Allow wrapping
        .gap(16.0)
        .child(
            div()
                .min_w(200.0)  // Minimum width
                .flex_grow(1.0) // Grow to fill
                // ...
        )
}
```

---

## Troubleshooting

### Common Issues

1. **Black Screen**
   - Check that you're running on macOS with Metal support
   - Ensure the app has a root element with dimensions

2. **Layout Issues**
   - Verify flex direction (row vs column)
   - Check that parent containers have sizes
   - Use `.flex_grow(1.0)` for dynamic sizing

3. **Text Not Visible**
   - Ensure text color contrasts with background
   - Check that parent container is large enough

4. **Performance**
   - Keep element tree depth reasonable
   - Avoid creating new elements in hot loops
   - Use GPU-friendly colors (avoid gradients for now)
