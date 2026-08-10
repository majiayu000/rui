//! macOS window creation

use crate::core::event::{KeyCode, KeyEvent, Modifiers, MouseButton, ScrollEvent};
use crate::core::geometry::{Point, Size};
use crate::core::window::WindowOptions;
use crate::platform::mac::MacAccessibilityBridge;
use crate::platform::mac::lifecycle::{MacLifecycleDelegate, MacLifecycleEvent};
use crate::platform::mac::text_input::{RuiContentView, append_ime_events_after_native_dispatch};
use crate::platform::window::{
    PlatformInputEvent, PlatformMouseEvent, PlatformMouseEventKind, PlatformRendererAttachment,
    PlatformRendererTarget, PlatformWindow, PlatformWindowError, PlatformWindowEvent,
    PlatformWindowFeatures, PlatformWindowState, validate_window_options,
};
use metal::Device;
use metal::foreign_types::ForeignType;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplication, NSEvent, NSEventMask, NSEventModifierFlags, NSEventType, NSPasteboard,
    NSPasteboardTypeString, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::CAMetalLayer;
use std::collections::HashSet;

use crate::platform::mac::events::{
    MAC_REDRAW_EVENT_DATA, MacApplicationEvent, MacWindowEvent, append_platform_events,
    application_event, post_application_event,
};

pub struct MacWindow {
    window: Retained<NSWindow>,
    content_view: Retained<RuiContentView>,
    metal_layer: Retained<CAMetalLayer>,
    accessibility_bridge: MacAccessibilityBridge,
    last_content_size: Size,
    last_scale_factor: f32,
    last_focused: bool,
    last_visible: bool,
    last_miniaturized: bool,
    created_event_pending: bool,
    suppressed_key_ups: SuppressedKeyUps,
    lifecycle_delegate: Retained<MacLifecycleDelegate>,
}

#[derive(Debug, Default)]
struct SuppressedKeyUps {
    key_codes: HashSet<u16>,
}

impl SuppressedKeyUps {
    fn record_consumed_key_down(&mut self, key_code: u16) {
        self.key_codes.insert(key_code);
    }

    fn should_emit_key_up(&mut self, key_code: u16) -> bool {
        !self.key_codes.remove(&key_code)
    }
}

impl MacWindow {
    pub(crate) fn accessibility_bridge_mut(&mut self) -> &mut MacAccessibilityBridge {
        &mut self.accessibility_bridge
    }

    pub(crate) fn make_key_and_order_front(&self) {
        unsafe {
            let _: () = msg_send![
                &*self.window,
                makeKeyAndOrderFront: std::ptr::null::<objc2::runtime::AnyObject>()
            ];
        }
    }

    pub(crate) fn window_number(&self) -> isize {
        self.window.windowNumber()
    }

    pub(crate) fn content_size(&self) -> Result<Size, PlatformWindowError> {
        let content_view = self
            .window
            .contentView()
            .ok_or_else(|| PlatformWindowError::backend("macos", "window has no content view"))?;
        let view_bounds: NSRect = unsafe { msg_send![&*content_view, bounds] };
        Ok(Size::new(
            view_bounds.size.width as f32,
            view_bounds.size.height as f32,
        ))
    }

    pub(crate) fn is_focused(&self) -> bool {
        unsafe { msg_send![&*self.window, isKeyWindow] }
    }

    pub(crate) fn is_visible(&self) -> bool {
        unsafe { msg_send![&*self.window, isVisible] }
    }

    pub(crate) fn is_miniaturized(&self) -> bool {
        self.window.isMiniaturized()
    }

    pub(crate) fn set_minimized(&self, minimized: bool) {
        unsafe {
            if minimized {
                let _: () = msg_send![
                    &*self.window,
                    miniaturize: std::ptr::null::<objc2::runtime::AnyObject>()
                ];
            } else {
                let _: () = msg_send![
                    &*self.window,
                    deminiaturize: std::ptr::null::<objc2::runtime::AnyObject>()
                ];
            }
        }
    }

    pub(crate) fn install_application_delegate(&self, app: &NSApplication) {
        self.lifecycle_delegate.install_as_app_delegate(app);
    }

    pub(crate) fn next_drawable(&self) -> Option<&metal::MetalDrawableRef> {
        let layer_ptr =
            objc2::rc::Retained::as_ptr(&self.metal_layer) as *mut objc2::runtime::AnyObject;
        let drawable: *mut objc2::runtime::AnyObject =
            unsafe { msg_send![layer_ptr, nextDrawable] };

        if drawable.is_null() {
            return None;
        }

        Some(unsafe { &*drawable.cast::<metal::MetalDrawableRef>() })
    }

