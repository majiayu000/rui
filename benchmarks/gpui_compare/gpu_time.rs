//! GPU time of every Metal command buffer this process commits, read the same way on
//! both sides of the comparison. RUI's `benches/compare_frame.rs` includes this file;
//! `run.sh` copies it next to GPUI's bench.
//!
//! GPUI's headless renderer commits one command buffer per frame inside `render_scene`
//! and keeps it, so the buffer cannot be read without changing Zed. Instead,
//! [`install`] swaps the implementation of `-commit` on the device's command buffer
//! class for one that adds a completed handler and then commits as before. The handler
//! records `GPUStartTime` and `GPUEndTime`. Frames do not wait for the one before, so
//! the next buffer can start before the previous one ends; [`take`] therefore counts each
//! buffer from the later of its start and the latest end of the buffers that started
//! before it: the GPU busy time that buffer added.
//!
//! A harness that commits several buffers a frame calls [`set_frame`] before each
//! frame and reads [`take_frames`]: each buffer is counted to the frame it was
//! committed in, rather than grouped by the order buffers end in.
//!
//! Both sides of the RUI/GPUI comparison tag the turns of one fixed-length
//! window and use [`steady_samples`] to keep its complete post-warmup interval.
//!
//! A frame of about 0.1 ms leaves the GPU idle most of the time, and its time then
//! follows the clock the GPU picked: the same empty frame took 0.06 ms in one case
//! and 0.13 ms in another. With `COMPARE_GPU_REDRAWS=N`, each side draws every
//! frame's scene N more times right after it, and [`steady_redraws`] reads their mean.

use std::ffi::{CStr, c_char, c_ulong, c_void};
use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};
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
/// the last [`take`], with the frame it was committed in.
static SPANS: Mutex<Vec<(usize, f64, f64)>> = Mutex::new(Vec::new());
/// The frame [`set_frame`] last named.
static FRAME: AtomicUsize = AtomicUsize::new(0);
/// Each command buffer committed and not yet completed, with its frame.
static COMMITTED: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());

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
        COMMITTED.lock().unwrap().reserve(1 << 10);
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
    let frame = FRAME.load(Ordering::Relaxed);
    COMMITTED.lock().unwrap().push((buffer as usize, frame));
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
    let frame = {
        let mut committed = COMMITTED.lock().unwrap();
        let at = committed.iter().position(|&(id, _)| id == buffer as usize);
        at.map_or(usize::MAX, |at| committed.swap_remove(at).1)
    };
    SPANS.lock().unwrap().push((frame, start, end));
}

/// Counts the command buffers committed from now on to `frame`.
#[allow(dead_code)]
pub fn set_frame(frame: usize) {
    FRAME.store(frame, Ordering::Relaxed);
}

/// Each completed command buffer since the last call, in the order they ended, with
/// its frame and the milliseconds of GPU busy time it added.
fn busy() -> Vec<(usize, f64)> {
    let mut spans: Vec<(usize, f64, f64)> = SPANS.lock().unwrap().drain(..).collect();
    spans.sort_by(|a, b| a.2.total_cmp(&b.2));
    // Busy time is the union of the spans. Taken in the order buffers started, each
    // adds the part of its span past the latest end so far. In the order they ended,
    // a long buffer that started first would add nothing once a short one inside it
    // had ended: wgpu commits small buffers that run inside a frame's render pass.
    let mut by_start: Vec<usize> = (0..spans.len()).collect();
    by_start.sort_by(|&a, &b| spans[a].1.total_cmp(&spans[b].1));
    let mut busy = vec![0.0; spans.len()];
    let mut covered = f64::MIN;
    for index in by_start {
        let (_, start, end) = spans[index];
        busy[index] = (end - start.max(covered)).max(0.0) * 1e3;
        covered = covered.max(end);
    }
    spans
        .iter()
        .zip(busy)
        .map(|(&(frame, _, _), busy)| (frame, busy))
        .collect()
}

/// The prescribed number of frames to draw before collecting GPU times.
#[allow(dead_code)]
pub const WARMUP_FRAMES: usize = 10;

/// Marks buffers outside the one comparison window's sample sequence.
#[allow(dead_code)]
pub const OUTSIDE_SAMPLE: usize = usize::MAX;

/// Number of post-warmup frames to collect on each side of the comparison.
#[allow(dead_code)]
pub fn sample_count() -> usize {
    std::env::var("COMPARE_SAMPLES")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|&samples| (1..REDRAW - WARMUP_FRAMES).contains(&samples))
        .unwrap_or(60)
}

/// Tag currently assigned to buffers, if a comparison turn is in progress.
#[allow(dead_code)]
pub fn sample_frame() -> Option<usize> {
    let frame = FRAME.load(Ordering::Relaxed);
    (frame != OUTSIDE_SAMPLE).then_some(frame)
}

