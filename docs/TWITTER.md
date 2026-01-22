# RUI Twitter Promotion Guide

A complete guide for promoting RUI on Twitter/X with ready-to-use content.

## Launch Thread

### Tweet 1 - Announcement

```
Announcing RUI - A GPU-accelerated UI framework for Rust

After months of development, I'm excited to share RUI:
- Metal GPU rendering for 120fps UIs
- Declarative builder API
- Flexbox layouts with Taffy
- React-like hooks

Built with Rust for performance and safety.

github.com/majiayu000/rui
```

### Tweet 2 - Code Example

```
RUI makes building UIs in Rust intuitive:

div()
    .bg(Color::hex(0x1a1a2e))
    .rounded(16.0)
    .shadow_lg()
    .child(
        text("Hello, RUI!")
            .size(48.0)
            .bold()
            .color(Color::WHITE)
    )

Chainable methods, type-safe, zero cost abstractions.
```

### Tweet 3 - Architecture

```
RUI Architecture:

Application Layer
    ↓
Element Layer (Div, Text, Button, Image...)
    ↓
Core Layer (Geometry, Color, Style, Animation)
    ↓
Metal GPU Renderer

Direct GPU rendering = buttery smooth performance
```

### Tweet 4 - Test Coverage

```
Quality matters. RUI has:

- 66% test coverage
- 1208 unit tests
- 4 modules at 100% coverage
- Comprehensive API testing

Good tests = reliable framework
```

### Tweet 5 - Call to Action

```
Try RUI today:

cargo add rui

Or check out the examples:
cargo run --example hello_world
cargo run --example counter
cargo run --example dashboard

Star on GitHub: github.com/majiayu000/rui

Contributions welcome!
```

---

## Individual Tweets

### Feature Highlights

```
RUI's color system is flexible:

Color::hex(0xFF5733)     // Hex
Color::rgb(1.0, 0.5, 0.0) // RGB
Color::hsl(180.0, 0.5, 0.5) // HSL
Color::rgba(1.0, 0.0, 0.0, 0.8) // With alpha

All colors are GPU-optimized for fast rendering.
```

```
Flexbox layouts in RUI are intuitive:

div()
    .flex_col()           // Vertical
    .items_center()       // Center horizontally
    .justify_center()     // Center vertically
    .gap(16.0)            // Space between
    .p(20.0)              // Padding

CSS developers will feel right at home.
```

```
React developers: RUI has hooks!

use_mouse()        // Track mouse events
use_paste()        // Clipboard handling
use_window_focus() // Focus detection

More hooks coming soon: use_state, use_effect
```

### Technical Deep Dives

```
Why Metal for RUI?

1. Native macOS performance
2. 120Hz display support
3. Low-level GPU control
4. Efficient batched rendering
5. Future: Vulkan/WebGPU for cross-platform

Performance is a feature, not an afterthought.
```

```
RUI uses Taffy for layout - the same engine powering:

- Dioxus
- Bevy UI
- Iced

Battle-tested Flexbox implementation with:
- Correct CSS3 behavior
- Fast incremental updates
- Zero allocations in hot path
```

### Code Snippets

```
Building a button in RUI:

button("Click Me")
    .w(120.0)
    .h(48.0)
    .bg(Color::hex(0x0984e3))
    .rounded(8.0)
    .on_click(|_| println!("Clicked!"))

Simple. Clean. Type-safe.
```

```
A feature card component in 15 lines:

fn feature_card(title: &str, desc: &str) -> Div {
    div()
        .w(200.0)
        .bg(Color::hex(0x16213e))
        .rounded(12.0)
        .p(20.0)
        .child(text(title).size(20.0).bold())
        .child(text(desc).size(14.0))
}

Composable, reusable, maintainable.
```

---

## Image Assets Checklist

### Required Screenshots

1. **Hello World Demo**
   - Clean dark theme
   - Title with gradient text
   - Feature cards below
   - Size: 1200x675 (Twitter card ratio)

2. **Counter Demo**
   - Interactive UI elements
   - Circular buttons
   - Counter display
   - Size: 1200x675

3. **Dashboard Demo**
   - Complex layout
   - Sidebar + content
   - Multiple stat cards
   - Size: 1200x675

4. **Code + Result Split**
   - Left: Code editor with RUI code
   - Right: Rendered output
   - Size: 1200x675

### GIF/Video Ideas

1. **Animation Demo** (5-10 seconds)
   - Show easing functions
   - Smooth transitions
   - Size: 800x600

2. **Building UI in Real-time** (15-30 seconds)
   - Start with empty
   - Add elements step by step
   - Show final result

3. **Performance Demo** (10 seconds)
   - FPS counter visible
   - Complex UI with animations
   - Show smooth 120fps

### Logo/Banner

1. **Profile Image**
   - RUI logo
   - Dark background
   - 400x400px

2. **Header Banner**
   - "RUI - GPU-accelerated UI for Rust"
   - Code snippet background
   - 1500x500px

---

## Hashtags

Primary:
```
#Rust #RustLang #GPU #UI #Framework
```

Technical:
```
#Metal #macOS #OpenSource #Developer #Coding
```

Community:
```
#RustUI #RustDev #100DaysOfCode #BuildInPublic
```

---

## Engagement Strategy

### Best Times to Post
- Weekdays: 9am-11am PST
- Weekends: 10am-12pm PST
- Avoid: Late night, major holidays

### Response Templates

**For Questions:**
```
Great question! [Answer]. Check out the docs for more: github.com/majiayu000/rui/docs
```

**For Feature Requests:**
```
Love this idea! Created an issue to track: [link]. PRs welcome!
```

**For Bug Reports:**
```
Thanks for reporting! Can you open an issue with details? github.com/majiayu000/rui/issues
```

### Cross-Promotion

1. **r/rust** - Post announcement with code examples
2. **Hacker News** - "Show HN: RUI - GPU-accelerated UI framework for Rust"
3. **Dev.to** - Write tutorial article
4. **Discord** - Rust community servers

---

## Metrics to Track

- Stars on GitHub
- Forks
- Issues/PRs
- Downloads (crates.io when published)
- Tweet impressions
- Profile visits
- Follower growth

---

## Content Calendar

### Week 1 - Launch
- Day 1: Launch thread (5 tweets)
- Day 2: Architecture deep dive
- Day 3: Code example video
- Day 4: Community engagement
- Day 5: Feature highlight

### Week 2 - Education
- Tutorial thread
- FAQ answers
- Comparison with alternatives
- Performance benchmarks

### Week 3 - Community
- Highlight contributions
- Share user projects
- Answer questions
- Feature roadmap

### Ongoing
- Weekly tips
- Version release announcements
- Community spotlights
- Technical deep dives
