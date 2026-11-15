//! In-process macOS probe: real Pinyin input through NSTextInputContext and
//! the NSAccessibility tree VoiceOver reads, checked from inside a running
//! rui window.
//!
//! Run in the foreground on a Mac with the Simplified Chinese Pinyin input
//! method enabled: `cargo run --example mac_native_probe`. The window must be
//! able to become key. The probe selects Pinyin with Carbon TIS and restores
//! the original input source when it finishes, fails, panics or times out.
//! It prints every NSTextInputClient call rui receives and PASS/FAIL per
//! scenario, then exits 0 when everything passed.

#[cfg(target_os = "macos")]
mod ax;
#[cfg(target_os = "macos")]
mod cg;
#[cfg(target_os = "macos")]
mod ime;
#[cfg(target_os = "macos")]
mod native;
#[cfg(target_os = "macos")]
mod tis;
#[cfg(target_os = "macos")]
mod view;

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("mac_native_probe drives AppKit and only runs on macOS");
}

#[cfg(target_os = "macos")]
fn main() {
    probe::run();
}

#[cfg(target_os = "macos")]
mod probe {
    use super::{ax, ime, native, tis, view};
    use objc2::rc::Retained;
    use objc2_app_kit::NSWindow;
    use rui::prelude::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::ffi::c_void;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    /// The rui target its NSTextInputClient methods log to, at trace level.
    pub const IME_TARGET: &str = "rui::ime";
    const TIMEOUT: Duration = Duration::from_secs(240);

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static START: Mutex<Option<Instant>> = Mutex::new(None);

    struct ProbeLogger;

    impl log::Log for ProbeLogger {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            metadata.target() == IME_TARGET || metadata.level() <= log::Level::Warn
        }

        fn log(&self, record: &log::Record) {
            if !self.enabled(record.metadata()) {
                return;
            }
            if record.target() == IME_TARGET {
                note(format!("{}", record.args()));
            } else {
                eprintln!("[{} {}] {}", record.level(), record.target(), record.args());
            }
        }

        fn flush(&self) {}
    }

    /// Append a line to the call log, stamped with milliseconds since start.
    pub fn note(line: String) {
        let elapsed = START
            .lock()
            .ok()
            .and_then(|start| *start)
            .map_or(0, |start| start.elapsed().as_millis());
        let line = format!("{elapsed:>6}ms {line}");
        println!("  ime| {line}");
        LOG.lock().expect("log lock").push(line);
    }

    pub fn log_len() -> usize {
        LOG.lock().expect("log lock").len()
    }

    pub fn log_since(mark: usize) -> Vec<String> {
        LOG.lock().expect("log lock")[mark..].to_vec()
    }

    pub type Step = Box<dyn FnOnce(&mut Probe) -> u64>;

    pub fn step(work: impl FnOnce(&mut Probe) -> u64 + 'static) -> Step {
        Box::new(work)
    }

    pub struct Outcome {
        pub name: String,
        pub pass: bool,
        pub detail: String,
    }

    pub struct Probe {
        pub window: Retained<NSWindow>,
        pub steps: VecDeque<Step>,
        pub outcomes: Vec<Outcome>,
        /// Log index where the current scenario started.
        pub mark: usize,
        /// Keys go straight to `NSTextInputContext handleEvent:`.
        pub direct_keys: bool,
    }

    impl Probe {
        pub fn record(&mut self, name: impl Into<String>, pass: bool, detail: impl Into<String>) {
            let outcome = Outcome {
                name: name.into(),
                pass,
                detail: detail.into(),
            };
            println!(
                "{} {}: {}",
                if outcome.pass { "PASS" } else { "FAIL" },
                outcome.name,
                outcome.detail
            );
            self.outcomes.push(outcome);
        }

        /// Run `steps` next, before what is already queued.
        pub fn push_front(&mut self, steps: Vec<Step>) {
            for step in steps.into_iter().rev() {
                self.steps.push_front(step);
            }
        }
    }

    thread_local! {
        static PROBE: RefCell<Option<Probe>> = const { RefCell::new(None) };
    }

    extern "C" fn tick(_: *mut c_void) {
        let result = PROBE.with(|probe| {
            let mut probe = probe.borrow_mut();
            let probe = probe.as_mut()?;
            let step = probe.steps.pop_front()?;
            Some(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || step(probe),
            )))
        });
        match result {
            Some(Ok(delay)) => native::after(delay, tick),
            Some(Err(_)) => finish("a probe step panicked"),
            None => finish("done"),
        }
    }

    extern "C" fn start(_: *mut c_void) {
        let Some(window) = native::window() else {
            native::after(200, start);
            return;
        };
        let mut steps: VecDeque<Step> = VecDeque::new();
        steps.extend(ime::steps());
        steps.extend(ax::steps());
        PROBE.with(|probe| {
            *probe.borrow_mut() = Some(Probe {
                window,
                steps,
                outcomes: Vec::new(),
                mark: 0,
                direct_keys: false,
            });
        });
        native::after(0, tick);
    }

    fn finish(reason: &str) {
        tis::restore();
        let outcomes = PROBE.with(|probe| {
            probe
                .borrow_mut()
                .take()
                .map(|probe| probe.outcomes)
                .unwrap_or_default()
        });
        println!("\n==== mac_native_probe summary ({reason}) ====");
        let failed = outcomes.iter().filter(|outcome| !outcome.pass).count();
        for outcome in &outcomes {
            println!(
                "{} {}",
                if outcome.pass { "PASS" } else { "FAIL" },
                outcome.name
            );
        }
        println!("{} passed, {failed} failed", outcomes.len() - failed);
        let failed = failed > 0 || reason != "done";
        std::process::exit(i32::from(failed));
    }

    pub fn run() {
        log::set_logger(&ProbeLogger).expect("the probe installs the first logger");
        log::set_max_level(log::LevelFilter::Trace);
        *START.lock().expect("start lock") = Some(Instant::now());
        let _guard = tis::InputSourceGuard::remember();
        std::thread::spawn(|| {
            std::thread::sleep(TIMEOUT);
            eprintln!("probe: timed out after {TIMEOUT:?}");
            tis::restore();
            std::process::exit(3);
        });
        // The first frame and accessibility tree land before the first step.
        native::after(1500, start);
        let options = WindowOptions::default()
            .title("rui mac native probe")
            .size(980.0, 820.0);
        App::new().run_view_with_options(options, view::ProbeView::default());
    }
}
