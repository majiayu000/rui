//! Frame pacing check
//!
//! `RUI_FRAME_PACING_MODE=animate` (the default) animates a bar for
//! `RUI_FRAME_PACING_SECONDS` (default 2) with frames driven by the display, then
//! goes idle. Run with `RUI_PROFILE=1` to see one telemetry line per frame and a
//! line each time the display link starts or stops. With
//! `RUI_FRAME_PACING_QUIT_AFTER_IDLE_MS` set, the app quits after idling that long.
//!
//! `RUI_FRAME_PACING_MODE=input` queues slow idle work, posts a real click from
//! inside it, and reports how much idle work ran before the click was handled.
//!
//! `RUI_FRAME_PACING_MODE=latency` animates like `animate` while another thread
//! posts mouse moves at uneven intervals, then quits. With `RUI_PROFILE=1` each
//! frame's `rui.present.v1` line reports how long after its first event it
//! reached the screen.

use rui::core::{AppContext, TaskPriority};
use rui::prelude::*;
use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

const IDLE_TASKS: usize = 200;
const IDLE_TASK_COST: Duration = Duration::from_millis(2);
const CLICK_AFTER_IDLE_TASKS: usize = 20;

fn env_u64(name: &str) -> Option<u64> {
    let value = std::env::var(name).ok()?;
    match value.parse() {
        Ok(value) => Some(value),
        Err(err) => panic!("{name}={value:?} is not a number: {err}"),
    }
}

struct Animate {
    started_at: Option<Instant>,
    duration: Duration,
    frames: u32,
    stopped: bool,
    quit_after_idle: Option<Duration>,
    post_mouse_moves: bool,
}

impl View for Animate {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        if self.started_at.is_none() && self.post_mouse_moves {
            post_mouse_moves_until(self.duration);
        }
        let started_at = *self.started_at.get_or_insert_with(Instant::now);
        let elapsed = started_at.elapsed();
        self.frames += 1;
        let progress = if elapsed < self.duration {
            cx.request_animation_frame();
            (elapsed.as_secs_f32() * std::f32::consts::TAU).sin() * 0.5 + 0.5
        } else {
            if !self.stopped {
                self.stopped = true;
                eprintln!(
                    "frame_pacing: animation stopped after {} frames in {:?}",
                    self.frames, elapsed
                );
                if let Some(idle) = self.quit_after_idle {
                    let handle = cx.app_handle();
                    std::thread::spawn(move || {
                        std::thread::sleep(idle);
                        eprintln!("frame_pacing: idle for {idle:?}, quitting");
                        let _ = handle.update(|cx| cx.quit());
                    });
                }
            }
            1.0
        };

        div()
            .w(480.0)
            .h(160.0)
            .bg(Color::hex(0x0f172a))
            .p(24.0)
            .child(
                div()
                    .w(32.0 + 400.0 * progress)
                    .h(32.0)
                    .rounded(8.0)
                    .bg(Color::hex(0x6366f1)),
            )
    }
}

struct InputCheck {
    queued: bool,
    idle_done: Rc<Cell<usize>>,
}

impl View for InputCheck {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        if !self.queued {
            self.queued = true;
            queue_idle_work(cx.app_mut(), Rc::clone(&self.idle_done));
        }
        let idle_done = Rc::clone(&self.idle_done);
        div()
            .w_full()
            .h_full()
            .bg(Color::hex(0x1e293b))
            .on_click(move || {
                eprintln!(
                    "frame_pacing: click handled after {} idle tasks (posted after {})",
                    idle_done.get(),
                    CLICK_AFTER_IDLE_TASKS
                );
            })
    }
}

fn queue_idle_work(cx: &mut AppContext, idle_done: Rc<Cell<usize>>) {
    for index in 1..=IDLE_TASKS {
        let idle_done = Rc::clone(&idle_done);
        cx.defer(TaskPriority::Idle, move |cx| {
            let until = Instant::now() + IDLE_TASK_COST;
            while Instant::now() < until {
                std::hint::spin_loop();
            }
            idle_done.set(index);
            if index == CLICK_AFTER_IDLE_TASKS {
                post_click_at_window_center();
            }
            if index == IDLE_TASKS {
                eprintln!("frame_pacing: all {IDLE_TASKS} idle tasks ran");
                cx.quit();
            }
        });
    }
}

