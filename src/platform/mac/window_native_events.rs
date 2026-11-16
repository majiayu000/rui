//! AppKit events turned into rui input events.

use super::*;

pub(super) fn append_platform_events_from_native_event(
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

pub(super) fn mouse_event_from_native_event(
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
        modifiers: modifiers_from_native_event(event),
    })
}

pub(super) fn other_mouse_button(event: &NSEvent) -> MouseButton {
    match event.buttonNumber() {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        number => MouseButton::Other(number.clamp(0, u8::MAX as isize) as u8),
    }
}

pub(super) fn scroll_event_from_native_event(event: &NSEvent, viewport_size: Size) -> ScrollEvent {
    ScrollEvent {
        position: event_position(event, viewport_size),
        delta_x: event.scrollingDeltaX() as f32,
        delta_y: event.scrollingDeltaY() as f32,
        modifiers: modifiers_from_native_event(event),
    }
}

pub(super) fn event_position(event: &NSEvent, viewport_size: Size) -> Point {
    let location: NSPoint = event.locationInWindow();
    Point::new(
        location.x as f32,
        (viewport_size.height as f64 - location.y) as f32,
    )
}

pub(super) fn modifiers_from_native_event(event: &NSEvent) -> Modifiers {
    let flags = event.modifierFlags();
    Modifiers {
        shift: flags.contains(NSEventModifierFlags::Shift),
        ctrl: flags.contains(NSEventModifierFlags::Control),
        alt: flags.contains(NSEventModifierFlags::Option),
        meta: flags.contains(NSEventModifierFlags::Command),
    }
}

pub(super) fn key_event_from_native_event(event: &NSEvent) -> KeyEvent {
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

pub(super) fn keycode_from_char(ch: char) -> KeyCode {
    match ch {
        '\r' | '\n' => KeyCode::Enter,
        '\u{7f}' => KeyCode::Backspace,
        '\u{1b}' => KeyCode::Escape,
        '\t' | '\u{19}' => KeyCode::Tab,
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