    fn scale_factor(&self) -> f32 {
        let scale_factor: f64 = unsafe { msg_send![&*self.window, backingScaleFactor] };
        scale_factor as f32
    }

    fn update_metal_layer_size(&self, content_size: Size, scale_factor: f32) {
        let drawable_size = NSSize::new(
            content_size.width as f64 * scale_factor as f64,
            content_size.height as f64 * scale_factor as f64,
        );
        unsafe {
            let _: () = msg_send![&*self.metal_layer, setDrawableSize: drawable_size];
            let _: () = msg_send![&*self.metal_layer, setContentsScale: scale_factor as f64];
        }
    }

    pub(crate) fn poll_events_for_app(
        &mut self,
        app: &NSApplication,
        wait_for_event: bool,
    ) -> Result<Vec<MacWindowEvent>, PlatformWindowError> {
        let mut events = Vec::new();
        let mut lifecycle_events = Vec::new();
        self.push_lifecycle_events(&mut lifecycle_events)?;
        append_platform_events(&mut events, lifecycle_events);

        let mask = platform_event_mask();
        let mut expiration = if wait_for_event {
            NSDate::distantFuture()
        } else {
            NSDate::distantPast()
        };
        let window_number = self.window_number();

        while let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(
            mask,
            Some(&expiration),
            unsafe { NSDefaultRunLoopMode },
            true,
        ) {
            expiration = NSDate::distantPast();

            if event.windowNumber() != window_number {
                app.sendEvent(&event);
                let mut lifecycle_events = Vec::new();
                self.push_delegate_lifecycle_events(&mut lifecycle_events)?;
                append_platform_events(&mut events, lifecycle_events);
                continue;
            }

            let event_type = event.r#type();
            if event_type == NSEventType::ApplicationDefined {
                match application_event(event.data1()) {
                    Some(MacApplicationEvent::Redraw) => {
                        events.push(MacWindowEvent::Platform(
                            PlatformWindowEvent::RedrawRequested,
                        ));
                        continue;
                    }
                    Some(MacApplicationEvent::Accessibility) => {
                        if let Some(request) = self.accessibility_bridge.take_action_request() {
                            events.push(MacWindowEvent::Accessibility(request));
                        }
                        continue;
                    }
                    None => {}
                }
            }

            let mut platform_events = Vec::new();
            let suppress_key_up = event_type == NSEventType::KeyUp
                && !self.suppressed_key_ups.should_emit_key_up(event.keyCode());
            if !suppress_key_up {
                append_platform_events_from_native_event(
                    &event,
                    self.last_content_size,
                    &mut platform_events,
                );
            }
            app.sendEvent(&event);
            let consumed_key_down = append_ime_events_after_native_dispatch(
                &mut platform_events,
                self.content_view.drain_ime_events(),
            );
            if consumed_key_down && event_type == NSEventType::KeyDown {
                self.suppressed_key_ups
                    .record_consumed_key_down(event.keyCode());
            }
            append_platform_events(&mut events, platform_events);
            let mut lifecycle_events = Vec::new();
            self.push_delegate_lifecycle_events(&mut lifecycle_events)?;
            append_platform_events(&mut events, lifecycle_events);
        }

        let mut lifecycle_events = Vec::new();
        self.push_lifecycle_events(&mut lifecycle_events)?;
        append_platform_events(&mut events, lifecycle_events);
        Ok(events)
    }

    fn push_lifecycle_events(
        &mut self,
        events: &mut Vec<PlatformWindowEvent>,
    ) -> Result<(), PlatformWindowError> {
        self.push_delegate_lifecycle_events(events)?;

        if self.created_event_pending {
            events.push(PlatformWindowEvent::Created);
            self.created_event_pending = false;
        }

        let content_size = self.content_size()?;
        let scale_factor = self.scale_factor();
        if content_size != self.last_content_size {
            self.update_metal_layer_size(content_size, scale_factor);
            self.last_content_size = content_size;
            events.push(PlatformWindowEvent::Resized(content_size));
        }
        if (scale_factor - self.last_scale_factor).abs() > f32::EPSILON {
            self.update_metal_layer_size(content_size, scale_factor);
            self.last_scale_factor = scale_factor;
            events.push(PlatformWindowEvent::ScaleFactorChanged(scale_factor));
        }

        let focused = self.is_focused();
        if focused != self.last_focused {
            self.last_focused = focused;
            events.push(PlatformWindowEvent::FocusChanged(focused));
        }

        let visible = self.is_visible();
        let miniaturized = self.is_miniaturized();
        if miniaturized != self.last_miniaturized {
            self.last_miniaturized = miniaturized;
            events.push(PlatformWindowEvent::Minimized(miniaturized));
        }

        if self.last_visible && !visible && !miniaturized {
            events.push(PlatformWindowEvent::CloseRequested);
        }
        self.last_visible = visible;

        Ok(())
    }

