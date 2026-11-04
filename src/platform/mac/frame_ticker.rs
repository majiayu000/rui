//! Frame ticks for animations: from the display when it can, from a timer when not.
//!
//! Ticks run only while an animation asks for frames, so an idle window receives
//! none. Each tick posts one application event to wake the UI thread; while a
//! tick is still unhandled no further event is posted, so a slow frame skips
//! refreshes instead of queueing them.
//!
//! The display link is created on the first request. It cannot be created while
//! no display is awake (screen asleep or locked, a headless Mac); ticks then come
//! from a timer at the last known refresh interval, and the display link is tried
//! again at most once per [`DISPLAY_LINK_RETRY_INTERVAL`] while animating.

use crate::platform::mac::display_link::DisplayLink;
use crate::platform::mac::events::{
    MAC_DISPLAY_TICK_EVENT_DATA, post_application_event_from_any_thread,
};
use crate::platform::window::PlatformWindowError;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const DISPLAY_LINK_RETRY_INTERVAL: Duration = Duration::from_secs(1);

type PostTick = Box<dyn Fn() -> Result<(), PlatformWindowError> + Send + Sync>;
type CreateDisplayLink =
    Box<dyn FnMut(Arc<TickSink>) -> Result<Box<dyn TickSource>, PlatformWindowError>>;

/// Where ticks go: one wake-up of the UI thread per unhandled tick.
pub(crate) struct TickSink {
    tick_pending: AtomicBool,
    post: PostTick,
}

impl TickSink {
    /// Called from the tick thread on each refresh.
    pub(crate) fn tick(&self) {
        if self.tick_pending.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Err(err) = (self.post)() {
            self.tick_pending.store(false, Ordering::Release);
            log::error!("failed to post a frame tick: {err}");
        }
    }
}

/// A source of display-paced ticks that can be started and stopped.
pub(crate) trait TickSource {
    fn start(&mut self, display: Option<u32>) -> Result<(), PlatformWindowError>;
    fn stop(&mut self) -> Result<(), PlatformWindowError>;
    fn refresh_interval(&self) -> Option<Duration>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TickSourceKind {
    DisplayLink,
    Timer,
}

impl TickSourceKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::DisplayLink => "display_link",
            Self::Timer => "timer",
        }
    }
}

/// A timer thread ticking at a fixed interval until stopped.
struct TimerTicks {
    running: Arc<AtomicBool>,
}

impl TimerTicks {
    fn start(sink: Arc<TickSink>, interval: Duration) -> Result<Self, PlatformWindowError> {
        let running = Arc::new(AtomicBool::new(true));
        let thread_running = Arc::clone(&running);
        std::thread::Builder::new()
            .name(String::from("rui-frame-timer"))
            .spawn(move || {
                let mut next = Instant::now() + interval;
                while thread_running.load(Ordering::Acquire) {
                    std::thread::sleep(next.saturating_duration_since(Instant::now()));
                    if !thread_running.load(Ordering::Acquire) {
                        break;
                    }
                    sink.tick();
                    next += interval;
                    let now = Instant::now();
                    if next < now {
                        next = now + interval;
                    }
                }
            })
            .map_err(|err| {
                PlatformWindowError::backend(
                    "macos",
                    format!("failed to start the frame timer: {err}"),
                )
            })?;
        Ok(Self { running })
    }
}

impl Drop for TimerTicks {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
    }
}

pub(crate) struct FrameTicker {
    sink: Arc<TickSink>,
    create_display_link: CreateDisplayLink,
    display_link: Option<Box<dyn TickSource>>,
    timer: Option<TimerTicks>,
    last_display_link_attempt: Option<Instant>,
    display_link_failure_logged: bool,
    running: Option<TickSourceKind>,
}

impl FrameTicker {
    pub(crate) fn new(window_number: isize) -> Self {
        Self::with_parts(
            Box::new(move || {
                post_application_event_from_any_thread(window_number, MAC_DISPLAY_TICK_EVENT_DATA)
            }),
            Box::new(|sink| {
                DisplayLink::new(sink).map(|link| Box::new(link) as Box<dyn TickSource>)
            }),
        )
    }

