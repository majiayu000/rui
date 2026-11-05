//! Event-to-photon timing for `RUI_PROFILE`.
//!
//! Each profiled drawable gets a Metal presented handler. When the drawable
//! reaches the screen the handler prints one `rui.present.v1` line with the
//! time from the frame's first observed event, and from the start of its render, to
//! the moment it was shown. All times are on the `CACurrentMediaTime` clock,
//! which `presentedTime` also uses.

use objc2::encode::{Encoding, RefEncode};
use objc2::msg_send;
use objc2::runtime::AnyObject;
use std::collections::HashMap;
use std::ffi::{c_ulong, c_void};
use std::sync::{Mutex, OnceLock};

#[link(name = "QuartzCore", kind = "framework")]
unsafe extern "C" {
    fn CACurrentMediaTime() -> f64;
}

unsafe extern "C" {
    static _NSConcreteGlobalBlock: c_void;
}

/// Seconds on the clock `MTLDrawable.presentedTime` reports.
pub(crate) fn media_time() -> f64 {
    // SAFETY: CACurrentMediaTime has no preconditions.
    unsafe { CACurrentMediaTime() }
}

struct PendingPresent {
    frame_index: u64,
    event_observed_at: Option<f64>,
    render_started_at: f64,
}

/// Drawable pointer and drawable ID: the pointer tells layers apart, the ID
/// tells reuses of one drawable apart.
type DrawableKey = (usize, u64);

fn pending() -> &'static Mutex<HashMap<DrawableKey, PendingPresent>> {
    static PENDING: OnceLock<Mutex<HashMap<DrawableKey, PendingPresent>>> = OnceLock::new();
    PENDING.get_or_init(Default::default)
}

/// Report when `drawable` reaches the screen. Call right before rendering into it.
pub(crate) fn track(
    drawable: &metal::MetalDrawableRef,
    frame_index: u64,
    event_observed_at: Option<f64>,
) {
    let drawable = drawable as *const metal::MetalDrawableRef as *mut AnyObject;
    let key = drawable_key(drawable);
    let present = PendingPresent {
        frame_index,
        event_observed_at,
        render_started_at: media_time(),
    };
    match pending().lock() {
        Ok(mut pending) => {
            pending.insert(key, present);
        }
        Err(err) => panic!("present latency table poisoned: {err}"),
    }
    // SAFETY: the block is a global block with the presented-handler
    // signature; Metal copies it, which for a global block is a no-op.
    unsafe {
        let _: () = msg_send![drawable, addPresentedHandler: &PRESENTED_HANDLER as *const Block];
    }
}

fn drawable_key(drawable: *mut AnyObject) -> DrawableKey {
    // SAFETY: `drawable` is a live MTLDrawable.
    let id: usize = unsafe { msg_send![drawable, drawableID] };
    (drawable as usize, id as u64)
}

unsafe extern "C" fn on_presented(_block: *const Block, drawable: *mut AnyObject) {
    let key = drawable_key(drawable);
    // SAFETY: Metal passes the presented drawable.
    let presented_at: f64 = unsafe { msg_send![drawable, presentedTime] };
    let present = match pending().lock() {
        Ok(mut pending) => pending.remove(&key),
        Err(err) => panic!("present latency table poisoned: {err}"),
    };
    let Some(present) = present else {
        return;
    };
    eprintln!("{}", present_line(&present, presented_at));
}

fn present_line(present: &PendingPresent, presented_at: f64) -> String {
    // presentedTime is 0 when the drawable was dropped instead of shown.
    if presented_at <= 0.0 {
        return format!(
            "{{\"schema\":\"rui.present.v1\",\"frame_index\":{},\"presented\":false}}",
            present.frame_index
        );
    }
    let nanos = |since: f64| ((presented_at - since).max(0.0) * 1e9).round() as u128;
    let event_to_present = match present.event_observed_at {
        Some(observed_at) => nanos(observed_at).to_string(),
        None => "null".to_string(),
    };
    format!(
        "{{\"schema\":\"rui.present.v1\",\"frame_index\":{},\"presented\":true,\"event_to_present_ns\":{},\"render_to_present_ns\":{}}}",
        present.frame_index,
        event_to_present,
        nanos(present.render_started_at)
    )
}

#[repr(C)]
struct BlockDescriptor {
    reserved: c_ulong,
    size: c_ulong,
}

/// An Objective-C block literal with no captures (Clang's block ABI).
#[repr(C)]
struct Block {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: unsafe extern "C" fn(*const Block, *mut AnyObject),
    descriptor: *const BlockDescriptor,
}

// SAFETY: the block is immutable and captures nothing.
unsafe impl Sync for Block {}

// SAFETY: a pointer to a block literal is an Objective-C block.
unsafe impl RefEncode for Block {
    const ENCODING_REF: Encoding = Encoding::Block;
}

const BLOCK_IS_GLOBAL: i32 = 1 << 28;

static PRESENTED_HANDLER_DESCRIPTOR: BlockDescriptor = BlockDescriptor {
    reserved: 0,
    size: std::mem::size_of::<Block>() as c_ulong,
};

static PRESENTED_HANDLER: Block = Block {
    isa: &raw const _NSConcreteGlobalBlock,
    flags: BLOCK_IS_GLOBAL,
    reserved: 0,
    invoke: on_presented,
    descriptor: &PRESENTED_HANDLER_DESCRIPTOR,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn present_line_reports_both_spans() {
        let present = PendingPresent {
            frame_index: 7,
            event_observed_at: Some(1.0),
            render_started_at: 1.004,
        };
        assert_eq!(
            present_line(&present, 1.010),
            "{\"schema\":\"rui.present.v1\",\"frame_index\":7,\"presented\":true,\"event_to_present_ns\":10000000,\"render_to_present_ns\":6000000}"
        );
    }

    #[test]
    fn present_line_marks_dropped_drawables() {
        let present = PendingPresent {
            frame_index: 3,
            event_observed_at: None,
            render_started_at: 1.0,
        };
        assert_eq!(
            present_line(&present, 0.0),
            "{\"schema\":\"rui.present.v1\",\"frame_index\":3,\"presented\":false}"
        );
    }
}
