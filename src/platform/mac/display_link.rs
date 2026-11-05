//! Display-paced frame ticks from CoreVideo.
//!
//! A `CVDisplayLink` calls back on its own thread once per display refresh and
//! hands each refresh to a [`TickSink`].

use crate::platform::mac::frame_ticker::{TickSink, TickSource};
use crate::platform::window::PlatformWindowError;
use std::ffi::c_void;
use std::sync::Arc;
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

pub(crate) struct DisplayLink {
    link: CVDisplayLinkRef,
    state: *const TickSink,
    running: bool,
}

impl DisplayLink {
    /// Fails while no display is awake, such as with the screen asleep or locked.
    pub(crate) fn new(sink: Arc<TickSink>) -> Result<Self, PlatformWindowError> {
        let mut link = std::ptr::null_mut();
        // SAFETY: `link` is a valid out pointer.
        check(
            unsafe { CVDisplayLinkCreateWithActiveCGDisplays(&mut link) },
            "create display link",
        )?;
        let state = Arc::into_raw(sink);
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
}

impl TickSource for DisplayLink {
    fn start(&mut self, display: Option<u32>) -> Result<(), PlatformWindowError> {
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
        // SAFETY: `self.link` is a live display link with its callback set.
        check(
            unsafe { CVDisplayLinkStart(self.link) },
            "start display link",
        )?;
        self.running = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PlatformWindowError> {
        if !self.running {
            return Ok(());
        }
        // SAFETY: `self.link` is a live display link.
        check(unsafe { CVDisplayLinkStop(self.link) }, "stop display link")?;
        self.running = false;
        Ok(())
    }

    fn refresh_interval(&self) -> Option<Duration> {
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
}

impl Drop for DisplayLink {
    fn drop(&mut self) {
        if let Err(err) = TickSource::stop(self) {
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
    // SAFETY: `context` is the `TickSink` the owning DisplayLink keeps alive.
    let sink = unsafe { &*(context as *const TickSink) };
    sink.tick();
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
