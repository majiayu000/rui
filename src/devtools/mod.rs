//! Developer tools: a profiling overlay drawn into each window's scene.
//!
//! Everything here is off unless `RUI_DEVTOOLS=1` is set or the app calls
//! [`AppContext::toggle_profiler`](crate::AppContext::toggle_profiler). Off, a
//! frame pays one branch per stage and nothing is allocated. See
//! `docs/devtools.md`.

mod profiler;

pub use profiler::{FrameSample, Profiler, RenderSample};

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

pub const RUI_DEVTOOLS_ENV: &str = "RUI_DEVTOOLS";

/// Which tools are on, app-wide.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DevtoolsFlags {
    pub(crate) profiler: bool,
}

impl DevtoolsFlags {
    pub(crate) fn from_env() -> Self {
        let on = std::env::var(RUI_DEVTOOLS_ENV)
            .is_ok_and(|value| !matches!(value.as_str(), "" | "0" | "false" | "off"));
        Self { profiler: on }
    }
}

/// Profilers alive in any window; allocations are counted while there are any.
static PROFILERS: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

/// A global allocator that counts allocations while the profiler is on, for
/// its allocations-per-frame line. Rust lets only the binary choose the
/// allocator, so an app that wants the count installs it:
///
/// ```no_run
/// #[global_allocator]
/// static ALLOCATOR: rui::devtools::CountingAllocator = rui::devtools::CountingAllocator;
/// ```
///
/// With the profiler off each allocation costs one relaxed load more than
/// [`System`].
pub struct CountingAllocator;

// SAFETY: forwards every call to `System` unchanged; only counts allocations.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count_allocation();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count_allocation();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

fn count_allocation() {
    if PROFILERS.load(Ordering::Relaxed) > 0 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    }
}

fn profiler_started() {
    PROFILERS.fetch_add(1, Ordering::Relaxed);
}

fn profiler_stopped() {
    PROFILERS.fetch_sub(1, Ordering::Relaxed);
}

/// Allocations counted so far. Stays 0 when [`CountingAllocator`] is not the
/// global allocator, since the profiler's own drawing always allocates.
fn allocations() -> u64 {
    ALLOCATIONS.load(Ordering::Relaxed)
}