#[cfg(target_os = "macos")]
fn post_click_at_window_center() {
    use objc2::MainThreadMarker;
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::{NSPoint, NSRect};

    const LEFT_MOUSE_DOWN: usize = 1;
    const LEFT_MOUSE_UP: usize = 2;

    let mtm = MainThreadMarker::new().expect("idle work runs on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    unsafe {
        let windows: *mut AnyObject = msg_send![&*app, windows];
        let window: *mut AnyObject = msg_send![windows, firstObject];
        assert!(!window.is_null(), "the app has a window");
        let window_number: isize = msg_send![window, windowNumber];
        let content_view: *mut AnyObject = msg_send![window, contentView];
        let bounds: NSRect = msg_send![content_view, bounds];
        let center = NSPoint::new(bounds.size.width / 2.0, bounds.size.height / 2.0);
        for event_type in [LEFT_MOUSE_DOWN, LEFT_MOUSE_UP] {
            let event: *mut AnyObject = msg_send![
                objc2::class!(NSEvent),
                mouseEventWithType: event_type,
                location: center,
                modifierFlags: 0usize,
                timestamp: 0.0f64,
                windowNumber: window_number,
                context: std::ptr::null_mut::<AnyObject>(),
                eventNumber: 0isize,
                clickCount: 1isize,
                pressure: 1.0f32
            ];
            assert!(!event.is_null(), "AppKit created the click event");
            let _: () = msg_send![&*app, postEvent: event, atStart: false];
        }
    }
}

/// Post mouse moves over the first window from another thread, 9 to 25 ms
/// apart so they land at every phase of the display refresh.
#[cfg(target_os = "macos")]
fn post_mouse_moves_until(duration: Duration) {
    use objc2::MainThreadMarker;
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::{NSPoint, NSRect};

    const MOUSE_MOVED: usize = 5;
    const CAN_JOIN_ALL_SPACES: usize = 1;
    const FLOATING_WINDOW_LEVEL: isize = 3;

    let mtm = MainThreadMarker::new().expect("views render on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    let (window_number, size) = unsafe {
        let windows: *mut AnyObject = msg_send![&*app, windows];
        let window: *mut AnyObject = msg_send![windows, firstObject];
        assert!(!window.is_null(), "the app has a window");
        let window_number: isize = msg_send![window, windowNumber];
        let content_view: *mut AnyObject = msg_send![window, contentView];
        let bounds: NSRect = msg_send![content_view, bounds];
        // Float on every Space so no other window hides the presents measured.
        let _: () = msg_send![window, setCollectionBehavior: CAN_JOIN_ALL_SPACES];
        let _: () = msg_send![window, setLevel: FLOATING_WINDOW_LEVEL];
        (window_number, bounds.size)
    };
    std::thread::spawn(move || {
        let until = Instant::now() + duration;
        let mut seed: u32 = 0x9e37_79b9;
        while Instant::now() < until {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let delay_us = 9_000 + u64::from(seed >> 8) % 16_000;
            std::thread::sleep(Duration::from_micros(delay_us));
            let x = f64::from(seed % 1000) / 1000.0 * size.width;
            unsafe {
                let app: *mut AnyObject =
                    msg_send![objc2::class!(NSApplication), sharedApplication];
                let event: *mut AnyObject = msg_send![
                    objc2::class!(NSEvent),
                    mouseEventWithType: MOUSE_MOVED,
                    location: NSPoint::new(x, size.height / 2.0),
                    modifierFlags: 0usize,
                    timestamp: 0.0f64,
                    windowNumber: window_number,
                    context: std::ptr::null_mut::<AnyObject>(),
                    eventNumber: 0isize,
                    clickCount: 0isize,
                    pressure: 0.0f32
                ];
                assert!(!event.is_null(), "AppKit created the mouse event");
                let _: () = msg_send![app, postEvent: event, atStart: false];
            }
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn post_mouse_moves_until(_duration: Duration) {
    panic!("the latency check posts native macOS events");
}

#[cfg(not(target_os = "macos"))]
fn post_click_at_window_center() {
    panic!("the input check posts native macOS events");
}

fn main() {
    let mode = std::env::var("RUI_FRAME_PACING_MODE").unwrap_or_else(|_| "animate".to_string());
    match mode.as_str() {
        "animate" => App::new().run_view(Animate {
            started_at: None,
            duration: Duration::from_secs(env_u64("RUI_FRAME_PACING_SECONDS").unwrap_or(2)),
            frames: 0,
            stopped: false,
            quit_after_idle: env_u64("RUI_FRAME_PACING_QUIT_AFTER_IDLE_MS")
                .map(Duration::from_millis),
            post_mouse_moves: false,
        }),
        "latency" => App::new().run_view(Animate {
            started_at: None,
            duration: Duration::from_secs(env_u64("RUI_FRAME_PACING_SECONDS").unwrap_or(5)),
            frames: 0,
            stopped: false,
            quit_after_idle: Some(Duration::ZERO),
            post_mouse_moves: true,
        }),
        "input" => App::new().run_view(InputCheck {
            queued: false,
            idle_done: Rc::new(Cell::new(0)),
        }),
        other => panic!("RUI_FRAME_PACING_MODE must be animate, latency or input, not {other:?}"),
    }
}
