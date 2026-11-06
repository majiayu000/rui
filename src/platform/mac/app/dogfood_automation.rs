//! Scripted input for the native dogfood run (`RUI_NATIVE_DOGFOOD_*`).

use super::*;

pub(super) const NATIVE_DOGFOOD_INPUT_ID: ElementId = ElementId(29_001);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NativeDogfoodAutomationPhase {
    Warmup,
    Interact,
    Minimize,
    Reopen,
    Submit,
    Done,
}

pub(super) struct NativeDogfoodAutomation {
    phase: NativeDogfoodAutomationPhase,
    text: String,
}

impl NativeDogfoodAutomation {
    pub(super) fn load_from_environment() -> Option<Self> {
        if std::env::var_os("RUI_NATIVE_DOGFOOD_AUTOMATION").is_none() {
            return None;
        }

        let text = std::env::var("RUI_NATIVE_DOGFOOD_TEXT")
            .unwrap_or_else(|_| String::from("rui-native-dogfood"));
        Some(Self {
            phase: NativeDogfoodAutomationPhase::Warmup,
            text,
        })
    }

    pub(super) fn append_events(
        &mut self,
        window: &crate::platform::mac::window::MacWindow,
        scene: &crate::renderer::Scene,
        events: &mut Vec<MacWindowEvent>,
    ) -> Option<ElementId> {
        let click_point = || {
            scene
                .layers()
                .iter()
                .flat_map(|layer| layer.hit_regions())
                .find(|region| region.id == NATIVE_DOGFOOD_INPUT_ID)
                .expect("native dogfood input must have a visible hit region")
                .visible_bounds
                .center()
        };
        let focused_element = match self.phase {
            NativeDogfoodAutomationPhase::Warmup => {
                self.phase = NativeDogfoodAutomationPhase::Interact;
                request_automation_redraw(window);
                None
            }
            NativeDogfoodAutomationPhase::Interact => {
                append_pointer_click(events, click_point());
                append_text_keys(events, &self.text);
                self.phase = NativeDogfoodAutomationPhase::Minimize;
                request_automation_redraw(window);
                Some(NATIVE_DOGFOOD_INPUT_ID)
            }
            NativeDogfoodAutomationPhase::Minimize => {
                window.set_minimized(true);
                self.phase = NativeDogfoodAutomationPhase::Reopen;
                request_automation_redraw(window);
                None
            }
            NativeDogfoodAutomationPhase::Reopen => {
                // AppKit can animate minimization asynchronously. Do not count a
                // request as a completed transition; the script owns the timeout.
                if !window.is_miniaturized() {
                    request_automation_redraw(window);
                    return None;
                }
                eprintln!("native dogfood: AppKit minimized=true");
                window.set_minimized(false);
                self.phase = NativeDogfoodAutomationPhase::Submit;
                request_automation_redraw(window);
                None
            }
            NativeDogfoodAutomationPhase::Submit => {
                if window.is_miniaturized() || !window.is_visible() {
                    request_automation_redraw(window);
                    return None;
                }
                eprintln!("native dogfood: AppKit restored=true");
                append_pointer_click(events, click_point());
                append_key_pair(
                    events,
                    KeyEvent::new(KeyCode::Enter, Modifiers::none()).with_char('\n'),
                );
                self.phase = NativeDogfoodAutomationPhase::Done;
                request_automation_redraw(window);
                Some(NATIVE_DOGFOOD_INPUT_ID)
            }
            NativeDogfoodAutomationPhase::Done => None,
        };
        focused_element
    }
}

pub(super) fn append_pointer_click(events: &mut Vec<MacWindowEvent>, position: Point) {
    events.push(MacWindowEvent::Platform(PlatformWindowEvent::Input(
        PlatformInputEvent::Mouse(PlatformMouseEvent {
            kind: PlatformMouseEventKind::Down,
            position,
            button: Some(MouseButton::Left),
        }),
    )));
    events.push(MacWindowEvent::Platform(PlatformWindowEvent::Input(
        PlatformInputEvent::Mouse(PlatformMouseEvent {
            kind: PlatformMouseEventKind::Up,
            position,
            button: Some(MouseButton::Left),
        }),
    )));
}

pub(super) fn append_text_keys(events: &mut Vec<MacWindowEvent>, text: &str) {
    for ch in text.chars() {
        let key = key_code_for_dogfood_char(ch);
        append_key_pair(events, KeyEvent::new(key, Modifiers::none()).with_char(ch));
    }
}

pub(super) fn append_key_pair(events: &mut Vec<MacWindowEvent>, event: KeyEvent) {
    events.push(MacWindowEvent::Platform(PlatformWindowEvent::Input(
        PlatformInputEvent::KeyDown(event.clone()),
    )));
    events.push(MacWindowEvent::Platform(PlatformWindowEvent::Input(
        PlatformInputEvent::KeyUp(event),
    )));
}

pub(super) fn key_code_for_dogfood_char(ch: char) -> KeyCode {
    match ch {
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
        '-' => KeyCode::Minus,
        '_' => KeyCode::Minus,
        ' ' => KeyCode::Space,
        other => KeyCode::Unknown(other as u32),
    }
}

pub(super) fn request_automation_redraw<W: PlatformWindow>(window: &W) {
    if let Err(err) = window.request_redraw() {
        panic!("native dogfood automation failed to request redraw: {err}");
    }
}
