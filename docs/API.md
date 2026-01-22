# RUI API Reference

Complete API documentation for the RUI framework.

## Table of Contents

- [Core Types](#core-types)
- [Elements](#elements)
- [Hooks](#hooks)
- [Styling](#styling)
- [Colors](#colors)
- [Animation](#animation)

---

## Core Types

### App

Application entry point and lifecycle management.

```rust
use rui::prelude::*;

// Create and run application
App::new().run(|cx: &mut AppContext| {
    div().child(text("Hello"))
});
```

#### Methods

| Method | Description |
|--------|-------------|
| `new()` | Create new application instance |
| `run(callback)` | Start the application with a root view builder |

---

### Geometry Types

#### Point

2D point with x and y coordinates.

```rust
let p = Point::new(10.0, 20.0);
let origin = Point::ZERO;
```

| Method | Description |
|--------|-------------|
| `new(x, y)` | Create point at (x, y) |
| `ZERO` | Point at origin (0, 0) |
| `distance_to(other)` | Euclidean distance to another point |

#### Size

2D size with width and height.

```rust
let size = Size::new(100.0, 50.0);
let square = Size::square(100.0);
```

| Method | Description |
|--------|-------------|
| `new(width, height)` | Create size |
| `square(side)` | Create square size |
| `ZERO` | Zero size |
| `area()` | Calculate area |

#### Bounds

Rectangle defined by origin point and size.

```rust
let bounds = Bounds::new(Point::new(10.0, 10.0), Size::new(100.0, 50.0));
let bounds = Bounds::from_xywh(10.0, 10.0, 100.0, 50.0);
```

| Method | Description |
|--------|-------------|
| `new(origin, size)` | Create from point and size |
| `from_xywh(x, y, w, h)` | Create from coordinates |
| `x()`, `y()` | Get origin coordinates |
| `width()`, `height()` | Get dimensions |
| `center()` | Get center point |
| `contains(point)` | Check if point is inside |
| `intersects(other)` | Check intersection |
| `union(other)` | Get bounding box of both |
| `intersection(other)` | Get overlapping area |

#### Edges

Four-sided values for padding, margin, border.

```rust
let edges = Edges::all(10.0);
let edges = Edges::xy(horizontal: 20.0, vertical: 10.0);
let edges = Edges::new(top: 10.0, right: 20.0, bottom: 10.0, left: 20.0);
```

| Method | Description |
|--------|-------------|
| `all(value)` | Same value all sides |
| `xy(x, y)` | Horizontal and vertical |
| `new(t, r, b, l)` | Individual sides |
| `ZERO` | No spacing |

---

## Elements

### Div

Flexbox container element. The primary layout building block.

```rust
div()
    .w(200.0)
    .h(100.0)
    .bg(Color::BLUE)
    .flex_col()
    .items_center()
    .justify_center()
    .gap(16.0)
    .child(text("Child 1"))
    .child(text("Child 2"))
```

#### Size Methods

| Method | Description |
|--------|-------------|
| `.w(width)` | Set width |
| `.h(height)` | Set height |
| `.size(Size)` | Set both dimensions |
| `.min_w(width)` | Minimum width |
| `.min_h(height)` | Minimum height |
| `.max_w(width)` | Maximum width |
| `.max_h(height)` | Maximum height |

#### Background & Border

| Method | Description |
|--------|-------------|
| `.bg(Color)` | Background color |
| `.rounded(radius)` | Border radius all corners |
| `.rounded_full()` | Circular (radius = 9999) |
| `.border(width, color)` | Add border |
| `.shadow_sm()` | Small shadow |
| `.shadow_md()` | Medium shadow |
| `.shadow_lg()` | Large shadow |
| `.opacity(value)` | Set opacity (0.0-1.0) |

#### Flexbox Layout

| Method | Description |
|--------|-------------|
| `.flex()` | Enable flex |
| `.flex_row()` | Horizontal direction |
| `.flex_col()` | Vertical direction |
| `.flex_wrap()` | Enable wrapping |
| `.flex_grow(value)` | Flex grow factor |
| `.flex_shrink(value)` | Flex shrink factor |

#### Alignment

| Method | Description |
|--------|-------------|
| `.items_start()` | Align items to start |
| `.items_center()` | Align items to center |
| `.items_end()` | Align items to end |
| `.items_stretch()` | Stretch items |
| `.justify_start()` | Justify to start |
| `.justify_center()` | Justify to center |
| `.justify_end()` | Justify to end |
| `.justify_between()` | Space between |
| `.justify_around()` | Space around |
| `.justify_evenly()` | Space evenly |

#### Spacing

| Method | Description |
|--------|-------------|
| `.gap(value)` | Gap between children |
| `.gap_x(value)` | Horizontal gap |
| `.gap_y(value)` | Vertical gap |
| `.p(value)` | Padding all sides |
| `.px(value)` | Horizontal padding |
| `.py(value)` | Vertical padding |
| `.pt(value)` | Top padding |
| `.pr(value)` | Right padding |
| `.pb(value)` | Bottom padding |
| `.pl(value)` | Left padding |
| `.m(value)` | Margin all sides |
| `.mx(value)` | Horizontal margin |
| `.my(value)` | Vertical margin |

#### Children

| Method | Description |
|--------|-------------|
| `.child(element)` | Add single child |
| `.children(vec)` | Add multiple children |

---

### Text

Text rendering element with styling.

```rust
text("Hello, World!")
    .size(24.0)
    .bold()
    .italic()
    .color(Color::WHITE)
    .align(TextAlign::Center)
```

#### Methods

| Method | Description |
|--------|-------------|
| `text(content)` | Create text element |
| `.size(pixels)` | Font size |
| `.bold()` | Bold weight |
| `.italic()` | Italic style |
| `.underline()` | Underlined text |
| `.strikethrough()` | Strikethrough |
| `.color(Color)` | Text color |
| `.align(TextAlign)` | Text alignment |
| `.line_height(value)` | Line height multiplier |
| `.font_weight(weight)` | Numeric weight (100-900) |
| `.font_family(name)` | Font family name |

#### TextAlign

```rust
pub enum TextAlign {
    Left,
    Center,
    Right,
}
```

---

### Button

Interactive button element.

```rust
button("Click Me")
    .variant(ButtonVariant::Primary)
    .size(ButtonSize::Medium)
    .on_click(|_| println!("Clicked!"))
```

#### Methods

| Method | Description |
|--------|-------------|
| `button(label)` | Create button |
| `.variant(ButtonVariant)` | Button style variant |
| `.size(ButtonSize)` | Button size preset |
| `.on_click(handler)` | Click handler |
| `.disabled(bool)` | Disable button |

#### ButtonVariant

```rust
pub enum ButtonVariant {
    Primary,
    Secondary,
    Outline,
    Ghost,
    Destructive,
}
```

#### ButtonSize

```rust
pub enum ButtonSize {
    Small,
    Medium,
    Large,
}
```

---

### Input

Text input field.

```rust
input()
    .placeholder("Enter text...")
    .input_type(InputType::Password)
    .on_change(|value| println!("Value: {}", value))
```

#### Methods

| Method | Description |
|--------|-------------|
| `input()` | Create input |
| `.value(String)` | Set value |
| `.placeholder(text)` | Placeholder text |
| `.input_type(InputType)` | Input type |
| `.on_change(handler)` | Change handler |
| `.on_submit(handler)` | Submit handler |
| `.disabled(bool)` | Disable input |
| `.max_length(n)` | Maximum characters |

#### InputType

```rust
pub enum InputType {
    Text,
    Password,
    Email,
    Number,
    Search,
}
```

---

### Image

Image display element.

```rust
image("path/to/image.png")
    .w(200.0)
    .h(150.0)
    .fit(ImageFit::Cover)
    .rounded(8.0)
    .alt("Description")
```

#### Constructors

| Function | Description |
|----------|-------------|
| `image(path)` | From file path |
| `image_url(url)` | From URL |
| `Image::from_data(data, w, h)` | From raw pixels |
| `Image::from_texture(id)` | From GPU texture |

#### Methods

| Method | Description |
|--------|-------------|
| `.fit(ImageFit)` | Scaling mode |
| `.cover()` | Cover container |
| `.contain()` | Contain in container |
| `.fill()` | Stretch to fill |
| `.alt(text)` | Alt text |
| `.placeholder(Color)` | Loading placeholder |
| `.on_load(handler)` | Load complete handler |
| `.on_error(handler)` | Error handler |

#### ImageFit

```rust
pub enum ImageFit {
    Cover,      // Scale to fill, may crop
    Contain,    // Scale to fit, may letterbox
    Fill,       // Stretch to fill exactly
    None,       // No scaling
    ScaleDown,  // Only scale down if needed
}
```

---

### Table

Table layout element.

```rust
table()
    .child(header_row()
        .child(cell("Name"))
        .child(cell("Age")))
    .child(row()
        .child(cell("Alice"))
        .child(cell("30")))
```

#### Components

| Function | Description |
|----------|-------------|
| `table()` | Create table |
| `row()` | Create data row |
| `header_row()` | Create header row |
| `cell(content)` | Create table cell |

---

### List

Ordered and unordered lists.

```rust
unordered_list()
    .child(ListItem::new("First item"))
    .child(ListItem::new("Second item"))

ordered_list()
    .style(ListStyle::Decimal)
    .child(ListItem::new("Step 1"))
    .child(ListItem::new("Step 2"))
```

#### ListStyle

```rust
pub enum ListStyle {
    Disc,       // Bullet points
    Circle,     // Hollow circles
    Square,     // Squares
    Decimal,    // 1, 2, 3
    Alpha,      // a, b, c
    Roman,      // i, ii, iii
}
```

---

### Progress

Progress bar element.

```rust
progress()
    .value(0.75)  // 75%
    .w(200.0)
    .h(8.0)
    .color(Color::GREEN)
```

#### Methods

| Method | Description |
|--------|-------------|
| `progress()` | Create progress bar |
| `.value(0.0-1.0)` | Progress value |
| `.color(Color)` | Bar color |
| `.track_color(Color)` | Track background |
| `.animated(bool)` | Animate changes |

---

### Spinner

Loading spinner element.

```rust
spinner()
    .spinner_type(SpinnerType::Dots)
    .size(24.0)
    .color(Color::BLUE)
```

#### SpinnerType

```rust
pub enum SpinnerType {
    Circular,
    Dots,
    Bars,
    Pulse,
}
```

---

## Hooks

### use_mouse

Track mouse events and position.

```rust
use rui::hooks::UseMouse;

let mouse = UseMouse::new();

mouse.on_move(|event| {
    println!("Mouse at: {:?}", event.position);
});

mouse.on_click(|event| {
    println!("Clicked: {:?}", event.button);
});
```

#### TerminalMouseEvent

```rust
pub struct TerminalMouseEvent {
    pub kind: TerminalMouseEventKind,
    pub button: TerminalMouseButton,
    pub column: u16,
    pub row: u16,
    pub modifiers: u8,
}
```

---

### use_paste

Handle clipboard paste events.

```rust
use rui::hooks::UsePaste;

let paste = UsePaste::new();

paste.on_paste(|event| {
    println!("Pasted: {}", event.content);
});
```

---

### use_window_focus

Detect window focus changes.

```rust
use rui::hooks::UseWindowFocus;

let focus = UseWindowFocus::new();

focus.on_focus(|_| println!("Window focused"));
focus.on_blur(|_| println!("Window blurred"));
```

---

## Colors

### Color

Color type with multiple constructors.

```rust
// Named colors
Color::WHITE
Color::BLACK
Color::RED
Color::GREEN
Color::BLUE
Color::TRANSPARENT

// Hex
Color::hex(0xFF5733)
Color::hex(0x1a1a2e)

// RGB (0.0 - 1.0)
Color::rgb(1.0, 0.5, 0.0)

// RGBA with alpha
Color::rgba(1.0, 0.5, 0.0, 0.8)

// HSL (hue: 0-360, s/l: 0.0-1.0)
Color::hsl(180.0, 0.5, 0.5)
Color::hsla(180.0, 0.5, 0.5, 0.8)
```

### Rgba

Raw RGBA values.

```rust
let rgba = Rgba::new(1.0, 0.0, 0.0, 1.0);  // Red
let rgba = Rgba::WHITE;
let rgba = Rgba::TRANSPARENT;
```

### Hsla

HSL color with alpha.

```rust
let hsla = Hsla::new(180.0, 0.5, 0.5, 1.0);  // Cyan
```

---

## Animation

### Easing Functions

```rust
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInQuart,
    EaseOutQuart,
    EaseInOutQuart,
    EaseInQuint,
    EaseOutQuint,
    EaseInOutQuint,
    EaseInSine,
    EaseOutSine,
    EaseInOutSine,
    EaseInExpo,
    EaseOutExpo,
    EaseInOutExpo,
    EaseInCirc,
    EaseOutCirc,
    EaseInOutCirc,
    EaseInBack,
    EaseOutBack,
    EaseInOutBack,
    EaseInElastic,
    EaseOutElastic,
    EaseInOutElastic,
    EaseInBounce,
    EaseOutBounce,
    EaseInOutBounce,
}
```

### Animation Struct

```rust
let animation = Animation::new(Duration::from_millis(300))
    .easing(Easing::EaseOutCubic)
    .delay(Duration::from_millis(100));

// Get interpolated value at time t
let value = animation.value_at(0.5);  // 0.0 - 1.0
```

---

## Style

### Style Struct

Complete style definition.

```rust
pub struct Style {
    // Size
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,

    // Flexbox
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub align_items: AlignItems,
    pub align_self: AlignSelf,
    pub justify_content: JustifyContent,
    pub gap: f32,

    // Spacing
    pub padding: Edges,
    pub margin: Edges,

    // Background
    pub background: Option<Background>,

    // Border
    pub border: BorderStyle,

    // Effects
    pub opacity: f32,
    pub shadow: Option<Shadow>,
}
```

### BorderStyle

```rust
pub struct BorderStyle {
    pub width: Edges,
    pub color: Color,
    pub radius: Corners,
}
```

### Corners

Border radius for each corner.

```rust
let corners = Corners::all(8.0);
let corners = Corners::new(tl: 8.0, tr: 8.0, br: 0.0, bl: 0.0);
```
