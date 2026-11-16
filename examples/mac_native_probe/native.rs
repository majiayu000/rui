//! Main-queue scheduling, synthesized NSEvents and NSAccessibility reads.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadMarker, msg_send, sel};
use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType, NSWindow};
use objc2_foundation::{
    NSArray, NSNumber, NSPoint, NSProcessInfo, NSRange, NSRect, NSSize, NSString,
};
use std::ffi::c_void;

unsafe extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_time(when: u64, delta: i64) -> u64;
    fn dispatch_after_f(
        when: u64,
        queue: *const c_void,
        context: *mut c_void,
        work: extern "C" fn(*mut c_void),
    );
}

/// Run `work` on the main queue after `ms`; rui's event loop drains it while it
/// waits for events.
pub fn after(ms: u64, work: extern "C" fn(*mut c_void)) {
    // SAFETY: the main queue is a static owned by libdispatch.
    unsafe {
        let when = dispatch_time(0, (ms * 1_000_000) as i64);
        dispatch_after_f(when, &_dispatch_main_q, std::ptr::null_mut(), work);
    }
}

pub fn mtm() -> MainThreadMarker {
    MainThreadMarker::new().expect("probe steps run on the main thread")
}

pub fn app() -> Retained<NSApplication> {
    NSApplication::sharedApplication(mtm())
}

pub fn window() -> Option<Retained<NSWindow>> {
    app().windows().iter().find(|window| window.isVisible())
}

pub fn content_view(window: &NSWindow) -> Option<Retained<AnyObject>> {
    window
        .contentView()
        .map(|view| Retained::into_super(Retained::into_super(Retained::into_super(view))))
}

fn uptime() -> f64 {
    NSProcessInfo::processInfo().systemUptime()
}

/// One key on the ANSI layout: keyCode, the characters it types and whether
/// Shift is down.
#[derive(Clone, Copy, Debug)]
pub struct Key {
    pub code: u16,
    pub chars: &'static str,
    pub flags: NSEventModifierFlags,
}

pub const SPACE: Key = Key::plain(49, " ");
pub const RETURN: Key = Key::plain(36, "\r");
pub const TAB: Key = Key::plain(48, "\t");
pub const ESCAPE: Key = Key::plain(53, "\u{1b}");
pub const BACKSPACE: Key = Key::plain(51, "\u{7f}");
pub const ONE: Key = Key::plain(18, "1");

impl Key {
    pub const fn plain(code: u16, chars: &'static str) -> Self {
        Self {
            code,
            chars,
            flags: NSEventModifierFlags::empty(),
        }
    }

    pub fn letter(ch: char) -> Self {
        const LETTERS: &str = "abcdefghijklmnopqrstuvwxyz";
        const CODES: [u16; 26] = [
            0, 11, 8, 2, 14, 3, 5, 4, 34, 38, 40, 37, 46, 45, 31, 35, 12, 15, 1, 17, 32, 9, 13, 7,
            16, 6,
        ];
        let index = LETTERS
            .find(ch)
            .unwrap_or_else(|| panic!("no key for {ch:?}"));
        Self::plain(CODES[index], &LETTERS[index..index + 1])
    }
}

/// Which kind of synthesized key event the probe sends: `keyEventWithType:`
/// queued for rui's loop (the default), or with `RUI_PROBE_EVENTS=cg`
/// CGEvent-backed key downs, as a hardware keystroke has, sent to the window.
/// Those carry no window number, so rui sees only their text, not the raw
/// key down: Tab and Backspace do nothing in that mode.
pub fn cg_events() -> bool {
    std::env::var("RUI_PROBE_EVENTS").is_ok_and(|kind| kind == "cg")
}

/// `RUI_PROBE_EVENTS=hid`: post keys at the HID event tap so they travel the
/// window server and the real input method like typing does.
pub fn hid_events() -> bool {
    std::env::var("RUI_PROBE_EVENTS").is_ok_and(|kind| kind == "hid")
}

fn post_hid_pair(code: u16, flags: u64) {
    for down in [true, false] {
        if !crate::cg::post_hid(code, down, flags) {
            eprintln!("probe: CoreGraphics refused a HID key event");
        }
    }
}

