//! Display-paced frame ticks from CoreVideo.
//!
//! A `CVDisplayLink` calls back on its own thread once per display refresh. Each
//! callback posts one application event to wake the UI thread; while a tick is
//! still unhandled no further event is posted, so a slow frame skips refreshes
//! instead of queueing them. The link only runs while an animation asks for
//! frames, so an idle window receives no ticks.

use crate::platform::mac::events::{
    MAC_DISPLAY_TICK_EVENT_DATA, post_application_event_from_any_thread,
};
use crate::platform::window::PlatformWindowError;
use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

type CVDisplayLinkRef = *mut c_void;
type CVReturn = i32;
type CVOptionFlags = u64;
type CGDirectDisplayID = u32;

const K_CV_RETURN_SUCCESS: CVReturn = 0;
const K_CV_TIME_IS_INDEFINITE: i32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct CVTime {
    time_value: i64,
    time_scale: i32,
    flags: i32,
}

type CVDisplayLinkOutputCallback = extern "C" fn(
    display_link: CVDisplayLinkRef,
    now: *const c_void,
    output_time: *const c_void,
    flags_in: CVOptionFlags,
    flags_out: *mut CVOptionFlags,
    context: *mut c_void,
) -> CVReturn;

#[link(name = "CoreVideo", kind = "framework")]
unsafe extern "C" {
    fn CVDisplayLinkCreateWithActiveCGDisplays(link_out: *mut CVDisplayLinkRef) -> CVReturn;
    fn CVDisplayLinkSetOutputCallback(
        link: CVDisplayLinkRef,
        callback: CVDisplayLinkOutputCallback,
        context: *mut c_void,
    ) -> CVReturn;
    fn CVDisplayLinkSetCurrentCGDisplay(
        link: CVDisplayLinkRef,
        display: CGDirectDisplayID,
    ) -> CVReturn;
    fn CVDisplayLinkGetNominalOutputVideoRefreshPeriod(link: CVDisplayLinkRef) -> CVTime;
    fn CVDisplayLinkStart(link: CVDisplayLinkRef) -> CVReturn;
    fn CVDisplayLinkStop(link: CVDisplayLinkRef) -> CVReturn;
    fn CVDisplayLinkRelease(link: CVDisplayLinkRef);
}

struct TickState {
    window_number: isize,
    tick_pending: AtomicBool,
}

pub(crate) struct DisplayLink {
    link: CVDisplayLinkRef,
    state: *const TickState,
    running: bool,
}

impl DisplayLink {
    pub(crate) fn new(window_number: isize) -> Result<Self, PlatformWindowError> {
        let mut link = std::ptr::null_mut();
        // SAFETY: `link` is a valid out pointer.
        check(
            unsafe { CVDisplayLinkCreateWithActiveCGDisplays(&mut link) },
            "create display link",
        )?;
        let state = Arc::into_raw(Arc::new(TickState {
            window_number,
            tick_pending: AtomicBool::new(false),
        }));
        let display_link = Self {
            link,
            state,
            running: false,
        };
        // SAFETY: `state` stays alive until Drop, which stops the link first.
        check(
            unsafe {
                CVDisplayLinkSetOutputCallback(link, on_display_refresh, state as *mut c_void)
            },
            "set display link callback",
        )?;
        Ok(display_link)
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running
    }

    /// Start ticking with the refresh of `display`, the screen the window is on.
    pub(crate) fn start(&mut self, display: Option<u32>) -> Result<(), PlatformWindowError> {
        if self.running {
            return Ok(());
        }
        if let Some(display) = display {
            // SAFETY: `self.link` is a live display link.
            check(
                unsafe { CVDisplayLinkSetCurrentCGDisplay(self.link, display) },
                "move display link to the window's display",
            )?;
        }
        self.tick_state()
            .tick_pending
            .store(false, Ordering::Release);
        // SAFETY: `self.link` is a live display link with its callback set.
        check(
            unsafe { CVDisplayLinkStart(self.link) },
            "start display link",
        )?;
        self.running = true;
        Ok(())
    }

    pub(crate) fn stop(&mut self) -> Result<(), PlatformWindowError> {
        if !self.running {
            return Ok(());
        }
        // SAFETY: `self.link` is a live display link.
        check(unsafe { CVDisplayLinkStop(self.link) }, "stop display link")?;
        self.running = false;
        Ok(())
    }

    /// The UI thread took the posted tick; the next refresh may post again.
    pub(crate) fn tick_handled(&self) {
        self.tick_state()
            .tick_pending
            .store(false, Ordering::Release);
    }

    /// The refresh interval of the display the link follows, when it is known.
    pub(crate) fn refresh_interval(&self) -> Option<Duration> {
        // SAFETY: `self.link` is a live display link.
        let period = unsafe { CVDisplayLinkGetNominalOutputVideoRefreshPeriod(self.link) };
        if period.flags & K_CV_TIME_IS_INDEFINITE != 0
            || period.time_value <= 0
            || period.time_scale <= 0
        {
            return None;
        }
        Some(Duration::from_secs_f64(
            period.time_value as f64 / period.time_scale as f64,
        ))
    }

    fn tick_state(&self) -> &TickState {
        // SAFETY: `state` came from `Arc::into_raw` and is released only in Drop.
        unsafe { &*self.state }
    }
}

impl Drop for DisplayLink {
    fn drop(&mut self) {
        if let Err(err) = self.stop() {
            log::error!("{err}");
        }
        // SAFETY: the link is stopped, so its callback no longer reads `state`.
        unsafe {
            CVDisplayLinkRelease(self.link);
            drop(Arc::from_raw(self.state));
        }
    }
}

extern "C" fn on_display_refresh(
    _display_link: CVDisplayLinkRef,
    _now: *const c_void,
    _output_time: *const c_void,
    _flags_in: CVOptionFlags,
    _flags_out: *mut CVOptionFlags,
    context: *mut c_void,
) -> CVReturn {
    // SAFETY: `context` is the `TickState` the owning DisplayLink keeps alive.
    let state = unsafe { &*(context as *const TickState) };
    if state.tick_pending.swap(true, Ordering::AcqRel) {
        return K_CV_RETURN_SUCCESS;
    }
    if let Err(err) =
        post_application_event_from_any_thread(state.window_number, MAC_DISPLAY_TICK_EVENT_DATA)
    {
        state.tick_pending.store(false, Ordering::Release);
        log::error!("failed to post a display refresh tick: {err}");
    }
    K_CV_RETURN_SUCCESS
}

fn check(status: CVReturn, action: &str) -> Result<(), PlatformWindowError> {
    if status == K_CV_RETURN_SUCCESS {
        Ok(())
    } else {
        Err(PlatformWindowError::backend(
            "macos",
            format!("failed to {action}: CVReturn {status}"),
        ))
    }
}