    fn push_delegate_lifecycle_events(
        &mut self,
        events: &mut Vec<PlatformWindowEvent>,
    ) -> Result<(), PlatformWindowError> {
        for event in self.lifecycle_delegate.drain_events() {
            match event {
                MacLifecycleEvent::CloseRequested => {
                    self.last_visible = false;
                    events.push(PlatformWindowEvent::CloseRequested);
                }
                MacLifecycleEvent::QuitRequested => {
                    events.push(PlatformWindowEvent::QuitRequested);
                }
                MacLifecycleEvent::ReopenRequested => {
                    events.push(PlatformWindowEvent::ReopenRequested);
                }
                MacLifecycleEvent::ApplicationActivated(active) => {
                    events.push(PlatformWindowEvent::ApplicationActivated(active));
                }
                MacLifecycleEvent::FocusChanged(focused) => {
                    self.last_focused = focused;
                    events.push(PlatformWindowEvent::FocusChanged(focused));
                }
                MacLifecycleEvent::Resized => {
                    let content_size = self.content_size()?;
                    let scale_factor = self.scale_factor();
                    self.update_metal_layer_size(content_size, scale_factor);
                    self.last_content_size = content_size;
                    events.push(PlatformWindowEvent::Resized(content_size));
                }
                MacLifecycleEvent::Miniaturized(miniaturized) => {
                    self.last_miniaturized = miniaturized;
                    events.push(PlatformWindowEvent::Minimized(miniaturized));
                }
            }
        }

        Ok(())
    }
}

impl PlatformWindow for MacWindow {
    fn platform_name(&self) -> &'static str {
        "macos"
    }

    fn features(&self) -> PlatformWindowFeatures {
        PlatformWindowFeatures {
            lifecycle: true,
            input_events: true,
            dpi: true,
            resizing: true,
            focus: true,
            clipboard: true,
            renderer_attachment: true,
            multi_window: false,
        }
    }

    fn state(&self) -> Result<PlatformWindowState, PlatformWindowError> {
        Ok(PlatformWindowState {
            size: self.content_size()?,
            scale_factor: self.scale_factor(),
            focused: self.is_focused(),
            visible: self.is_visible(),
            renderer_attached: true,
        })
    }

    fn show(&mut self) -> Result<(), PlatformWindowError> {
        self.make_key_and_order_front();
        Ok(())
    }

    fn set_title(&mut self, title: &str) -> Result<(), PlatformWindowError> {
        let title = NSString::from_str(title);
        self.window.setTitle(&title);
        Ok(())
    }

    fn set_size(&mut self, size: Size) -> Result<(), PlatformWindowError> {
        if size.width <= 0.0 || size.height <= 0.0 {
            return Err(PlatformWindowError::invalid_options(
                "window width and height must be greater than zero",
            ));
        }

        let content_size = NSSize::new(size.width as f64, size.height as f64);
        unsafe {
            let _: () = msg_send![&*self.window, setContentSize: content_size];
        }
        self.update_metal_layer_size(size, self.scale_factor());
        Ok(())
    }

    fn set_focus(&mut self, focused: bool) -> Result<(), PlatformWindowError> {
        unsafe {
            if focused {
                self.make_key_and_order_front();
            } else {
                let _: () = msg_send![&*self.window, resignKeyWindow];
            }
        }
        Ok(())
    }

    fn read_clipboard_text(&mut self) -> Result<String, PlatformWindowError> {
        let pasteboard = NSPasteboard::generalPasteboard();
        clipboard_text_or_error(
            pasteboard
                .stringForType(unsafe { NSPasteboardTypeString })
                .map(|value| value.to_string()),
        )
    }

    fn write_clipboard_text(&mut self, text: &str) -> Result<(), PlatformWindowError> {
        let pasteboard = NSPasteboard::generalPasteboard();
        pasteboard.clearContents();
        let value = NSString::from_str(text);
        if pasteboard.setString_forType(&value, unsafe { NSPasteboardTypeString }) {
            Ok(())
        } else {
            Err(PlatformWindowError::backend(
                "macos",
                "failed to write text to the general pasteboard",
            ))
        }
    }

    fn poll_events(&mut self) -> Result<Vec<PlatformWindowEvent>, PlatformWindowError> {
        let mtm = MainThreadMarker::new().ok_or_else(|| {
            PlatformWindowError::backend("macos", "event polling must run on the main thread")
        })?;
        let app = NSApplication::sharedApplication(mtm);
        self.poll_events_for_app(&app, false).map(|events| {
            events
                .into_iter()
                .map(|event| match event {
                    MacWindowEvent::Platform(event) => event,
                    MacWindowEvent::Accessibility(_) => PlatformWindowEvent::RedrawRequested,
                })
                .collect()
        })
    }

    fn renderer_attachment(&self) -> Result<PlatformRendererAttachment, PlatformWindowError> {
        Ok(PlatformRendererAttachment {
            target: PlatformRendererTarget::MetalLayer,
            viewport_size: self.content_size()?,
            scale_factor: self.scale_factor(),
        })
    }

    fn request_redraw(&self) -> Result<(), PlatformWindowError> {
        let content_view = self
            .window
            .contentView()
            .ok_or_else(|| PlatformWindowError::backend("macos", "window has no content view"))?;
        unsafe {
            let _: () = msg_send![&*content_view, setNeedsDisplay: true];
        }
        post_application_event(self.window_number(), MAC_REDRAW_EVENT_DATA)
    }

    fn close(&mut self) -> Result<(), PlatformWindowError> {
        unsafe {
            let _: () = msg_send![&*self.window, close];
        }
        Ok(())
    }
}

