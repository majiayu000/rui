# RUI Architecture

This document describes the architecture of RUI, a GPU-accelerated UI framework for Rust.

## Overview

RUI is designed with a layered architecture that separates concerns and enables high-performance GPU rendering.

```mermaid
graph TB
    subgraph Application["Application Layer"]
        App["App::new().run()"]
        View["View"]
    end

    subgraph Elements["Element Layer"]
        Div["Div"]
        Text["Text"]
        Button["Button"]
        Input["Input"]
        Image["Image"]
        More["..."]
    end

    subgraph Core["Core Layer"]
        Geometry["Geometry"]
        Color["Color"]
        Style["Style"]
        Animation["Animation"]
        Entity["Entity"]
    end

    subgraph Renderer["Renderer Layer"]
        Scene["Scene"]
        Primitive["Primitives"]
        Metal["Metal Renderer"]
    end

    subgraph Platform["Platform Layer"]
        Window["Window"]
        Event["Event Loop"]
        GPU["GPU/Metal"]
    end

    App --> View
    View --> Elements
    Elements --> Core
    Elements --> Renderer
    Renderer --> Platform
```

## Layer Responsibilities

### 1. Application Layer

The entry point for RUI applications.

```rust
App::new().run(|cx| {
    // Build your UI here
    div()
        .child(text("Hello"))
});
```

**Components:**
- `App` - Application lifecycle management
- `AppContext` - Global application state and services
- `View` - View abstraction for rendering
- `ViewContext` - View-local state and rendering context

### 2. Element Layer

UI building blocks with a declarative builder pattern.

```mermaid
classDiagram
    class Element {
        <<trait>>
        +id() ElementId
        +style() Style
        +layout(cx) NodeId
        +paint(cx)
    }

    class Div {
        +children: Vec~AnyElement~
        +style: Style
        +child(element)
        +flex_row()
        +flex_col()
    }

    class Text {
        +content: String
        +font_size: f32
        +size(f32)
        +bold()
        +color(Color)
    }

    class Button {
        +label: String
        +on_click: Callback
        +variant(ButtonVariant)
    }

    class Image {
        +source: ImageSource
        +fit: ImageFit
        +cover()
        +contain()
    }

    Element <|-- Div
    Element <|-- Text
    Element <|-- Button
    Element <|-- Image
```

### 3. Core Layer

Fundamental types and abstractions.

```mermaid
graph LR
    subgraph Geometry
        Point["Point {x, y}"]
        Size["Size {width, height}"]
        Bounds["Bounds {origin, size}"]
        Edges["Edges {top, right, bottom, left}"]
    end

    subgraph ColorSystem["Color"]
        Color["Color"]
        Rgba["Rgba {r, g, b, a}"]
        Hsla["Hsla {h, s, l, a}"]
    end

    subgraph StyleSystem["Style"]
        Style["Style"]
        Border["BorderStyle"]
        Corners["Corners"]
        Background["Background"]
    end

    subgraph AnimationSystem["Animation"]
        Animation["Animation"]
        Easing["Easing Functions"]
        Transition["Transition"]
    end
```

### 4. Renderer Layer

GPU-accelerated rendering pipeline.

```mermaid
sequenceDiagram
    participant App
    participant Element
    participant Scene
    participant Renderer
    participant GPU

    App->>Element: build()
    Element->>Scene: add primitives
    Scene->>Renderer: render(scene)
    Renderer->>GPU: draw commands
    GPU-->>Renderer: rendered frame
    Renderer-->>App: display
```

**Primitives:**
- `Quad` - Rectangles with background, border, corners
- `Text` - Text rendering with font styling
- `Image` - Texture-based image rendering
- `Shadow` - Drop shadows with blur

### 5. Platform Layer

OS-specific window and event handling.

```mermaid
graph TB
    subgraph macOS["macOS Platform"]
        Window["NSWindow"]
        View["NSView + CAMetalLayer"]
        EventLoop["NSRunLoop"]
        Metal["Metal API"]
    end

    subgraph Future["Future Platforms"]
        Windows["Windows (DX12/Vulkan)"]
        Linux["Linux (Vulkan)"]
        Web["Web (WebGPU)"]
    end
```

## Data Flow

### Rendering Pipeline

```mermaid
flowchart LR
    A[App::run] --> B[Build Element Tree]
    B --> C[Layout with Taffy]
    C --> D[Generate Primitives]
    D --> E[Build Scene]
    E --> F[Metal Render Pass]
    F --> G[Present to Screen]
```

