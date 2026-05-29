//! macOS window creation

use crate::core::event::{KeyCode, KeyEvent, Modifiers, MouseButton, ScrollEvent};
use crate::core::geometry::{Point, Size};
use crate::core::window::WindowOptions;
use crate::platform::window::{
    PlatformImeEvent, PlatformInputEvent, PlatformMouseEvent, PlatformMouseEventKind,
    PlatformRendererAttachment, PlatformRendererTarget, PlatformWindow, PlatformWindowError,
    PlatformWindowEvent, PlatformWindowFeatures, PlatformWindowState, validate_window_options,
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

pub(crate) const MAC_REDRAW_EVENT_DATA: isize = 0x5255_4952;
const MAC_REDRAW_EVENT_SUBTYPE: i16 = 0;

pub struct MacWindow {
    window: Retained<NSWindow>,
    metal_layer: Retained<CAMetalLayer>,
    last_content_size: Size,
    last_scale_factor: f32,
    last_focused: bool,
    last_visible: bool,
    created_event_pending: bool,
}

impl MacWindow {
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
    ) -> Result<Vec<PlatformWindowEvent>, PlatformWindowError> {
        let mut events = Vec::new();
        self.push_lifecycle_events(&mut events)?;

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
                continue;
            }

            let event_type = event.r#type();
            if event_type == NSEventType::ApplicationDefined
                && event.data1() == MAC_REDRAW_EVENT_DATA
            {
                events.push(PlatformWindowEvent::RedrawRequested);
                continue;
            }

            append_platform_events_from_native_event(&event, self.last_content_size, &mut events);
            app.sendEvent(&event);
        }

        self.push_lifecycle_events(&mut events)?;
        Ok(events)
    }

    fn push_lifecycle_events(
        &mut self,
        events: &mut Vec<PlatformWindowEvent>,
    ) -> Result<(), PlatformWindowError> {
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
        if self.last_visible && !visible {
            events.push(PlatformWindowEvent::CloseRequested);
        }
        self.last_visible = visible;

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
        self.poll_events_for_app(&app, false)
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
        let mtm = MainThreadMarker::new().ok_or_else(|| {
            PlatformWindowError::backend("macos", "redraw requested off the main thread")
        })?;
        let app = NSApplication::sharedApplication(mtm);
        unsafe {
            let _: () = msg_send![&*content_view, setNeedsDisplay: true];
        }
        let event =
            NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
                NSEventType::ApplicationDefined,
                NSPoint::new(0.0, 0.0),
                NSEventModifierFlags::empty(),
                NSDate::timeIntervalSinceReferenceDate_class(),
                self.window_number(),
                None,
                MAC_REDRAW_EVENT_SUBTYPE,
                MAC_REDRAW_EVENT_DATA,
                0,
            )
            .ok_or_else(|| PlatformWindowError::backend("macos", "failed to create redraw event"))?;
        app.postEvent_atStart(&event, false);
        Ok(())
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
        let mut key_event = key_event_from_native_event(event);
        let committed_text = committed_text_from_native_event(event);
        let commit_represented_by_key = committed_text
            .as_deref()
            .is_some_and(|text| committed_text_matches_key_event(text, &key_event));
        if committed_text.is_some() && !commit_represented_by_key {
            key_event.char = None;
        }
        events.push(PlatformWindowEvent::Input(PlatformInputEvent::KeyDown(
            key_event,
        )));
        if let Some(text) = committed_text {
            if commit_represented_by_key {
                return;
            }
            events.push(PlatformWindowEvent::Input(PlatformInputEvent::Ime(
                PlatformImeEvent::Commit(text),
            )));
        }
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

fn committed_text_from_native_event(event: &NSEvent) -> Option<String> {
    if event.isARepeat() {
        return None;
    }

    let text = event.characters()?.to_string();
    if text.is_empty() || text.chars().all(char::is_control) {
        return None;
    }
    Some(text)
}

fn committed_text_matches_key_event(text: &str, event: &KeyEvent) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    chars.next().is_none() && event.char == Some(first)
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

    // Set title
    let title = NSString::from_str(&options.title);
    window.setTitle(&title);

    // Get content view
    let content_view = window.contentView().expect("No content view");

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

    // Center window on screen
    window.center();

    Ok(MacWindow {
        window,
        metal_layer,
        last_content_size: options.size,
        last_scale_factor: scale_factor as f32,
        last_focused: false,
        last_visible: false,
        created_event_pending: true,
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_text_or_error_rejects_missing_text() {
        assert_eq!(
            clipboard_text_or_error(None),
            Err(PlatformWindowError::backend(
                "macos",
                "general pasteboard does not contain text",
            ))
        );
        assert_eq!(
            clipboard_text_or_error(Some(String::new())),
            Ok(String::new())
        );
        assert_eq!(
            clipboard_text_or_error(Some("copied".to_string())),
            Ok("copied".to_string())
        );
    }

    #[test]
    fn committed_text_only_matches_single_char_key_events() {
        let a_key = KeyEvent::new(KeyCode::A, Modifiers::none()).with_char('a');
        assert!(committed_text_matches_key_event("a", &a_key));
        assert!(!committed_text_matches_key_event("ab", &a_key));
        assert!(!committed_text_matches_key_event("", &a_key));

        let ime_key = KeyEvent::new(KeyCode::Unknown(0), Modifiers::none());
        assert!(!committed_text_matches_key_event("好", &ime_key));
    }
}