fn platform_event_mask() -> NSEventMask {
    NSEventMask::MouseMoved
        | NSEventMask::LeftMouseDown
        | NSEventMask::LeftMouseUp
        | NSEventMask::LeftMouseDragged
        | NSEventMask::RightMouseDown
        | NSEventMask::RightMouseUp
        | NSEventMask::RightMouseDragged
        | NSEventMask::OtherMouseDown
        | NSEventMask::OtherMouseUp
        | NSEventMask::OtherMouseDragged
        | NSEventMask::ScrollWheel
        | NSEventMask::KeyDown
        | NSEventMask::KeyUp
        | NSEventMask::ApplicationDefined
}

fn append_platform_events_from_native_event(
    event: &NSEvent,
    viewport_size: Size,
    events: &mut Vec<PlatformWindowEvent>,
) {
    let event_type = event.r#type();
    if let Some(mouse_event) = mouse_event_from_native_event(event, event_type, viewport_size) {
        events.push(PlatformWindowEvent::Input(PlatformInputEvent::Mouse(
            mouse_event,
        )));
        return;
    }

    if event_type == NSEventType::ScrollWheel {
        events.push(PlatformWindowEvent::Input(PlatformInputEvent::Scroll(
            scroll_event_from_native_event(event, viewport_size),
        )));
        return;
    }

    if event_type == NSEventType::KeyDown {
        events.push(PlatformWindowEvent::Input(PlatformInputEvent::KeyDown(
            key_event_from_native_event(event),
        )));
    } else if event_type == NSEventType::KeyUp {
        events.push(PlatformWindowEvent::Input(PlatformInputEvent::KeyUp(
            key_event_from_native_event(event),
        )));
    }
}

fn mouse_event_from_native_event(
    event: &NSEvent,
    event_type: NSEventType,
    viewport_size: Size,
) -> Option<PlatformMouseEvent> {
    let position = event_position(event, viewport_size);
    let (kind, button) = if event_type == NSEventType::LeftMouseDown {
        (PlatformMouseEventKind::Down, Some(MouseButton::Left))
    } else if event_type == NSEventType::LeftMouseUp {
        (PlatformMouseEventKind::Up, Some(MouseButton::Left))
    } else if event_type == NSEventType::RightMouseDown {
        (PlatformMouseEventKind::Down, Some(MouseButton::Right))
    } else if event_type == NSEventType::RightMouseUp {
        (PlatformMouseEventKind::Up, Some(MouseButton::Right))
    } else if event_type == NSEventType::OtherMouseDown {
        (
            PlatformMouseEventKind::Down,
            Some(other_mouse_button(event)),
        )
    } else if event_type == NSEventType::OtherMouseUp {
        (PlatformMouseEventKind::Up, Some(other_mouse_button(event)))
    } else if event_type == NSEventType::LeftMouseDragged {
        (PlatformMouseEventKind::Move, Some(MouseButton::Left))
    } else if event_type == NSEventType::RightMouseDragged {
        (PlatformMouseEventKind::Move, Some(MouseButton::Right))
    } else if event_type == NSEventType::OtherMouseDragged {
        (
            PlatformMouseEventKind::Move,
            Some(other_mouse_button(event)),
        )
    } else if event_type == NSEventType::MouseMoved {
        (PlatformMouseEventKind::Move, None)
    } else {
        return None;
    };

    Some(PlatformMouseEvent {
        kind,
        position,
        button,
    })
}

