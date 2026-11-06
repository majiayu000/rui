//! Developer tools drawn into each window's scene: a profiling overlay, a
//! damage and replay visualizer and an element inspector.
//!
//! Everything here is off unless `RUI_DEVTOOLS` names a tool or the app turns
//! one on, such as with
//! [`AppContext::toggle_profiler`](crate::AppContext::toggle_profiler). Off, a
//! frame pays one branch per stage and nothing is allocated. See
//! `docs/devtools.md`.

mod damage;
mod inspector;
mod profiler;
mod window;

pub(crate) use inspector::Inspection;
pub use inspector::{InspectedElement, Inspector, PaintState};
pub use profiler::{FrameSample, Profiler, RenderSample};
pub(crate) use window::WindowDevtools;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

pub const RUI_DEVTOOLS_ENV: &str = "RUI_DEVTOOLS";

/// Which tools are on, app-wide.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DevtoolsFlags {
    pub(crate) profiler: bool,
    pub(crate) damage: bool,
    pub(crate) inspector: bool,
}

impl DevtoolsFlags {
    pub(crate) fn from_env() -> Self {
        match std::env::var(RUI_DEVTOOLS_ENV) {
            Ok(value) => match Self::parse(&value) {
                Ok(flags) => flags,
                Err(tool) => panic!(
                    "{RUI_DEVTOOLS_ENV}: unknown tool `{tool}`; expected a comma-separated \
                     list of profiler, damage, inspector, all, or 1 for the profiler"
                ),
            },
            Err(_) => Self::default(),
        }
    }

    /// `1` or `profiler`, `damage`, `inspector`, `all`, comma-separated; empty, `0`,
    /// `false` and `off` turn everything off.
    fn parse(value: &str) -> Result<Self, &str> {
        let mut flags = Self::default();
        if matches!(value, "" | "0" | "false" | "off") {
            return Ok(flags);
        }
        for tool in value.split(',').map(str::trim) {
            match tool {
                "1" | "profiler" => flags.profiler = true,
                "damage" => flags.damage = true,
                "inspector" => flags.inspector = true,
                "all" => {
                    (flags.profiler, flags.damage, flags.inspector) = (true, true, true);
                }
                tool => return Err(tool),
            }
        }
        Ok(flags)
    }

    pub(crate) fn any(self) -> bool {
        self.profiler || self.damage || self.inspector
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

#[cfg(test)]
mod tests {
    use super::DevtoolsFlags;

    #[test]
    fn rui_devtools_names_the_tools_to_turn_on() {
        let flags = |profiler, damage, inspector| {
            Ok(DevtoolsFlags {
                profiler,
                damage,
                inspector,
            })
        };
        assert_eq!(DevtoolsFlags::parse(""), flags(false, false, false));
        assert_eq!(DevtoolsFlags::parse("0"), flags(false, false, false));
        assert_eq!(DevtoolsFlags::parse("1"), flags(true, false, false));
        assert_eq!(DevtoolsFlags::parse("damage"), flags(false, true, false));
        assert_eq!(DevtoolsFlags::parse("inspector"), flags(false, false, true));
        assert_eq!(
            DevtoolsFlags::parse("profiler, damage"),
            flags(true, true, false)
        );
        assert_eq!(DevtoolsFlags::parse("all"), flags(true, true, true));
        assert_eq!(DevtoolsFlags::parse("profiler,inspect"), Err("inspect"));
    }
}
