//! Streaming reply example
//!
//! A background thread produces a reply word by word and sends each word to the UI
//! through an `AppHandle`, which wakes the event loop without any user input.

use rui::core::entity::Entity;
use rui::prelude::*;
use std::time::Duration;

const REPLY: &str = "Each word of this reply comes from a background thread, \
    which hands it to the UI thread through an AppHandle.";

#[derive(Default)]
struct StreamingReply {
    reply: Option<Entity<String>>,
}

impl View for StreamingReply {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        let reply = *self
            .reply
            .get_or_insert_with(|| cx.app_mut().create(String::new()));
        let shown = cx
            .app()
            .get(reply)
            .map(|text| text.clone())
            .unwrap_or_default();
        let handle = cx.app_handle();

        div()
            .w(640.0)
            .h(320.0)
            .bg(Color::hex(0x1a202c))
            .p(24.0)
            .flex_col()
            .gap(16.0)
            .child(button("Ask").primary().on_click(move || {
                let handle = handle.clone();
                std::thread::spawn(move || {
                    let _ = handle.update(move |cx| {
                        if let Some(mut text) = cx.get_mut(reply) {
                            text.clear();
                        }
                        cx.request_rebuild();
                    });
                    for word in REPLY.split(' ') {
                        std::thread::sleep(Duration::from_millis(120));
                        let word = word.to_string();
                        let sent = handle.update(move |cx| {
                            if let Some(mut text) = cx.get_mut(reply) {
                                if !text.is_empty() {
                                    text.push(' ');
                                }
                                text.push_str(&word);
                            }
                            cx.request_rebuild();
                        });
                        if sent.is_err() {
                            return;
                        }
                    }
                });
            }))
            .child(text(shown).size(18.0).color(Color::WHITE))
    }
}

fn main() {
    App::new().run_view(StreamingReply::default());
}