fn other_mouse_button(event: &NSEvent) -> MouseButton {
    match event.buttonNumber() {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        number => MouseButton::Other(number.clamp(0, u8::MAX as isize) as u8),
    }
}

fn scroll_event_from_native_event(event: &NSEvent, viewport_size: Size) -> ScrollEvent {
    ScrollEvent {
        position: event_position(event, viewport_size),
        delta_x: event.scrollingDeltaX() as f32,
        delta_y: event.scrollingDeltaY() as f32,
        modifiers: modifiers_from_native_event(event),
    }
}

fn event_position(event: &NSEvent, viewport_size: Size) -> Point {
    let location: NSPoint = event.locationInWindow();
    Point::new(
        location.x as f32,
        (viewport_size.height as f64 - location.y) as f32,
    )
}

fn modifiers_from_native_event(event: &NSEvent) -> Modifiers {
    let flags = event.modifierFlags();
    Modifiers {
        shift: flags.contains(NSEventModifierFlags::Shift),
        ctrl: flags.contains(NSEventModifierFlags::Control),
        alt: flags.contains(NSEventModifierFlags::Option),
        meta: flags.contains(NSEventModifierFlags::Command),
    }
}

fn key_event_from_native_event(event: &NSEvent) -> KeyEvent {
    let modifiers = modifiers_from_native_event(event);
    let mut key_event = KeyEvent::new(KeyCode::Unknown(event.keyCode() as u32), modifiers);
    key_event.is_repeat = event.isARepeat();

    if let Some(chars) = event.charactersIgnoringModifiers()
        && let Some(ch) = chars.to_string().chars().next()
    {
        key_event.key = keycode_from_char(ch);
    }

    if let Some(chars) = event.characters()
        && let Some(ch) = chars.to_string().chars().next()
        && !ch.is_control()
    {
        key_event.char = Some(ch);
    }

    key_event
}

fn clipboard_text_or_error(text: Option<String>) -> Result<String, PlatformWindowError> {
    text.ok_or_else(|| {
        PlatformWindowError::backend("macos", "general pasteboard does not contain text")
    })
}

fn keycode_from_char(ch: char) -> KeyCode {
    match ch {
        '\r' | '\n' => KeyCode::Enter,
        '\u{7f}' => KeyCode::Backspace,
        '\u{1b}' => KeyCode::Escape,
        '\t' => KeyCode::Tab,
        ' ' => KeyCode::Space,
        '0' => KeyCode::Key0,
        '1' => KeyCode::Key1,
        '2' => KeyCode::Key2,
        '3' => KeyCode::Key3,
        '4' => KeyCode::Key4,
        '5' => KeyCode::Key5,
        '6' => KeyCode::Key6,
        '7' => KeyCode::Key7,
        '8' => KeyCode::Key8,
        '9' => KeyCode::Key9,
        'a' | 'A' => KeyCode::A,
        'b' | 'B' => KeyCode::B,
        'c' | 'C' => KeyCode::C,
        'd' | 'D' => KeyCode::D,
        'e' | 'E' => KeyCode::E,
        'f' | 'F' => KeyCode::F,
        'g' | 'G' => KeyCode::G,
        'h' | 'H' => KeyCode::H,
        'i' | 'I' => KeyCode::I,
        'j' | 'J' => KeyCode::J,
        'k' | 'K' => KeyCode::K,
        'l' | 'L' => KeyCode::L,
        'm' | 'M' => KeyCode::M,
        'n' | 'N' => KeyCode::N,
        'o' | 'O' => KeyCode::O,
        'p' | 'P' => KeyCode::P,
        'q' | 'Q' => KeyCode::Q,
        'r' | 'R' => KeyCode::R,
        's' | 'S' => KeyCode::S,
        't' | 'T' => KeyCode::T,
        'u' | 'U' => KeyCode::U,
        'v' | 'V' => KeyCode::V,
        'w' | 'W' => KeyCode::W,
        'x' | 'X' => KeyCode::X,
        'y' | 'Y' => KeyCode::Y,
        'z' | 'Z' => KeyCode::Z,
        _ => {
            let code = ch as u32;
            match code {
                0xF700 => KeyCode::ArrowUp,
                0xF701 => KeyCode::ArrowDown,
                0xF702 => KeyCode::ArrowLeft,
                0xF703 => KeyCode::ArrowRight,
                0xF729 => KeyCode::Home,
                0xF72B => KeyCode::End,
                0xF72C => KeyCode::PageUp,
                0xF72D => KeyCode::PageDown,
                0xF728 => KeyCode::Delete,
                _ => KeyCode::Unknown(code),
            }
        }
    }
}

