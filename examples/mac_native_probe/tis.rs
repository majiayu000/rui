//! Carbon Text Input Sources: select an input method and always put the
//! user's original one back.

use objc2::rc::Retained;
use objc2_foundation::{NSArray, NSString};
use std::ffi::c_void;
use std::sync::Mutex;

type TISInputSourceRef = *mut c_void;
type CFTypeRef = *const c_void;

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    static kTISPropertyInputSourceID: CFTypeRef;
    fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
    fn TISCreateInputSourceList(properties: CFTypeRef, include_all: u8) -> CFTypeRef;
    fn TISGetInputSourceProperty(source: TISInputSourceRef, key: CFTypeRef) -> CFTypeRef;
    fn TISSelectInputSource(source: TISInputSourceRef) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: CFTypeRef);
}

/// The input source that was current before the probe changed it, as its id.
/// Kept globally so the panic hook and the watchdog can restore it too.
static ORIGINAL: Mutex<Option<String>> = Mutex::new(None);

fn source_id(source: TISInputSourceRef) -> Option<String> {
    // SAFETY: the property is a CFString owned by the source (get rule), which
    // is toll-free bridged to NSString.
    unsafe {
        let id = TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
        (!id.is_null()).then(|| (*(id as *const NSString)).to_string())
    }
}

pub fn current_id() -> Option<String> {
    // SAFETY: copy rule; released below.
    unsafe {
        let source = TISCopyCurrentKeyboardInputSource();
        if source.is_null() {
            return None;
        }
        let id = source_id(source);
        CFRelease(source as CFTypeRef);
        id
    }
}

/// Select the enabled input source `id`. Returns the TIS status (0 is noErr),
/// or `None` when no enabled source has that id.
pub fn select(id: &str) -> Option<i32> {
    // SAFETY: the list follows the create rule and is released after use; its
    // elements are TISInputSourceRefs it retains.
    unsafe {
        let list = TISCreateInputSourceList(std::ptr::null(), 0);
        if list.is_null() {
            return None;
        }
        let sources: Retained<NSArray> =
            Retained::retain(list as *mut NSArray).expect("non-null input source list");
        CFRelease(list);
        sources.iter().find_map(|source| {
            let source = Retained::as_ptr(&source) as TISInputSourceRef;
            (source_id(source).as_deref() == Some(id)).then(|| TISSelectInputSource(source))
        })
    }
}

/// Puts the remembered input source back. Safe to call more than once.
pub fn restore() {
    let original = ORIGINAL.lock().map(|mut original| original.take());
    if let Ok(Some(id)) = original {
        let status = select(&id);
        eprintln!(
            "probe: restored input source {id} (status {status:?}, now {:?})",
            current_id()
        );
    }
}

/// Remembers the current input source and restores it when dropped, from the
/// panic hook, or when the probe finishes.
pub struct InputSourceGuard;

impl InputSourceGuard {
    pub fn remember() -> Self {
        let id = current_id();
        eprintln!("probe: original input source {id:?}");
        *ORIGINAL.lock().expect("input source lock") = id;
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            previous(info);
        }));
        Self
    }
}

impl Drop for InputSourceGuard {
    fn drop(&mut self) {
        restore();
    }
}