fn key_event(window: &NSWindow, kind: NSEventType, key: Key) -> Retained<NSEvent> {
    let chars = NSString::from_str(key.chars);
    NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        kind,
        NSPoint::new(0.0, 0.0),
        key.flags,
        uptime(),
        window.windowNumber(),
        None,
        &chars,
        &chars,
        false,
        key.code,
    )
    .expect("key NSEvent")
}

/// Queue a key down and up for the window; rui's loop dequeues them and sends
/// them through `NSApp sendEvent:` like real keystrokes.
pub fn post_key(window: &NSWindow, key: Key) {
    if hid_events() {
        post_hid_pair(key.code, key.flags.bits() as u64);
        return;
    }
    let app = app();
    if cg_events() {
        // A CGEvent-backed NSEvent carries no window number, so rui's loop
        // would not route it to the window: send the key down to the window
        // directly. The queued key up that follows is routed, and rui drains
        // the input method's commands with it.
        let flags = key.flags.bits() as u64;
        match crate::cg::key_event(window.windowNumber(), key.code, true, flags, key.chars) {
            Some(event) => window.sendEvent(&event),
            None => eprintln!("probe: CoreGraphics refused a key event"),
        }
        app.postEvent_atStart(&key_event(window, NSEventType::KeyUp, key), false);
        return;
    }
    app.postEvent_atStart(&key_event(window, NSEventType::KeyDown, key), false);
    app.postEvent_atStart(&key_event(window, NSEventType::KeyUp, key), false);
}

pub fn post_key_up(window: &NSWindow, key: Key) {
    if hid_events() {
        return;
    }
    app().postEvent_atStart(&key_event(window, NSEventType::KeyUp, key), false);
}

/// Hand a key down straight to the view's NSTextInputContext, bypassing rui's
/// `keyDown:`; the fallback when queued keys do not reach the input method.
pub fn handle_key_in_context(window: &NSWindow, key: Key) -> bool {
    let Some(view) = content_view(window) else {
        return false;
    };
    let context: Option<Retained<AnyObject>> = unsafe { msg_send![&*view, inputContext] };
    let Some(context) = context else {
        return false;
    };
    let event = key_event(window, NSEventType::KeyDown, key);
    unsafe { msg_send![&*context, handleEvent: &*event] }
}

/// Press and release Shift alone (a flags-changed pair).
pub fn post_shift_tap(window: &NSWindow) {
    if hid_events() {
        let shift = NSEventModifierFlags::Shift.bits() as u64;
        for (down, flags) in [(true, shift), (false, 0)] {
            if !crate::cg::post_hid(56, down, flags) {
                eprintln!("probe: CoreGraphics refused a HID Shift event");
            }
        }
        return;
    }
    let app = app();
    if cg_events() {
        let shift = NSEventModifierFlags::Shift.bits() as u64;
        for (down, flags) in [(true, shift), (false, 0)] {
            match crate::cg::key_event(window.windowNumber(), 56, down, flags, "") {
                Some(event) => window.sendEvent(&event),
                None => eprintln!("probe: CoreGraphics refused a Shift event"),
            }
        }
        // A routed event, so rui drains what the input method sent.
        app.postEvent_atStart(
            &key_event(window, NSEventType::KeyUp, Key::plain(56, "")),
            false,
        );
        return;
    }
    for flags in [NSEventModifierFlags::Shift, NSEventModifierFlags::empty()] {
        let empty = NSString::from_str("");
        let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            NSEventType::FlagsChanged,
            NSPoint::new(0.0, 0.0),
            flags,
            uptime(),
            window.windowNumber(),
            None,
            &empty,
            &empty,
            false,
            56,
        );
        match event {
            Some(event) => app.postEvent_atStart(&event, false),
            None => eprintln!("probe: AppKit refused a flags-changed event"),
        }
    }
}

/// Click at the centre of a screen rectangle, as a queued mouse down and up.
pub fn post_click(window: &NSWindow, screen_rect: NSRect) {
    let rect = window.convertRectFromScreen(screen_rect);
    let point = NSPoint::new(
        rect.origin.x + rect.size.width / 2.0,
        rect.origin.y + rect.size.height / 2.0,
    );
    let app = app();
    for kind in [NSEventType::LeftMouseDown, NSEventType::LeftMouseUp] {
        let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            kind,
            point,
            NSEventModifierFlags::empty(),
            uptime(),
            window.windowNumber(),
            None,
            0,
            1,
            1.0,
        )
        .expect("mouse NSEvent");
        app.postEvent_atStart(&event, false);
    }
}

