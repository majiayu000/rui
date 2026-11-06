//! GPU time of every Metal command buffer this process commits, read the same way on
//! both sides of the comparison. RUI's `benches/compare_frame.rs` includes this file;
//! `run.sh` copies it next to GPUI's bench.
//!
//! GPUI's headless renderer commits one command buffer per frame inside `render_scene`
//! and keeps it, so the buffer cannot be read without changing Zed. Instead,
//! [`install`] swaps the implementation of `-commit` on the device's command buffer
//! class for one that adds a completed handler and then commits as before. The handler
//! records `GPUStartTime` and `GPUEndTime`. Neither side waits for its frames, so the
//! next buffer can start before the previous one ends; [`take`] therefore counts each
//! frame from the later of its start and the previous frame's end: the GPU busy time
//! that frame added.

use std::ffi::{CStr, c_char, c_ulong, c_void};
use std::sync::{Mutex, Once};

type Id = *mut c_void;
type Sel = *const c_void;
type Imp = unsafe extern "C" fn(Id, Sel);

#[link(name = "Metal", kind = "framework")]
unsafe extern "C" {
    fn MTLCreateSystemDefaultDevice() -> Id;
}

unsafe extern "C" {
    fn objc_msgSend();
    fn sel_registerName(name: *const c_char) -> Sel;
    fn object_getClass(object: Id) -> Id;
    fn class_getInstanceMethod(class: Id, selector: Sel) -> Id;
    fn method_setImplementation(method: Id, imp: Imp) -> Imp;
    static _NSConcreteGlobalBlock: c_void;
}

static ORIGINAL_COMMIT: Mutex<Option<Imp>> = Mutex::new(None);
/// `GPUStartTime` and `GPUEndTime`, in seconds, of each command buffer completed since
/// the last [`take`].
static SPANS: Mutex<Vec<(f64, f64)>> = Mutex::new(Vec::new());

fn selector(name: &CStr) -> Sel {
    // SAFETY: `name` is a NUL-terminated selector name.
    unsafe { sel_registerName(name.as_ptr()) }
}

/// `[receiver selector]` returning an object.
unsafe fn send_id(receiver: Id, name: &CStr) -> Id {
    let send: unsafe extern "C" fn(Id, Sel) -> Id =
        unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
    unsafe { send(receiver, selector(name)) }
}

/// `[receiver selector]` returning a double.
unsafe fn send_f64(receiver: Id, name: &CStr) -> f64 {
    let send: unsafe extern "C" fn(Id, Sel) -> f64 =
        unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
    unsafe { send(receiver, selector(name)) }
}

/// Hook `-commit` once for this process.
pub fn install() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| unsafe {
        // Room for every frame of a bench, so recording allocates nothing per frame.
        SPANS.lock().unwrap().reserve(1 << 20);
        let device = MTLCreateSystemDefaultDevice();
        assert!(!device.is_null(), "no Metal device");
        let queue = send_id(device, c"newCommandQueue");
        let probe = send_id(queue, c"commandBuffer");
        assert!(!probe.is_null(), "Metal made no command buffer");
        let class = object_getClass(probe);
        let method = class_getInstanceMethod(class, selector(c"commit"));
        assert!(!method.is_null(), "command buffers have no -commit");
        let original = method_setImplementation(method, commit_with_handler);
        *ORIGINAL_COMMIT.lock().unwrap() = Some(original);
    });
}

unsafe extern "C" fn commit_with_handler(buffer: Id, cmd: Sel) {
    let add: unsafe extern "C" fn(Id, Sel, *const Block) =
        unsafe { std::mem::transmute(objc_msgSend as unsafe extern "C" fn()) };
    unsafe { add(buffer, selector(c"addCompletedHandler:"), &COMPLETED) };
    let original = ORIGINAL_COMMIT
        .lock()
        .unwrap()
        .expect("installed before commit");
    unsafe { original(buffer, cmd) };
}

unsafe extern "C" fn on_completed(_block: *const Block, buffer: Id) {
    let (start, end) = unsafe {
        (
            send_f64(buffer, c"GPUStartTime"),
            send_f64(buffer, c"GPUEndTime"),
        )
    };
    SPANS.lock().unwrap().push((start, end));
}

/// Milliseconds of GPU busy time each command buffer completed since the last call
/// added, in completion order. The record keeps its capacity.
pub fn take() -> Vec<f64> {
    let mut spans: Vec<(f64, f64)> = SPANS.lock().unwrap().drain(..).collect();
    spans.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut previous_end = f64::MIN;
    spans
        .into_iter()
        .map(|(start, end)| {
            let busy = end - start.max(previous_end);
            previous_end = end;
            busy.max(0.0) * 1e3
        })
        .collect()
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
    invoke: unsafe extern "C" fn(*const Block, Id),
    descriptor: *const BlockDescriptor,
}

// SAFETY: the block is immutable and captures nothing.
unsafe impl Sync for Block {}

static DESCRIPTOR: BlockDescriptor = BlockDescriptor {
    reserved: 0,
    size: std::mem::size_of::<Block>() as c_ulong,
};

static COMPLETED: Block = Block {
    isa: &raw const _NSConcreteGlobalBlock,
    flags: 1 << 28, // BLOCK_IS_GLOBAL
    reserved: 0,
    invoke: on_completed,
    descriptor: &DESCRIPTOR,
};
