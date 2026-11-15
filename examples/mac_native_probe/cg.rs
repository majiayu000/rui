//! Key events built as CGEvents: wrapped in NSEvents and sent to the window
//! (no permission needed), or with `RUI_PROBE_EVENTS=hid` posted at the HID
//! event tap like a physical keystroke, which needs Accessibility permission
//! for the probe app.

use objc2::encode::{Encoding, RefEncode};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_app_kit::NSEvent;
use std::ffi::c_void;

#[repr(C)]
pub struct CGEvent {
    _private: [u8; 0],
}

// SAFETY: CGEventRef is `struct __CGEvent *`.
unsafe impl RefEncode for CGEvent {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__CGEvent", &[]));
}

/// `kCGMouseEventWindowUnderMousePointer` and the variant that can handle the
/// event: NSEvent reads its window number from them.
const WINDOW_FIELDS: [u32; 2] = [91, 92];
/// `kCGEventSourceStateHIDSystemState`.
const HID_SYSTEM_STATE: i32 = 1;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceCreate(state: i32) -> *const c_void;
    fn CGEventCreateKeyboardEvent(source: *const c_void, key: u16, down: bool) -> *mut CGEvent;
    fn CGEventSetIntegerValueField(event: *mut CGEvent, field: u32, value: i64);
    fn CGEventSetFlags(event: *mut CGEvent, flags: u64);
    fn CGEventKeyboardSetUnicodeString(event: *mut CGEvent, length: usize, text: *const u16);
    fn CGEventPost(tap: u32, event: *mut CGEvent);
    fn CGEventPostToPid(pid: i32, event: *mut CGEvent);
    fn CGPreflightPostEventAccess() -> bool;
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: *const AnyObject) -> bool;
}

/// `kCGHIDEventTap`: where the window server injects hardware events.
const HID_EVENT_TAP: u32 = 0;

/// Whether this process may post events at the HID tap.
pub fn trusted() -> bool {
    // SAFETY: a plain query with no arguments.
    unsafe { AXIsProcessTrusted() && CGPreflightPostEventAccess() }
}

/// Where HID keystrokes go: `RUI_PROBE_TAP=session` posts at the session
/// tap, `pid` straight to this process, anything else at the HID tap.
fn post(event: *mut CGEvent) {
    // SAFETY: `event` is a live CGEvent owned by the caller.
    unsafe {
        match std::env::var("RUI_PROBE_TAP").as_deref() {
            Ok("session") => CGEventPost(1, event),
            Ok("pid") => CGEventPostToPid(std::process::id() as i32, event),
            _ => CGEventPost(HID_EVENT_TAP, event),
        }
    }
}

/// Ask macOS to show its own Accessibility prompt for this app, which links
/// to the switch in System Settings. Only the user can turn it on.
pub fn request_trust() -> bool {
    // SAFETY: an NSDictionary is toll-free bridged to the CFDictionary the
    // call expects; `kAXTrustedCheckOptionPrompt` is this string.
    unsafe {
        let key: *mut AnyObject = msg_send![class!(NSString), stringWithUTF8String: c"AXTrustedCheckOptionPrompt".as_ptr()];
        let yes: *mut AnyObject = msg_send![class!(NSNumber), numberWithBool: true];
        let options: *mut AnyObject =
            msg_send![class!(NSDictionary), dictionaryWithObject: yes, forKey: key];
        AXIsProcessTrustedWithOptions(options)
    }
}

/// Post a key down or up at the HID event tap, as a physical keyboard would:
/// the window server routes it to the key window through the active input
/// method. Returns false if CoreGraphics refused to build the event.
pub fn post_hid(code: u16, down: bool, flags: u64) -> bool {
    // SAFETY: both objects follow the create rule and are released here.
    unsafe {
        let source = CGEventSourceCreate(HID_SYSTEM_STATE);
        let event = CGEventCreateKeyboardEvent(source, code, down);
        if !source.is_null() {
            CFRelease(source);
        }
        if event.is_null() {
            return false;
        }
        CGEventSetFlags(event, flags);
        post(event);
        CFRelease(event as *const c_void);
        true
    }
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: *const c_void);
}

/// A key down or up (a flags-changed event for modifier keys) addressed to
/// `window_number`.
pub fn key_event(
    window_number: isize,
    code: u16,
    down: bool,
    flags: u64,
    text: &str,
) -> Option<Retained<NSEvent>> {
    // SAFETY: the CGEvent follows the create rule and is released after the
    // NSEvent, which retains it, wraps it.
    unsafe {
        // A HID-system-state source, as the window server stamps keystrokes.
        let source = CGEventSourceCreate(HID_SYSTEM_STATE);
        let event = CGEventCreateKeyboardEvent(source, code, down);
        if !source.is_null() {
            CFRelease(source);
        }
        if event.is_null() {
            return None;
        }
        CGEventSetFlags(event, flags);
        if !text.is_empty() {
            let units: Vec<u16> = text.encode_utf16().collect();
            CGEventKeyboardSetUnicodeString(event, units.len(), units.as_ptr());
        }
        for field in WINDOW_FIELDS {
            CGEventSetIntegerValueField(event, field, window_number as i64);
        }
        let wrapped: Option<Retained<NSEvent>> =
            msg_send![class!(NSEvent), eventWithCGEvent: event];
        CFRelease(event as *const c_void);
        wrapped
    }
}