// ---- NSAccessibility reads ------------------------------------------------

pub type Ax = Retained<AnyObject>;

pub fn responds(object: &AnyObject, selector: Sel) -> bool {
    object.class().responds_to(selector)
}

pub fn ax_children(object: &AnyObject) -> Vec<Ax> {
    ax_attr(object, "AXChildren")
        .and_then(|children| children.downcast::<NSArray>().ok())
        .map(|children| children.to_vec())
        .unwrap_or_default()
}

/// Read an attribute the way the AX server does: through the NSAccessibility
/// protocol property AppKit maps it to, or the legacy
/// `accessibilityAttributeValue:` for objects without the protocol.
pub fn ax_attr(object: &AnyObject, name: &str) -> Option<Ax> {
    let selector = match name {
        "AXRole" => sel!(accessibilityRole),
        "AXSubrole" => sel!(accessibilitySubrole),
        "AXDescription" => sel!(accessibilityLabel),
        "AXTitle" => sel!(accessibilityTitle),
        "AXValue" => sel!(accessibilityValue),
        "AXHelp" => sel!(accessibilityHelp),
        "AXValueDescription" => sel!(accessibilityValueDescription),
        "AXMinValue" => sel!(accessibilityMinValue),
        "AXMaxValue" => sel!(accessibilityMaxValue),
        "AXSelectedText" => sel!(accessibilitySelectedText),
        "AXChildren" => sel!(accessibilityChildren),
        "AXParent" => sel!(accessibilityParent),
        "AXEnabled" => sel!(isAccessibilityEnabled),
        "AXFocused" => sel!(isAccessibilityFocused),
        "AXSelected" => sel!(isAccessibilitySelected),
        "AXNumberOfCharacters" => sel!(accessibilityNumberOfCharacters),
        "AXInsertionPointLineNumber" => sel!(accessibilityInsertionPointLineNumber),
        "AXSelectedTextRange" => sel!(accessibilitySelectedTextRange),
        _ => sel!(accessibilityAttributeValue:),
    };
    if !responds(object, selector) || selector == sel!(accessibilityAttributeValue:) {
        if !responds(object, sel!(accessibilityAttributeValue:)) {
            return None;
        }
        let key = NSString::from_str(name);
        return unsafe { msg_send![object, accessibilityAttributeValue: &*key] };
    }
    let number = |value: isize| {
        Some(Retained::into_super(Retained::into_super(
            Retained::into_super(NSNumber::new_isize(value)),
        )))
    };
    let flag = |value: bool| {
        Some(Retained::into_super(Retained::into_super(
            Retained::into_super(NSNumber::new_bool(value)),
        )))
    };
    unsafe {
        match name {
            "AXRole" => msg_send![object, accessibilityRole],
            "AXSubrole" => msg_send![object, accessibilitySubrole],
            "AXDescription" => msg_send![object, accessibilityLabel],
            "AXTitle" => msg_send![object, accessibilityTitle],
            "AXValue" => msg_send![object, accessibilityValue],
            "AXHelp" => msg_send![object, accessibilityHelp],
            "AXValueDescription" => msg_send![object, accessibilityValueDescription],
            "AXMinValue" => msg_send![object, accessibilityMinValue],
            "AXMaxValue" => msg_send![object, accessibilityMaxValue],
            "AXSelectedText" => msg_send![object, accessibilitySelectedText],
            "AXChildren" => msg_send![object, accessibilityChildren],
            "AXParent" => msg_send![object, accessibilityParent],
            "AXEnabled" => flag(msg_send![object, isAccessibilityEnabled]),
            "AXFocused" => flag(msg_send![object, isAccessibilityFocused]),
            "AXSelected" => flag(msg_send![object, isAccessibilitySelected]),
            "AXNumberOfCharacters" => number(msg_send![object, accessibilityNumberOfCharacters]),
            "AXInsertionPointLineNumber" => {
                number(msg_send![object, accessibilityInsertionPointLineNumber])
            }
            "AXSelectedTextRange" => {
                let range: NSRange = msg_send![object, accessibilitySelectedTextRange];
                let text = NSString::from_str(&format!("{{{}, {}}}", range.location, range.length));
                Some(Retained::into_super(Retained::into_super(text)))
            }
            _ => None,
        }
    }
}