    fn with_parts(post: PostTick, create_display_link: CreateDisplayLink) -> Self {
        Self {
            sink: Arc::new(TickSink {
                tick_pending: AtomicBool::new(false),
                post,
            }),
            create_display_link,
            display_link: None,
            timer: None,
            last_display_link_attempt: None,
            display_link_failure_logged: false,
            running: None,
        }
    }

    /// What is ticking now, if anything.
    pub(crate) fn source(&self) -> Option<TickSourceKind> {
        self.running
    }

    /// The UI thread took the posted tick; the next refresh may post again.
    pub(crate) fn tick_handled(&self) {
        self.sink.tick_pending.store(false, Ordering::Release);
    }

    /// The display's refresh interval, once a display link knows it.
    pub(crate) fn refresh_interval(&self) -> Option<Duration> {
        self.display_link
            .as_ref()
            .and_then(|link| link.refresh_interval())
    }

    /// Tick while `animating`, from the display when possible.
    ///
    /// `fallback_interval` paces the timer when the display link is unavailable.
    /// Display link failures fall back to the timer and are logged once per
    /// outage; only failing to start the timer itself is an error.
    pub(crate) fn update(
        &mut self,
        animating: bool,
        display: Option<u32>,
        fallback_interval: Duration,
    ) -> Result<(), PlatformWindowError> {
        if !animating {
            self.stop_all();
            return Ok(());
        }
        if self.running == Some(TickSourceKind::DisplayLink) {
            return Ok(());
        }
        // A link that started before is reused at once; a failed one is retried
        // at most once per retry interval.
        let try_display_link = self.display_link.is_some() || self.retry_display_link_due();
        if try_display_link && self.start_display_link(display) {
            self.timer = None;
            self.running = Some(TickSourceKind::DisplayLink);
            return Ok(());
        }
        if self.timer.is_none() {
            let interval = self.refresh_interval().unwrap_or(fallback_interval);
            self.timer = Some(TimerTicks::start(Arc::clone(&self.sink), interval)?);
            self.running = Some(TickSourceKind::Timer);
        }
        Ok(())
    }

    fn retry_display_link_due(&self) -> bool {
        self.last_display_link_attempt
            .is_none_or(|attempt| attempt.elapsed() >= DISPLAY_LINK_RETRY_INTERVAL)
    }

    fn start_display_link(&mut self, display: Option<u32>) -> bool {
        self.last_display_link_attempt = Some(Instant::now());
        let result = match self.display_link.as_mut() {
            Some(link) => link.start(display),
            None => (self.create_display_link)(Arc::clone(&self.sink)).and_then(|mut link| {
                link.start(display)?;
                self.display_link = Some(link);
                Ok(())
            }),
        };
        match result {
            Ok(()) => {
                if self.display_link_failure_logged {
                    log::warn!("display link available again; frames follow the display");
                    self.display_link_failure_logged = false;
                }
                true
            }
            Err(err) => {
                // A link that exists but cannot start is recreated on the next try.
                self.display_link = None;
                if !self.display_link_failure_logged {
                    log::error!("{err}; pacing animation frames with a timer until it succeeds");
                    self.display_link_failure_logged = true;
                }
                false
            }
        }
    }

