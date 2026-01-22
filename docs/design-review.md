# RUI Design Review

## What this repository does
RUI is a Rust UI framework that renders a declarative element tree directly to the GPU (Metal on macOS). It provides a chainable builder API for common UI elements (Div, Text, Button, etc.), uses Taffy for flexbox layout, and collects rendering primitives into a scene for the GPU backend.

## Current design issues (short list)
- Layout results were not used in painting, causing children to be painted with parent bounds and overlap.
- Taffy nodes were appended every frame without clearing, leading to unbounded layout tree growth.
- Events, view updates, and hooks are defined but not wired into the run loop.
- Window options are only partially applied on macOS.
- Text measurement is a rough estimate, so layout and text render can diverge.

## Module deep dive: layout -> paint pipeline
### Files
- `src/platform/mac/app.rs`
- `src/elements/element.rs`
- `src/elements/div.rs`
- `src/elements/scroll_view.rs`
- `src/elements/list.rs`

### How it worked before
1. The render loop built a Taffy tree each frame, but never cleared it.
2. Elements created Taffy nodes in `layout()`.
3. Layout was computed, and root bounds were used to paint.
4. Container elements painted all children using the same parent bounds, ignoring Taffy results.

### Fix implemented
- Clear the Taffy tree at the start of each frame, preventing unbounded growth.
- Add the Taffy tree to `PaintContext` so elements can query computed layout.
- Store child node IDs in container elements during layout.
- Use child layout results to compute per-child bounds in paint.

### Why this matters
Without using computed layout, any flex layout or sizing logic has no visual effect. Fixing this makes the layout system actually drive rendering, which is the core responsibility of a UI framework.

## Follow-up ideas
- Wire event handling into elements and the platform loop.
- Replace text size estimation with font metrics.
- Integrate view diffing/state updates into the render loop.
- Apply full window options on macOS and add a platform abstraction for other backends.

## Event wiring (AppKit event queue)
### What was added
- Pointer, scroll, and key events are pulled from the AppKit event queue and dispatched into the element tree.
- Container elements forward events to children using layout bounds, so nested elements can respond.
- Inputs and buttons update hover/pressed/focus and invoke callbacks on click and submit.
- Window focus/resize/close are detected and surfaced as window events.

### Limitations
- Events are still polled per frame via `nextEventMatchingMask`, not installed as delegates.
- Focus is best-effort and relies on element IDs; unfocused elements may render stale focus until the next pointer event.
