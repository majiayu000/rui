//! winit window events as backend-neutral [`PlatformWindowEvent`]s.

use crate::core::event::{KeyCode, KeyEvent, Modifiers, MouseButton, ScrollEvent};
use crate::core::geometry::{Point, Size};
use crate::platform::window::{
    PlatformInputEvent, PlatformMouseEvent, PlatformMouseEventKind, PlatformWindowEvent,
};
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key, NamedKey};

/// Logical pixels one scroll-wheel line moves, for devices reporting lines.
const SCROLL_LINE_HEIGHT: f32 = 20.0;

/// What translating later events needs from earlier ones: winit reports the
/// pointer position and modifiers apart from the button and key events.
#[derive(Debug, Clone)]
pub(super) struct InputState {
    pub(super) scale_factor: f64,
    cursor: Point,
    modifiers: Modifiers,
}

impl InputState {
    pub(super) fn new(scale_factor: f64) -> Self {
        Self {
            scale_factor,
            cursor: Point::new(0.0, 0.0),
            modifiers: Modifiers::none(),
        }
    }

    /// Logical size of `physical` device pixels.
    pub(super) fn logical_size(&self, physical: winit::dpi::PhysicalSize<u32>) -> Size {
        let logical = physical.to_logical::<f32>(self.scale_factor);
        Size::new(logical.width, logical.height)
    }

    /// The platform event `event` is, if it is one the frame driver uses.
    pub(super) fn translate(&mut self, event: &WindowEvent) -> Option<PlatformWindowEvent> {
        let input = |input| Some(PlatformWindowEvent::Input(input));
        match event {
            WindowEvent::Resized(size) => {
                Some(PlatformWindowEvent::Resized(self.logical_size(*size)))
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.scale_factor = *scale_factor;
                Some(PlatformWindowEvent::ScaleFactorChanged(
                    *scale_factor as f32,
                ))
            }
            WindowEvent::Focused(focused) => {
                if !focused {
                    self.modifiers = Modifiers::none();
                }
                Some(PlatformWindowEvent::FocusChanged(*focused))
            }
            WindowEvent::Occluded(occluded) => Some(PlatformWindowEvent::Minimized(*occluded)),
            WindowEvent::CloseRequested => Some(PlatformWindowEvent::CloseRequested),
            WindowEvent::RedrawRequested => Some(PlatformWindowEvent::RedrawRequested),
            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();
                self.modifiers = Modifiers {
                    shift: state.shift_key(),
                    ctrl: state.control_key(),
                    alt: state.alt_key(),
                    meta: state.super_key(),
                };
                None
            }
            WindowEvent::CursorMoved { position, .. } => {
                let position = position.to_logical::<f32>(self.scale_factor);
                self.cursor = Point::new(position.x, position.y);
                input(self.mouse(PlatformMouseEventKind::Move, None))
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let kind = match state {
                    ElementState::Pressed => PlatformMouseEventKind::Down,
                    ElementState::Released => PlatformMouseEventKind::Up,
                };
                input(self.mouse(kind, Some(mouse_button(*button))))
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (delta_x, delta_y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => {
                        (x * SCROLL_LINE_HEIGHT, y * SCROLL_LINE_HEIGHT)
                    }
                    MouseScrollDelta::PixelDelta(delta) => {
                        let delta = delta.to_logical::<f32>(self.scale_factor);
                        (delta.x, delta.y)
                    }
                };
                input(PlatformInputEvent::Scroll(ScrollEvent {
                    position: self.cursor,
                    delta_x,
                    delta_y,
                    modifiers: self.modifiers,
                }))
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let mut key = KeyEvent::new(key_code(&event.logical_key), self.modifiers);
                key.is_repeat = event.repeat;
                key.char = event
                    .text
                    .as_ref()
                    .and_then(|text| text.chars().next())
                    .filter(|ch| !ch.is_control());
                input(match event.state {
                    ElementState::Pressed => PlatformInputEvent::KeyDown(key),
                    ElementState::Released => PlatformInputEvent::KeyUp(key),
                })
            }
            _ => None,
        }
    }

    fn mouse(
        &self,
        kind: PlatformMouseEventKind,
        button: Option<MouseButton>,
    ) -> PlatformInputEvent {
        PlatformInputEvent::Mouse(PlatformMouseEvent {
            kind,
            position: self.cursor,
            button,
        })
    }
}

fn mouse_button(button: winit::event::MouseButton) -> MouseButton {
    match button {
        winit::event::MouseButton::Left => MouseButton::Left,
        winit::event::MouseButton::Right => MouseButton::Right,
        winit::event::MouseButton::Middle => MouseButton::Middle,
        winit::event::MouseButton::Back => MouseButton::Other(3),
        winit::event::MouseButton::Forward => MouseButton::Other(4),
        winit::event::MouseButton::Other(number) => {
            MouseButton::Other(number.min(u16::from(u8::MAX)) as u8)
        }
    }
}

fn key_code(key: &Key) -> KeyCode {
    match key {
        Key::Named(named) => named_key_code(*named),
        Key::Character(text) => text
            .chars()
            .next()
            .map_or(KeyCode::Unknown(0), character_key_code),
        _ => KeyCode::Unknown(0),
    }
}