### Event Flow

```mermaid
flowchart TB
    A[OS Event] --> B[Platform Layer]
    B --> C[Event Dispatcher]
    C --> D{Event Type}
    D -->|Mouse| E[Mouse Handler]
    D -->|Keyboard| F[Keyboard Handler]
    D -->|Window| G[Window Handler]
    E --> H[Update State]
    F --> H
    G --> H
    H --> I[Re-render]
```

## Layout System

RUI uses [Taffy](https://github.com/DioxusLabs/taffy) for Flexbox layout.

```mermaid
graph TB
    subgraph Layout["Layout Calculation"]
        A[Element Tree] --> B[Taffy Nodes]
        B --> C[Compute Layout]
        C --> D[Position + Size]
    end

    subgraph Flexbox["Flexbox Properties"]
        direction["flex_direction"]
        justify["justify_content"]
        align["align_items"]
        gap["gap"]
        wrap["flex_wrap"]
    end

    Flexbox --> Layout
```

**Layout Properties:**
- `flex_direction` - Row or Column
- `justify_content` - Main axis alignment
- `align_items` - Cross axis alignment
- `gap` - Space between children
- `padding` - Inner spacing
- `margin` - Outer spacing

## Hooks System

React-like hooks for managing state and side effects.

```mermaid
classDiagram
    class UseMouse {
        +on_move(callback)
        +on_click(callback)
        +on_scroll(callback)
        +position() Point
    }

    class UsePaste {
        +on_paste(callback)
        +enable_bracketed_paste()
    }

    class UseWindowFocus {
        +on_focus(callback)
        +on_blur(callback)
        +is_focused() bool
    }
```

## Memory Management

RUI uses Rust's ownership system for memory safety with minimal allocations.

```mermaid
graph LR
    subgraph Ownership
        A[App owns Window]
        B[Window owns View]
        C[View owns Elements]
        D[Elements own Children]
    end

    A --> B --> C --> D
```

**Strategies:**
- `SmallVec` for small collections
- `SlotMap` for entity storage
- Stack allocation for primitives
- GPU buffer pooling

## Module Structure

```
rui/
├── src/
│   ├── lib.rs              # Library entry
│   ├── prelude.rs          # Common exports
│   │
│   ├── core/               # Core types
│   │   ├── app.rs          # Application
│   │   ├── color.rs        # Color types
│   │   ├── geometry.rs     # Geometry types
│   │   ├── style.rs        # Style system
│   │   ├── animation.rs    # Animations
│   │   ├── entity.rs       # Entity system
│   │   ├── view.rs         # View abstraction
│   │   └── window.rs       # Window management
│   │
│   ├── elements/           # UI Elements
│   │   ├── element.rs      # Element trait
│   │   ├── div.rs          # Container
│   │   ├── text.rs         # Text
│   │   ├── button.rs       # Button
│   │   ├── input.rs        # Text input
│   │   ├── image.rs        # Image
│   │   ├── table.rs        # Table
│   │   ├── list.rs         # Lists
│   │   ├── progress.rs     # Progress bar
│   │   └── spinner.rs      # Spinner
│   │
│   ├── hooks/              # React-like hooks
│   │   ├── use_mouse.rs    # Mouse events
│   │   ├── use_paste.rs    # Paste events
│   │   └── use_window_focus.rs
│   │
│   ├── renderer/           # Rendering
│   │   ├── scene.rs        # Scene graph
│   │   └── primitive.rs    # Render primitives
│   │
│   └── platform/           # Platform-specific
│       └── macos/          # macOS (Metal)
│
└── examples/               # Example apps
    ├── hello_world.rs
    ├── counter.rs
    ├── dashboard.rs
    └── animation_demo.rs
```

## Performance Considerations

### GPU Rendering
- Direct Metal rendering bypasses CPU-bound drawing
- Batched draw calls reduce GPU state changes
- Vertex buffer reuse minimizes allocations

### Layout Caching
- Taffy caches layout calculations
- Only dirty subtrees are recalculated
- Incremental layout updates

### Memory Efficiency
- Zero-copy where possible
- Pre-allocated buffers
- Minimal heap allocations in hot paths

## Future Directions

1. **Cross-Platform Support**
   - Vulkan renderer for Windows/Linux
   - WebGPU for browser support

2. **State Management**
   - `use_state` hook
   - `use_effect` for side effects
   - Context system for shared state

3. **Advanced Features**
   - Text editing/selection
   - Accessibility support
   - Internationalization
