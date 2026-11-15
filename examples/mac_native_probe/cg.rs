//! Key events built as CGEvents and wrapped in NSEvents: the same backing a
//! hardware keystroke has. Creating one needs no permission; only posting it
//! to the system would, and the probe never does.

use objc2::encode::{Encoding, RefEncode};
use objc2::rc::Retained;
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
