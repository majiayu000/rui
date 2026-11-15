//! Streaming a reply with async tasks
//!
//! "Ask" spawns a task on the UI thread. For each word it awaits slow work on a
//! background thread, then shows the word. The UI thread stays free while the
//! work runs, and each result wakes the event loop on its own.
//!
//! `RUI_SPAWN_CHECK=1` asks once at startup, reports where each word was made
//! and how long it took to reach the screen, and quits when the reply is done.

use rui::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

const REPLY: &str = "each word is made on a background thread and shown by a UI task";
const WORK_PER_WORD: Duration = Duration::from_millis(80);

struct Word {
    text: String,
    thread: String,
    finished_at: Instant,
}

#[derive(Default)]
struct SpawnReply {
    reply: String,
    asked: bool,
    /// When each word's work finished, and when the frame showing it rendered.
    timings: SharedTimings,
}

impl SpawnReply {
    fn ask(cx: &mut ViewContext<Self>, check: Option<SharedTimings>) {
        cx.spawn(move |this, cx| async move {
            for word in REPLY.split(' ') {
                let word = word.to_string();
                let made = cx
                    .background_spawn(async move {
                        // Stands in for network or disk work.
                        std::thread::sleep(WORK_PER_WORD);
                        Word {
                            text: word,
                            thread: std::thread::current()
                                .name()
                                .unwrap_or("unnamed")
                                .to_string(),
                            finished_at: Instant::now(),
                        }
                    })
                    .await;
                if let Some(timings) = &check {
                    timings
                        .borrow_mut()
                        .push((made.thread.clone(), made.finished_at, None));
                }
                // The view renders again after this; nothing needs notifying.
                let shown = this.update(&cx, move |this, _| {
                    if !this.reply.is_empty() {
                        this.reply.push(' ');
                    }
                    this.reply.push_str(&made.text);
                });
                if shown.is_err() {
                    return;
                }
            }
            if let Some(timings) = check {
                cx.update(|cx| report_and_quit(cx, &timings));
            }
        })
        .detach();
    }
}

type SharedTimings = Rc<RefCell<Vec<(String, Instant, Option<Instant>)>>>;

impl View for SpawnReply {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        if let Some(last) = self.timings.borrow_mut().last_mut()
            && last.2.is_none()
        {
            last.2 = Some(Instant::now());
        }
        if !self.asked && std::env::var_os("RUI_SPAWN_CHECK").is_some() {
            self.asked = true;
            Self::ask(cx, Some(Rc::clone(&self.timings)));
        }

        div()
            .w(640.0)
            .h(320.0)
            .bg(Color::hex(0x1a202c))
            .p(24.0)
            .flex_col()
            .gap(16.0)
            .child(button("Ask").primary().on_click(cx.listener(|this, cx| {
                this.reply.clear();
                Self::ask(cx, None);
            })))
            .child(text(self.reply.clone()).size(18.0).color(Color::WHITE))
    }
}

fn report_and_quit(cx: &mut AppContext, timings: &SharedTimings) {
    let timings = timings.borrow();
    let mut threads: Vec<&str> = timings
        .iter()
        .map(|(thread, _, _)| thread.as_str())
        .collect();
    threads.sort_unstable();
    threads.dedup();
    // The last word's frame renders after this update, so it is not measured.
    let latencies: Vec<Duration> = timings
        .iter()
        .filter_map(|(_, finished, rendered)| rendered.map(|rendered| rendered - *finished))
        .collect();
    let max = latencies.iter().max().copied().unwrap_or_default();
    eprintln!(
        "spawn_reply: {} words, made on {:?}, ui thread {:?}",
        timings.len(),
        threads,
        std::thread::current().name()
    );
    eprintln!(
        "spawn_reply: work finished -> frame rendered, {} words measured, max {:?}",
        latencies.len(),
        max
    );
    cx.quit();
}

fn main() {
    App::new().run_view(SpawnReply::default());
}