pub fn describe(value: &AnyObject) -> String {
    let description: Retained<NSString> = unsafe { msg_send![value, description] };
    description.to_string()
}

pub fn ax_string(object: &AnyObject, name: &str) -> Option<String> {
    ax_attr(object, name).map(|value| describe(&value))
}

/// The actions an assistive app is offered: the legacy action names, or the
/// protocol `accessibilityPerform…` methods the element allows.
pub fn ax_actions(object: &AnyObject) -> Vec<String> {
    if responds(object, sel!(accessibilityActionNames)) {
        let names: Option<Retained<NSArray<NSString>>> =
            unsafe { msg_send![object, accessibilityActionNames] };
        return names
            .map(|names| names.iter().map(|name| name.to_string()).collect())
            .unwrap_or_default();
    }
    [
        ("AXPress", sel!(accessibilityPerformPress)),
        ("AXIncrement", sel!(accessibilityPerformIncrement)),
        ("AXDecrement", sel!(accessibilityPerformDecrement)),
        ("AXCancel", sel!(accessibilityPerformCancel)),
        ("AXShowMenu", sel!(accessibilityPerformShowMenu)),
    ]
    .into_iter()
    .filter(|(_, selector)| {
        responds(object, *selector)
            && unsafe { msg_send![object, isAccessibilitySelectorAllowed: *selector] }
    })
    .map(|(name, _)| name.to_string())
    .collect()
}

/// The frame in Cocoa screen coordinates (origin bottom left).
pub fn ax_frame(object: &AnyObject) -> NSRect {
    if !object.class().responds_to(sel!(accessibilityFrame)) {
        return empty_rect();
    }
    unsafe { msg_send![object, accessibilityFrame] }
}

/// AXDescription, which `accessibilityLabel` publishes.
pub fn ax_label(object: &AnyObject) -> Option<String> {
    ax_attr(object, "AXDescription")
        .and_then(|label| label.downcast::<NSString>().ok())
        .map(|label| label.to_string())
}

pub fn ax_selected_range(object: &AnyObject) -> NSRange {
    unsafe { msg_send![object, accessibilitySelectedTextRange] }
}

pub fn press(object: &AnyObject) -> bool {
    unsafe { msg_send![object, accessibilityPerformPress] }
}

pub fn increment(object: &AnyObject) -> bool {
    unsafe { msg_send![object, accessibilityPerformIncrement] }
}

pub fn hit_test(window: &NSWindow, screen_point: NSPoint) -> Option<Ax> {
    unsafe { msg_send![window, accessibilityHitTest: screen_point] }
}

pub fn app_focused() -> Option<Ax> {
    let app = app();
    unsafe { msg_send![&*app, accessibilityFocusedUIElement] }
}

/// The view's `firstRectForCharacterRange:actualRange:`, the rect the input
/// method places its candidate window at.
pub fn first_rect(view: &AnyObject, range: NSRange) -> (NSRect, NSRange) {
    let mut actual = NSRange::new(0, 0);
    let rect: NSRect =
        unsafe { msg_send![view, firstRectForCharacterRange: range, actualRange: &mut actual] };
    (rect, actual)
}

pub fn marked_range(view: &AnyObject) -> NSRange {
    unsafe { msg_send![view, markedRange] }
}

pub fn center(rect: NSRect) -> NSPoint {
    NSPoint::new(
        rect.origin.x + rect.size.width / 2.0,
        rect.origin.y + rect.size.height / 2.0,
    )
}

pub fn contains(outer: NSRect, inner: NSRect, slack: f64) -> bool {
    inner.origin.x >= outer.origin.x - slack
        && inner.origin.y >= outer.origin.y - slack
        && inner.origin.x + inner.size.width <= outer.origin.x + outer.size.width + slack
        && inner.origin.y + inner.size.height <= outer.origin.y + outer.size.height + slack
}

pub fn fmt_rect(rect: NSRect) -> String {
    format!(
        "({:.0},{:.0} {:.0}x{:.0})",
        rect.origin.x, rect.origin.y, rect.size.width, rect.size.height
    )
}

pub fn empty_rect() -> NSRect {
    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0))
}
