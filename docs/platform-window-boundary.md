# Platform Window Boundary

The platform window layer is the contract between core UI state, renderer
attachments, and native window APIs. Renderer modules must not depend on native
window APIs directly; they receive renderer targets through platform-owned
attachments.

## Required Features

A backend is not considered supported until it implements and tests these
features:

- lifecycle: create, show, close, and close-request handling
- input events: pointer, scroll, key down, and key up delivery
- dpi: scale-factor reporting and scale-factor change events
- resizing: content-size reporting and resize events
- focus: window focus state and focus-change events
- clipboard: explicit text read/write support
- renderer attachment: a tested native target for the active renderer

Unsupported features must return `PlatformWindowError::Unsupported`; a backend
must not report success by dropping the operation or returning placeholder data.
Multi-window support is not part of the required backend set yet. Until RUI can
create, route events for, and render multiple native windows end to end,
attempting to open a second window returns `PlatformWindowError::Unsupported`
with the `multi-window` feature.

## Current Scope

macOS is the only active runtime backend. Its AppKit window wrapper owns the
native window and Metal layer, and the app loop translates native events inside
`src/platform/mac` into backend-neutral `PlatformWindowEvent` values. Clipboard
support is wired to the AppKit general pasteboard and is reported as a native
backend capability.

macOS lifecycle signals are delegate-backed where AppKit provides callbacks:
window close, minimize/deminiaturize, key focus, resize, application activation,
dock reopen, and Cmd+Q. The event loop still polls native events, but delegate
callbacks are drained into the platform event stream explicitly instead of being
inferred from visibility alone.

Redraw scheduling tracks source counters in `AppContext`. Native platform events
mark redraw work by source, while explicit element/runtime redraw requests post a
coalesced platform redraw event so repeated requests wake the loop once without
dropping the final paint.

Non-macOS platforms intentionally use explicit unsupported errors until a real
backend is implemented and covered by the shared platform contract tests.
