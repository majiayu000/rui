//! Wakes the app loop when the earliest frame asked for with
//! [`AppContext::request_frame_at`] is due.
//!
//! The loop keeps waiting for events with no timeout, as it does when idle: a
//! finite `nextEventMatchingMask:untilDate:` returns early whenever the run
//! loop handles anything else (window server traffic for events the loop does
//! not take, for one), and each early return ran a whole loop iteration. A
//! sleeping thread posts the loop's wake event at the deadline instead.

use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub(super) struct DeadlineWake {
    /// The deadline the running sleeper wakes at, and its cancel flag.
    armed: Option<(Instant, Arc<AtomicBool>)>,
}

impl DeadlineWake {
    /// Have a wake posted at `deadline`, replacing a sleeper armed for
    /// another time; `None` cancels it.
    pub(super) fn arm(&mut self, deadline: Option<Instant>) {
        if self.armed.as_ref().map(|(at, _)| *at) == deadline {
            return;
        }
        if let Some((_, cancelled)) = self.armed.take() {
            cancelled.store(true, Ordering::Relaxed);
        }
        let Some(deadline) = deadline else {
            return;
        };
        let cancelled = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancelled);
        let spawned = std::thread::Builder::new()
            .name(String::from("rui-deadline-wake"))
            .spawn(move || {
                std::thread::sleep(deadline.saturating_duration_since(Instant::now()));
                if flag.load(Ordering::Relaxed) {
                    return;
                }
                if let Err(err) = post_application_event_from_any_thread(
                    APP_EVENT_WINDOW_NUMBER,
                    MAC_REDRAW_EVENT_DATA,
                ) {
                    log::error!("failed to wake the event loop at a frame deadline: {err}");
                }
            });
        match spawned {
            Ok(_) => self.armed = Some((deadline, cancelled)),
            Err(err) => log::error!("failed to start the frame deadline wake: {err}"),
        }
    }
}