fn named_key_code(key: NamedKey) -> KeyCode {
    match key {
        NamedKey::Escape => KeyCode::Escape,
        NamedKey::Tab => KeyCode::Tab,
        NamedKey::CapsLock => KeyCode::CapsLock,
        NamedKey::Shift => KeyCode::Shift,
        NamedKey::Control => KeyCode::Control,
        NamedKey::Alt => KeyCode::Alt,
        NamedKey::Super | NamedKey::Meta => KeyCode::Meta,
        NamedKey::Space => KeyCode::Space,
        NamedKey::Enter => KeyCode::Enter,
        NamedKey::Backspace => KeyCode::Backspace,
        NamedKey::Delete => KeyCode::Delete,
        NamedKey::ArrowUp => KeyCode::ArrowUp,
        NamedKey::ArrowDown => KeyCode::ArrowDown,
        NamedKey::ArrowLeft => KeyCode::ArrowLeft,
        NamedKey::ArrowRight => KeyCode::ArrowRight,
        NamedKey::Home => KeyCode::Home,
        NamedKey::End => KeyCode::End,
        NamedKey::PageUp => KeyCode::PageUp,
        NamedKey::PageDown => KeyCode::PageDown,
        NamedKey::Insert => KeyCode::Insert,
        NamedKey::F1 => KeyCode::F1,
        NamedKey::F2 => KeyCode::F2,
        NamedKey::F3 => KeyCode::F3,
        NamedKey::F4 => KeyCode::F4,
        NamedKey::F5 => KeyCode::F5,
        NamedKey::F6 => KeyCode::F6,
        NamedKey::F7 => KeyCode::F7,
        NamedKey::F8 => KeyCode::F8,
        NamedKey::F9 => KeyCode::F9,
        NamedKey::F10 => KeyCode::F10,
        NamedKey::F11 => KeyCode::F11,
        NamedKey::F12 => KeyCode::F12,
        NamedKey::F13 => KeyCode::F13,
        NamedKey::F14 => KeyCode::F14,
        NamedKey::F15 => KeyCode::F15,
        NamedKey::F16 => KeyCode::F16,
        NamedKey::F17 => KeyCode::F17,
        NamedKey::F18 => KeyCode::F18,
        NamedKey::F19 => KeyCode::F19,
        NamedKey::F20 => KeyCode::F20,
        _ => KeyCode::Unknown(0),
    }
}

fn character_key_code(ch: char) -> KeyCode {
    const LETTERS: [KeyCode; 26] = [
        KeyCode::A,
        KeyCode::B,
        KeyCode::C,
        KeyCode::D,
        KeyCode::E,
        KeyCode::F,
        KeyCode::G,
        KeyCode::H,
        KeyCode::I,
        KeyCode::J,
        KeyCode::K,
        KeyCode::L,
        KeyCode::M,
        KeyCode::N,
        KeyCode::O,
        KeyCode::P,
        KeyCode::Q,
        KeyCode::R,
        KeyCode::S,
        KeyCode::T,
        KeyCode::U,
        KeyCode::V,
        KeyCode::W,
        KeyCode::X,
        KeyCode::Y,
        KeyCode::Z,
    ];
    const DIGITS: [KeyCode; 10] = [
        KeyCode::Key0,
        KeyCode::Key1,
        KeyCode::Key2,
        KeyCode::Key3,
        KeyCode::Key4,
        KeyCode::Key5,
        KeyCode::Key6,
        KeyCode::Key7,
        KeyCode::Key8,
        KeyCode::Key9,
    ];
    let lower = ch.to_ascii_lowercase();
    match lower {
        'a'..='z' => LETTERS[(lower as u8 - b'a') as usize],
        '0'..='9' => DIGITS[(lower as u8 - b'0') as usize],
        ' ' => KeyCode::Space,
        '-' => KeyCode::Minus,
        '=' => KeyCode::Equal,
        '[' => KeyCode::BracketLeft,
        ']' => KeyCode::BracketRight,
        '\\' => KeyCode::Backslash,
        ';' => KeyCode::Semicolon,
        '\'' => KeyCode::Quote,
        ',' => KeyCode::Comma,
        '.' => KeyCode::Period,
        '/' => KeyCode::Slash,
        '`' => KeyCode::Backquote,
        _ => KeyCode::Unknown(ch as u32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_digits_and_named_keys_map_to_key_codes() {
        assert_eq!(key_code(&Key::Character("A".into())), KeyCode::A);
        assert_eq!(key_code(&Key::Character("7".into())), KeyCode::Key7);
        assert_eq!(key_code(&Key::Named(NamedKey::Enter)), KeyCode::Enter);
        assert_eq!(key_code(&Key::Named(NamedKey::F12)), KeyCode::F12);
    }

    #[test]
    fn sizes_are_logical_at_the_window_scale() {
        let mut state = InputState::new(2.0);
        let Some(PlatformWindowEvent::Resized(size)) = state.translate(&WindowEvent::Resized(
            winit::dpi::PhysicalSize::new(200, 100),
        )) else {
            panic!("a resize should translate to Resized");
        };
        assert_eq!(size, Size::new(100.0, 50.0));
    }
}