/// GPU time for each frame in `warmup..warmup + samples` of one window. A
/// missing or duplicate frame is an error. Setup, CPU-only frames and redraws
/// ([`redraw_tag`]) are left out.
#[allow(dead_code)]
pub fn steady_samples(
    tagged: &[(usize, f64)],
    warmup: usize,
    samples: usize,
) -> Result<Vec<f64>, String> {
    let frames = steady(tagged, warmup, samples)?;
    Ok(frames.into_iter().map(|(_, busy)| busy).collect())
}

/// Bit 31 of a frame number marks the frame's redraws.
const REDRAW: usize = 1 << 31;

/// The tag of the redraws of the frame tagged `tag`.
#[allow(dead_code)]
pub fn redraw_tag(tag: usize) -> usize {
    tag | REDRAW
}

/// The number of times `COMPARE_GPU_REDRAWS` asks for each frame's scene to be
/// drawn again right after it, back to back; 0 when unset.
#[allow(dead_code)]
pub fn redraws() -> usize {
    std::env::var("COMPARE_GPU_REDRAWS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

/// For each frame [`steady_samples`] chooses, the mean GPU time of its redraws
/// ([`redraw_tag`]). Drawn back to back, they keep the GPU busy, so its clock is up
/// and their time is the frame's cost rather than the clock the GPU happened to
/// run at; the mean counts any overlap between them once. A sampled frame must
/// contain every requested redraw.
#[allow(dead_code)]
pub fn steady_redraws(
    tagged: &[(usize, f64)],
    warmup: usize,
    samples: usize,
    redraw_count: usize,
) -> Result<Vec<f64>, String> {
    let frames = steady(tagged, warmup, samples)?;
    let mut redraws: Vec<(usize, f64)> = tagged
        .iter()
        .filter(|&&(tag, _)| tag != OUTSIDE_SAMPLE && tag & REDRAW != 0)
        .map(|&(tag, busy)| (tag & !REDRAW, busy))
        .collect();
    redraws.sort_by_key(|&(tag, _)| tag);
    let mut groups = redraws.chunk_by(|a, b| a.0 == b.0);
    frames
        .iter()
        .map(|&(tag, _)| {
            let group = groups
                .find(|group| group[0].0 >= tag)
                .filter(|group| group[0].0 == tag)
                .unwrap_or_default();
            if group.len() != redraw_count {
                return Err(format!(
                    "frame {tag} has {} redraws; expected {redraw_count}",
                    group.len()
                ));
            }
            let total: f64 = group.iter().map(|&(_, busy)| busy).sum();
            Ok(total / redraw_count as f64)
        })
        .collect()
}

/// The frames [`steady_samples`] chooses, with their tags.
fn steady(
    tagged: &[(usize, f64)],
    warmup: usize,
    samples: usize,
) -> Result<Vec<(usize, f64)>, String> {
    let wanted = warmup..warmup + samples;
    let mut frames: Vec<(usize, f64)> = tagged
        .iter()
        .copied()
        .filter(|&(frame, _)| wanted.contains(&frame))
        .collect();
    frames.sort_by_key(|&(frame, _)| frame);
    for (expected, &(frame, _)) in (warmup..).zip(&frames) {
        if frame < expected {
            return Err(format!("frame {frame} has more than one command buffer"));
        }
        if frame > expected {
            return Err(format!("frame {expected} has no command buffer"));
        }
    }
    if frames.len() != samples {
        return Err(format!(
            "frame {} has no command buffer",
            warmup + frames.len()
        ));
    }
    Ok(frames)
}

/// Each command buffer completed since the last call, with the tag [`set_frame`] gave
/// its frame and the milliseconds of GPU busy time it added.
#[allow(dead_code)]
pub fn take_tagged() -> Vec<(usize, f64)> {
    busy()
}

/// Milliseconds of GPU busy time each command buffer completed since the last call
/// added, in completion order. The record keeps its capacity.
#[allow(dead_code)]
pub fn take() -> Vec<f64> {
    busy().into_iter().map(|(_, busy)| busy).collect()
}

/// Milliseconds of GPU busy time the command buffers of each frame in `frames` added,
/// from those completed since the last call; `None` when one of the frames has none.
/// Buffers of other frames still take their share of the busy time.
#[allow(dead_code)]
pub fn take_frames(frames: Range<usize>) -> Option<Vec<f64>> {
    let mut per_frame = vec![(0.0, false); frames.len()];
    for (frame, busy) in busy() {
        if frames.contains(&frame) {
            let slot = &mut per_frame[frame - frames.start];
            *slot = (slot.0 + busy, true);
        }
    }
    per_frame
        .into_iter()
        .map(|(busy, seen)| seen.then_some(busy))
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