/// Create a macOS window with a Metal layer
pub unsafe fn create_window(
    options: &WindowOptions,
    device: &Device,
    mtm: MainThreadMarker,
) -> Result<MacWindow, PlatformWindowError> {
    validate_window_options(options)?;

    // Define window frame
    let frame = NSRect::new(
        NSPoint::new(100.0, 100.0),
        NSSize::new(options.size.width as f64, options.size.height as f64),
    );

    // Window style
    let mut style =
        NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Miniaturizable;
    if options.resizable {
        style |= NSWindowStyleMask::Resizable;
    }

    // Create window
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            style,
            objc2_app_kit::NSBackingStoreType(2), // NSBackingStoreBuffered = 2
            false,
        )
    };
    unsafe {
        window.setReleasedWhenClosed(false);
    }

    // Set title
    let title = NSString::from_str(&options.title);
    window.setTitle(&title);

    let content_frame = NSRect::new(NSPoint::new(0.0, 0.0), frame.size);
    let content_view = RuiContentView::new(content_frame, mtm);
    window.setContentView(Some(&content_view));

    // Create Metal layer
    let metal_layer = CAMetalLayer::new();

    // Set the Metal device on the layer
    let device_ptr = device.as_ptr() as *mut objc2::runtime::AnyObject;
    let _: () = msg_send![&*metal_layer, setDevice: device_ptr];

    // Configure layer
    let _: () = msg_send![&*metal_layer, setPixelFormat: 80u64]; // MTLPixelFormatBGRA8Unorm = 80
    let _: () = msg_send![&*metal_layer, setFramebufferOnly: true];

    let scale_factor: f64 = unsafe { msg_send![&*window, backingScaleFactor] };

    // Set layer size
    let drawable_size = NSSize::new(
        options.size.width as f64 * scale_factor,
        options.size.height as f64 * scale_factor,
    );
    let _: () = msg_send![&*metal_layer, setDrawableSize: drawable_size];

    // Set content scale factor
    let _: () = msg_send![&*metal_layer, setContentsScale: scale_factor];

    // Set layer on view
    let _: () = msg_send![&*content_view, setWantsLayer: true];
    let _: () = msg_send![&*content_view, setLayer: &*metal_layer];
    let _: () = msg_send![&*window, setAcceptsMouseMovedEvents: true];
    let _: bool = msg_send![&*window, makeFirstResponder: &*content_view];

    let lifecycle_delegate = MacLifecycleDelegate::new(mtm);
    lifecycle_delegate.install_as_window_delegate(&window);

    // Center window on screen
    window.center();

    let accessibility_bridge = MacAccessibilityBridge::attached_to(
        Retained::into_super(content_view.clone()),
        window.windowNumber(),
    );

    Ok(MacWindow {
        window,
        content_view,
        metal_layer,
        accessibility_bridge,
        last_content_size: options.size,
        last_scale_factor: scale_factor as f32,
        last_focused: false,
        last_visible: false,
        last_miniaturized: false,
        created_event_pending: true,
        suppressed_key_ups: SuppressedKeyUps::default(),
        lifecycle_delegate,
    })
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MacWindowBackend;

impl MacWindowBackend {
    pub fn new() -> Self {
        Self
    }

    pub fn features(&self) -> PlatformWindowFeatures {
        PlatformWindowFeatures {
            lifecycle: true,
            input_events: true,
            dpi: true,
            resizing: true,
            focus: true,
            clipboard: true,
            renderer_attachment: true,
            multi_window: false,
        }
    }
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
