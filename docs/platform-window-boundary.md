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

## Current Scope

macOS is the only active runtime backend. Its AppKit window wrapper owns the
native window and Metal layer, and the app loop translates native events inside
`src/platform/mac`. Clipboard support is still not wired to a native pasteboard,
so the shared contract reports that capability as unsupported.

Non-macOS platforms intentionally use explicit unsupported errors until a real
backend is implemented and covered by the shared platform contract tests.