    fn stop_all(&mut self) {
        self.timer = None;
        if let Some(link) = self.display_link.as_mut()
            && let Err(err) = link.stop()
        {
            log::error!("{err}; recreating the display link on the next animation frame");
            self.display_link = None;
        }
        self.running = None;
        self.tick_handled();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;

    struct FakeLink {
        started: Arc<AtomicBool>,
    }

    impl TickSource for FakeLink {
        fn start(&mut self, _display: Option<u32>) -> Result<(), PlatformWindowError> {
            self.started.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn stop(&mut self) -> Result<(), PlatformWindowError> {
            self.started.store(false, Ordering::SeqCst);
            Ok(())
        }

        fn refresh_interval(&self) -> Option<Duration> {
            Some(Duration::from_micros(8_333))
        }
    }

    fn asleep_display() -> PlatformWindowError {
        PlatformWindowError::backend("macos", "failed to create display link: CVReturn -6661")
    }

    /// A ticker whose display link fails while `display_asleep` is set, counting
    /// the ticks it posts.
    fn ticker(
        display_asleep: Arc<AtomicBool>,
        link_started: Arc<AtomicBool>,
    ) -> (FrameTicker, Arc<AtomicUsize>) {
        let posted = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&posted);
        let ticker = FrameTicker::with_parts(
            Box::new(move || {
                counted.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
            Box::new(move |_sink| {
                if display_asleep.load(Ordering::SeqCst) {
                    return Err(asleep_display());
                }
                Ok(Box::new(FakeLink {
                    started: Arc::clone(&link_started),
                }) as Box<dyn TickSource>)
            }),
        );
        (ticker, posted)
    }

    fn wait_for_ticks(ticker: &FrameTicker, posted: &AtomicUsize, count: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while posted.load(Ordering::SeqCst) < count {
            assert!(Instant::now() < deadline, "timer ticks did not arrive");
            std::thread::sleep(Duration::from_millis(1));
            ticker.tick_handled();
        }
    }

    #[test]
    fn nothing_ticks_or_is_created_until_an_animation_asks() {
        let created = Arc::new(Mutex::new(0));
        let counted = Arc::clone(&created);
        let mut ticker = FrameTicker::with_parts(
            Box::new(|| Ok(())),
            Box::new(move |_| {
                *counted.lock().unwrap() += 1;
                Err(asleep_display())
            }),
        );

        ticker
            .update(false, None, Duration::from_millis(16))
            .expect("idle update");

        assert_eq!(ticker.source(), None);
        assert_eq!(*created.lock().unwrap(), 0);
    }

    #[test]
    fn frames_keep_arriving_from_a_timer_while_the_display_is_asleep() {
        let asleep = Arc::new(AtomicBool::new(true));
        let link_started = Arc::new(AtomicBool::new(false));
        let (mut ticker, posted) = ticker(Arc::clone(&asleep), Arc::clone(&link_started));

        ticker
            .update(true, None, Duration::from_millis(2))
            .expect("the timer stands in for the display link");
        assert_eq!(ticker.source(), Some(TickSourceKind::Timer));
        wait_for_ticks(&ticker, &posted, 3);

        // Within the retry interval the timer keeps ticking without new attempts.
        ticker
            .update(true, None, Duration::from_millis(2))
            .expect("still animating");
        assert_eq!(ticker.source(), Some(TickSourceKind::Timer));

        ticker
            .update(false, None, Duration::from_millis(2))
            .expect("stop");
        assert_eq!(ticker.source(), None);
        let after_stop = posted.load(Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(20));
        ticker.tick_handled();
        std::thread::sleep(Duration::from_millis(20));
        assert!(posted.load(Ordering::SeqCst) <= after_stop + 1);
    }

    #[test]
    fn the_display_link_takes_over_once_the_display_wakes() {
        let asleep = Arc::new(AtomicBool::new(true));
        let link_started = Arc::new(AtomicBool::new(false));
        let (mut ticker, _posted) = ticker(Arc::clone(&asleep), Arc::clone(&link_started));

        ticker
            .update(true, None, Duration::from_millis(2))
            .expect("timer fallback");
        assert_eq!(ticker.source(), Some(TickSourceKind::Timer));

        asleep.store(false, Ordering::SeqCst);
        ticker.last_display_link_attempt = Some(Instant::now() - DISPLAY_LINK_RETRY_INTERVAL);
        ticker
            .update(true, None, Duration::from_millis(2))
            .expect("retry");

        assert_eq!(ticker.source(), Some(TickSourceKind::DisplayLink));
        assert!(ticker.timer.is_none());
        assert!(link_started.load(Ordering::SeqCst));
        assert_eq!(
            ticker.refresh_interval(),
            Some(Duration::from_micros(8_333))
        );
    }
}
