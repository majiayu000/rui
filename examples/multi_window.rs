//! Two windows sharing one counter.
//!
//! Each window counts its own clicks and shows the shared counter, which either
//! window's "Shared +1" button changes. Closing one window leaves the other
//! running; closing the last one quits.
//!
//! `RUI_MULTI_WINDOW_CHECK=1` drives the windows itself: it clicks each window in
//! turn through real AppKit events, closes the first window, updates the shared
//! counter from a background thread, reports how often each window rendered,
//! and quits.

use rui::core::entity::Entity;
use rui::core::{AppContext, AppHandle, WindowId};
use rui::prelude::*;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

const STEP_DELAY: Duration = Duration::from_millis(500);

#[derive(Default)]
struct Stats {
    renders: BTreeMap<&'static str, u32>,
    clicks: BTreeMap<&'static str, u32>,
}

type SharedStats = Rc<RefCell<Stats>>;

struct Panel {
    name: &'static str,
    shared: Option<Entity<u32>>,
    clicks: u32,
    stats: SharedStats,
    opens_second: bool,
}

impl View for Panel {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let shared = *self
            .shared
            .get_or_insert_with(|| cx.app_mut().create(0_u32));
        if self.opens_second {
            self.opens_second = false;
            open_second_window(cx.app_mut(), shared, Rc::clone(&self.stats));
        }
        {
            let mut stats = self.stats.borrow_mut();
            *stats.renders.entry(self.name).or_default() += 1;
            stats.clicks.insert(self.name, self.clicks);
        }
        let value = *cx.app().get(shared).expect("shared counter");

        div()
            .w_full()
            .h_full()
            .bg(Color::hex(0x1e293b))
            .p(16.0)
            .flex_col()
            .gap(12.0)
            .child(
                button(format!("{}: {} clicks", self.name, self.clicks))
                    .on_click(cx.listener(|this, _| this.clicks += 1)),
            )
            .child(
                button(format!("Shared {value} +1")).on_click(cx.listener(move |_, cx| {
                    *cx.app_mut().get_mut(shared).expect("shared counter") += 1;
                    cx.app_mut().notify(shared.id());
                })),
            )
    }
}

fn open_second_window(cx: &mut AppContext, shared: Entity<u32>, stats: SharedStats) {
    let second = cx.open_view_window(
        WindowOptions::new().title("RUI second").size(360.0, 200.0),
        Panel {
            name: "second",
            shared: Some(shared),
            clicks: 0,
            stats: Rc::clone(&stats),
            opens_second: false,
        },
    );
    if std::env::var_os("RUI_MULTI_WINDOW_CHECK").is_some() {
        start_check(cx.app_handle(), second, shared);
    }
}

/// Each step runs on the UI thread, `STEP_DELAY` after the previous one.
fn start_check(handle: AppHandle, second: WindowId, shared: Entity<u32>) {
    std::thread::spawn(move || {
        let step = |update: Box<dyn FnOnce(&mut AppContext) + Send>| {
            std::thread::sleep(STEP_DELAY);
            if handle.update(update).is_err() {
                panic!("the app closed during the check");
            }
        };
        step(Box::new(|_| report("start")));
        step(Box::new(|_| click_first_button("RUI first")));
        step(Box::new(|_| report("clicked first")));
        step(Box::new(|_| click_first_button("RUI second")));
        step(Box::new(|_| report("clicked second")));
        // As the close button does, so the window's own close request is routed.
        step(Box::new(|_| close_window_titled("RUI first")));
        step(Box::new(|_| report("closed first")));
        step(Box::new(move |cx| {
            *cx.get_mut(shared).expect("shared counter") += 1;
            cx.notify(shared.id());
        }));
        step(Box::new(move |cx| {
            report("shared +1");
            eprintln!(
                "multi_window: running={} open windows={} second open={}",
                cx.is_running(),
                cx.windows().len(),
                cx.windows().iter().any(|window| window.id() == second)
            );
            cx.close_window(second);
        }));
    });
}

thread_local! {
    static STATS: RefCell<Option<SharedStats>> = const { RefCell::new(None) };
}

fn report(stage: &str) {
    STATS.with(|stats| {
        let stats = stats.borrow();
        let stats = stats.as_ref().expect("stats registered").borrow();
        eprintln!(
            "multi_window: {stage}: renders {:?} clicks {:?}",
            stats.renders, stats.clicks
        );
    });
}

#[cfg(target_os = "macos")]
fn window_titled(title: &str) -> *mut objc2::runtime::AnyObject {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::NSString;

    let mtm = objc2::MainThreadMarker::new().expect("updates run on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    unsafe {
        let windows: *mut AnyObject = msg_send![&*app, windows];
        let count: usize = msg_send![windows, count];
        (0..count)
            .map(|index| -> *mut AnyObject { msg_send![windows, objectAtIndex: index] })
            .find(|window| {
                let window_title: Retained<NSString> = msg_send![*window, title];
                window_title.to_string() == title
            })
            .unwrap_or_else(|| panic!("no window titled {title:?}"))
    }
}

#[cfg(target_os = "macos")]
fn close_window_titled(title: &str) {
    let window = window_titled(title);
    unsafe {
        let _: () = objc2::msg_send![window, performClose: std::ptr::null_mut::<objc2::runtime::AnyObject>()];
    }
}

#[cfg(not(target_os = "macos"))]
fn close_window_titled(_title: &str) {
    panic!("the multi-window check closes native macOS windows");
}

/// Post a real left click on the first button of the window titled `title`.
///
/// AppKit maps posted mouse locations through the display's geometry; with the
/// display asleep that mapping is scaled and the click misses, so run the check
/// with the display awake.
#[cfg(target_os = "macos")]
fn click_first_button(title: &str) {
    use objc2::MainThreadMarker;
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::{NSPoint, NSRect};

    const LEFT_MOUSE_DOWN: usize = 1;
    const LEFT_MOUSE_UP: usize = 2;
    // Inside the first button: below the 16 pt padding from the top left.
    const BUTTON_POINT: (f64, f64) = (32.0, 28.0);

    let mtm = MainThreadMarker::new().expect("updates run on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    unsafe {
        let window = window_titled(title);
        let window_number: isize = msg_send![window, windowNumber];
        let content_view: *mut AnyObject = msg_send![window, contentView];
        let bounds: NSRect = msg_send![content_view, bounds];
        let location = NSPoint::new(BUTTON_POINT.0, bounds.size.height - BUTTON_POINT.1);
        for event_type in [LEFT_MOUSE_DOWN, LEFT_MOUSE_UP] {
            let event: *mut AnyObject = msg_send![
                objc2::class!(NSEvent),
                mouseEventWithType: event_type,
                location: location,
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

#[cfg(not(target_os = "macos"))]
fn click_first_button(_title: &str) {
    panic!("the multi-window check posts native macOS events");
}

fn main() {
    let stats = SharedStats::default();
    STATS.with(|registered| *registered.borrow_mut() = Some(Rc::clone(&stats)));
    App::new().run_view_with_options(
        WindowOptions::new().title("RUI first").size(360.0, 200.0),
        Panel {
            name: "first",
            shared: None,
            clicks: 0,
            stats,
            opens_second: true,
        },
    );
}
