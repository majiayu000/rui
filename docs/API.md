# RUI API Reference

Complete API documentation for the RUI framework.

## Table of Contents

- [Core Types](#core-types)
- [Elements](#elements)
- [Advanced UI Themes](#advanced-ui-themes)
- [One Widget Per Role](#one-widget-per-role)
- [Styling](#styling)
- [Colors](#colors)
- [Animation](#animation)
- [Validation](#validation)

---

## Drift-Checked Public Surface

`cargo test --test docs_api_drift` verifies that this section stays aligned with the public source API.

Builders: `div()`, `text(content)`, `button(label)`, `input()`, `image(path)`, `scroll_view()`, `table()`, `row()`, `header_row()`, `cell(content)`, `list()`, `ordered_list()`, `unordered_list()`, `spinner()`.

Advanced UI builders: `advanced_ui::container()`, `advanced_ui::row()`, `advanced_ui::column()`, `advanced_ui::text(content)`, `advanced_ui::hoverable(child)`, `advanced_ui::checkbox(label)`, `advanced_ui::progress_bar(value)`, `advanced_ui::segmented_control(options, selected)`, `advanced_ui::toolbar(label)`, `advanced_ui::tooltip(child, content)`, `advanced_ui::menu(label, items)`, `advanced_ui::popover(label, anchor, content)`, `advanced_ui::dialog(label, content)`, `advanced_ui::tab(value, label)`, `advanced_ui::tab_list(tabs, selected)`, `advanced_ui::tab_panel(value, child)`, `advanced_ui::tabs(tabs, selected)`, `advanced_ui::data_list(items)`, `advanced_ui::data_tree(roots)`, `advanced_ui::data_table_row(cells)`.

Enum variants: `InputType::Text`, `InputType::Password`, `InputType::Email`, `InputType::Number`, `InputType::Search`; `ImageFit::Cover`, `ImageFit::Contain`, `ImageFit::Fill`, `ImageFit::None`, `ImageFit::ScaleDown`; `ScrollDirection::Vertical`, `ScrollDirection::Horizontal`, `ScrollDirection::Both`; `ScrollbarVisibility::Auto`, `ScrollbarVisibility::Always`, `ScrollbarVisibility::Never`, `ScrollbarVisibility::Hover`; `ListStyle::Bullet`, `ListStyle::Numbered`, `ListStyle::LowercaseAlpha`, `ListStyle::UppercaseAlpha`, `ListStyle::LowercaseRoman`, `ListStyle::UppercaseRoman`, `ListStyle::None`; `SpinnerType::Dots`, `SpinnerType::Line`, `SpinnerType::Circle`, `SpinnerType::Arrow`, `SpinnerType::Box`, `SpinnerType::Bounce`, `SpinnerType::Grow`, `SpinnerType::Star`; `TextAlign::Left`, `TextAlign::Center`, `TextAlign::Right`; `ControlSize::Small`, `ControlSize::Medium`, `ControlSize::Large`; `ControlVariant::Primary`, `ControlVariant::Secondary`, `ControlVariant::Outline`, `ControlVariant::Ghost`, `ControlVariant::Danger`, `ControlVariant::Success`; `MainAxisAlignment::Start`, `MainAxisAlignment::End`, `MainAxisAlignment::Center`, `MainAxisAlignment::SpaceBetween`, `MainAxisAlignment::SpaceAround`; `CrossAxisAlignment::Start`, `CrossAxisAlignment::End`, `CrossAxisAlignment::Center`, `CrossAxisAlignment::Stretch`; `Easing::Linear`, `Easing::EaseIn`, `Easing::EaseOut`, `Easing::EaseInOut`, `Easing::EaseInQuad`, `Easing::EaseOutQuad`, `Easing::EaseInOutQuad`, `Easing::EaseInCubic`, `Easing::EaseOutCubic`, `Easing::EaseInOutCubic`, `Easing::EaseInQuart`, `Easing::EaseOutQuart`, `Easing::EaseInOutQuart`, `Easing::EaseInExpo`, `Easing::EaseOutExpo`, `Easing::EaseInOutExpo`, `Easing::EaseInBack`, `Easing::EaseOutBack`, `Easing::EaseInOutBack`, `Easing::EaseInElastic`, `Easing::EaseOutElastic`, `Easing::EaseInOutElastic`, `Easing::EaseInBounce`, `Easing::EaseOutBounce`, `Easing::EaseInOutBounce`, `Easing::Spring`, `Easing::Custom`.

Validation: CI-safe checks are `cargo test --test docs_api_drift`, `cargo test example_smoke`, `cargo test dogfood`, `cargo check`, and `cargo test`. The native macOS GUI smoke is local-only: `scripts/native_dogfood_macos.sh`.

---

## One Widget Per Role

Each role has one public widget. `button`, `input` and `scroll_view` are in
the prelude; the other controls live in `rui::advanced_ui`.

| Role | Widget | Replaces |
|------|--------|----------|
| Button | `button(label)` (`advanced_ui::Button`) | the old `elements::Button`, `ButtonVariant`, `ButtonSize` |
| Single-line text | `input()` (`elements::Input`) | `advanced_ui::TextField` / `text_field(label)` |
| Scroll area | `scroll_view()` (`elements::ScrollView`) | `advanced_ui::Scrollable` / `scrollable(child)` |
| Progress | `advanced_ui::progress_bar(value)` | the terminal-style `elements::Progress` / `progress()` |
| Text | `text(content)` (`elements::Text`); `.accessible()` adds an accessibility node labelled with the content, which `advanced_ui::text(content)` sets | the `advanced_ui::Text` wrapper |

Migration: `text_field("Name")` is `input().accessibility_label("Name").theme(Theme::default())`;
`scrollable(child)` is `scroll_view().child(child)` and `.background(color)`
is `.bg(color)`; `ButtonVariant` / `ButtonSize` are `ControlVariant` /
`ControlSize`. The terminal `rui::hooks` module (terminal mouse reports,
bracketed paste, focus reporting) is gone: window, pointer, paste and focus
events reach elements through the element event handlers.

---

## Advanced UI Themes

Advanced controls accept an app-owned `rui::advanced_ui::Theme` through
`.theme(theme)`. Store the theme in app or view state and rebuild controls when
it changes; the controls resolve their color, radius, spacing, typography,
density, and shared state tokens during layout and paint.

```rust
use rui::advanced_ui::{Theme, ThemeDensity, button};

let mut theme = Theme::dark().with_density(ThemeDensity { scale: 1.1 });
theme.radius.control = 8.0;

button("Save").theme(theme);
```

`Theme::light()`, `Theme::dark()`, and `Theme::high_contrast()` provide the
standard presets. Apps can customize the public token structs directly.

Raw elements such as `div()` and `text()` remain explicit primitive builders:
they use the colors, spacing, and style values passed to their own builder
methods. They do not read advanced UI theme tokens automatically. `input()`
uses its own default look until `.theme(theme)` or `.size(ControlSize)` opts it
into the theme contract. Use advanced controls when shared
disabled/read-only/invalid/focus/hover/pressed, loading, and error state
styling should come from the theme contract.

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

#### Clipboard

`AppContext` and `ViewContext` read and write plain text on the app clipboard:
the general pasteboard on macOS, an in-memory clipboard in headless sessions.

```rust
cx.write_to_clipboard(ClipboardItem::new_string("rui://link"))?;
let pasted = cx.read_from_clipboard()?.map(|item| item.text().to_owned());
```

`read_from_clipboard` returns `Ok(None)` when the clipboard holds no text.
`AppContext::set_clipboard` replaces the clipboard, for example with a
`MemoryClipboard` in tests. `Input` and `TextArea` copy, cut and paste through
it on the `Copy` / `Cut` / `Paste` standard actions, bound to Cmd+C / Cmd+X /
Cmd+V in the standard keymap.

#### Menus

`cx.set_menus` sets the application menu bar from a tree of `Menu`s whose
items run actions. Choosing an item routes its action like a key binding: to
the focused element, then the app's action handlers. Each item shows the
keymap shortcut bound to its action.

```rust
use rui::core::menu::{Menu, MenuAction};

cx.set_menus(vec![
    Menu::new("App").item(MenuAction::new("Quit", ActionId::custom("app.quit"))),
    Menu::new("Edit")
        .item(MenuAction::new("Copy", StandardAction::Copy))
        .item(MenuAction::new("Paste", StandardAction::Paste).enabled(can_paste))
        .separator()
        .submenu(Menu::new("View").item(MenuAction::new("Wrap", wrap).checked(self.wrap))),
]);

div().on_context_menu(cx.listener_with(|_, position, cx| {
    cx.show_context_menu(Menu::new("Card").item(MenuAction::new("Delete", delete)), position)
}))
```

A render may call `set_menus` every time; the bar is rebuilt only when it
changed. On macOS the first menu is the application menu. A right press runs
`Div::on_context_menu` and neither clicks nor starts a drag. Headless sessions
choose items with `choose_menu_item(&menu, label)` and read requested context
menus with `take_context_menu()`.

#### File Dialogs

`cx.prompt_for_paths(options)` and `cx.prompt_for_new_path(directory)` ask the
user for paths and return a `Task` that finishes with them, or `None` when the
user cancels. On macOS they show `NSOpenPanel` / `NSSavePanel`.

```rust
use rui::core::prompt::PathPromptOptions;

let paths = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: true });
cx.app_mut()
    .spawn(move |cx| async move {
        if let Ok(Some(paths)) = paths.await {
            cx.update(|cx| open_files(cx, paths));
        }
    })
    .detach();
```

A prompt dropped without an answer fails its task with `PromptDismissed`.
Headless sessions take the waiting prompts with `take_path_prompts()` and
answer each `PathPrompt` with `answer.send(..)`.

#### Focus

Tab and Shift-Tab move keyboard focus through enabled buttons, checkboxes,
text fields, segmented controls, tab lists and menus in tree order, wrapping
around, when the focused element does not take the key itself.

App code moves focus with `cx.focus(id)` and `cx.blur()` and reads it with
`cx.focused()`, on `AppContext` and `ViewContext`. A `FocusHandle` keeps an
id for one element:

```rust
use rui::core::focus::FocusHandle;

struct Search { field: FocusHandle }

input().id(self.field.id()).accessibility_label("Search");
div().on_click(cx.listener(|this, cx| this.field.focus(cx.app_mut())));
```

The change applies in the window's next frame: the old element's `on_blur`
runs, then the new one's `on_focus`. `focused()` already reports a change
that has been asked for. Focusing an id that is not in the window logs an
error and changes nothing.

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

#### Drag and Drop

```rust
div()
    .on_drag(|| Card { id }, |card| div().w(80.0).h(24.0).child(text(card.title())))
    .on_click(open_card);

div()
    .on_drop::<Card>({
        let add = cx.listener_with(|this, card: Card, _| this.cards.push(card));
        move |card| add(card.clone())
    })
    .on_drag_over::<Card>(cx.listener_with(|this, over, _| this.highlight = over))
    .on_drop_files(|paths| println!("dropped {paths:?}"));
```

| Method | Description |
|--------|-------------|
| `.on_drag(payload, preview)` | Drag source. A press makes the payload and its preview; past a 3 pt move the preview follows the pointer. A press and release without moving still clicks |
| `.on_drop::<T>(handler)` | Receive a `T` released over the div |
| `.on_drag_over::<T>(handler)` | `true` when a drag of `T` moves over the div, `false` when it leaves, drops or is cancelled |
| `.on_drop_files(handler)` | Receive files dropped from Finder or another app (a drag of `ExternalPaths`) |

Headless sessions drive in-app drags with `pointer_down` / `pointer_move` /
`pointer_up` and file drags with `dispatch_file_drag(FileDragEvent)`.

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

Themed, focusable button (`advanced_ui::Button`, re-exported by the prelude).
It exposes an accessible `Button` node and activates on click or the
`Activate` action.

```rust
button("Click Me")
    .variant(ControlVariant::Primary)
    .size(ControlSize::Medium)
    .on_click(|| println!("Clicked!"))
```

#### Methods

| Method | Description |
|--------|-------------|
| `button(label)` | Create button (label must not be empty) |
| `.variant(ControlVariant)` | Style variant; also `.primary()`, `.secondary()`, `.outline()`, `.ghost()`, `.danger()`, `.success()` |
| `.size(ControlSize)` | Size preset; also `.small()`, `.large()` |
| `.theme(Theme)` | Theme tokens |
| `.rounded(radius)` / `.rounded_full()` | Override the theme radius |
| `.on_click(handler)` | Click handler |
| `.disabled(bool)` / `.read_only(bool)` / `.invalid(bool)` | Shared control state |

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
| `.on_cancel(handler)` | Escape handler |
| `.on_focus(handler)` | Focus handler |
| `.on_blur(handler)` | Blur handler |
| `.accessibility_label(text)` | Accessible name (falls back to the placeholder) |
| `.disabled(bool)` / `.read_only(bool)` / `.invalid(bool)` | Shared control state |
| `.theme(Theme)` | Paint with theme tokens, control height and radius |
| `.size(ControlSize)` | Themed control size (default theme unless `.theme` sets one) |
| `.w(width)` / `.h(height)` | Fixed size |

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

### ScrollView

Clipped, scrollable container. It exposes a `ScrollArea` accessibility node
with its scroll position and scroll actions.

```rust
scroll_view()
    .h(240.0)
    .accessibility_label("Activity")
    .child(div().children(rows))
```

| Method | Description |
|--------|-------------|
| `scroll_view()` | Create scroll view |
| `.child(element)` / `.children(iter)` | Content |
| `.direction(ScrollDirection)` | Also `.vertical()`, `.horizontal()`, `.both()` |
| `.scrollbar_always()` / `.scrollbar_never()` | Scrollbar visibility (default: when content overflows) |
| `.w(width)` / `.h(height)` / `.size(size)` / `.bg(color)` | Viewport size and background |
| `.accessibility_label(text)` | Accessible name |
| `.disabled(bool)` / `.read_only(bool)` | Freeze scrolling; children still get pointer and text input |
| `.on_scroll(handler)` | Called with the new `(x, y)` offset |

---

### TextArea

Multiline text editing field.

```rust
text_area()
    .placeholder("Write a message...")
    .on_change(|value| println!("Value: {}", value))
```

#### Methods

| Method | Description |
|--------|-------------|
| `text_area()` | Create multiline text area |
| `.value(String)` | Set value |
| `.placeholder(text)` | Placeholder text |
| `.on_change(handler)` | Change handler |
| `.disabled(bool)` | Disable editing and focus |
| `.read_only(bool)` | Preserve value while allowing navigation |
| `.invalid(bool)` | Mark validation state |

---

### Stateful Control Contracts

Stateful controls separate persistent values from transient interaction state:

- Controlled values are supplied on every render, for example `Checkbox::checked`, `SegmentedControl::selected`, and `Input::value`. Wrap event handlers in `cx.listener` (no arguments), `cx.listener_with` (a value such as `bool`) or `cx.text_listener` (`&str`): the handler receives the view and a `ViewContext` that reaches every `AppContext` entity, runs before the next render, and the view renders again afterwards without a manual notify.
- Uncontrolled values are local to the current element instance. They can handle immediate interaction, but they are not a persistence boundary across rebuilds.
- Disabled controls refuse activation, do not call mutation callbacks, expose disabled accessibility state, and clear hover/pressed transients.
- Read-only controls still expose their value, but refuse internal value mutation and mutation callbacks.
- Hover, pressed, focus, validation, and accessibility announcement state are component-local transients. Persist application data outside the element tree when it must survive rebuilds.

```rust
struct SyncView {
    checked: bool,
}

impl View for SyncView {
    type Element = Checkbox;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        Checkbox::new("Sync")
            .checked(self.checked)
            .on_change(cx.listener_with(|this, value, _| this.checked = value))
    }
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
    .list_style(ListStyle::Numbered)
    .child(ListItem::new("Step 1"))
    .child(ListItem::new("Step 2"))
```

#### ListStyle

```rust
pub enum ListStyle {
    Bullet,
    Numbered,
    LowercaseAlpha,
    UppercaseAlpha,
    LowercaseRoman,
    UppercaseRoman,
    None,
}
```

---

### Spinner

Loading spinner element.

```rust
spinner()
    .spinner_type(SpinnerType::Dots)
    .font_size(24.0)
    .color(Color::BLUE)
```

#### SpinnerType

```rust
pub enum SpinnerType {
    Dots,
    Line,
    Circle,
    Arrow,
    Box,
    Bounce,
    Grow,
    Star,
}
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
    EaseInExpo,
    EaseOutExpo,
    EaseInOutExpo,
    EaseInBack,
    EaseOutBack,
    EaseInOutBack,
    EaseInElastic,
    EaseOutElastic,
    EaseInOutElastic,
    EaseInBounce,
    EaseOutBounce,
    EaseInOutBounce,
    Spring { stiffness: f32, damping: f32 },
    Custom(fn(f32) -> f32),
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
