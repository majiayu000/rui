//! Streaming reply example
//!
//! A background thread produces a reply word by word and sends each word to the UI
//! through an `AppHandle`, which wakes the event loop without any user input. The
//! reply is Markdown: each frame parses only the part of it the new word changed.

use rui::core::entity::Entity;
use rui::prelude::*;
use std::time::Duration;

const REPLY: &str = "Each word of this reply comes from a **background thread**, \
    which hands it to the UI thread through an `AppHandle`.\n\n\
    Markdown keeps up as it streams:\n\n\
    - *emphasis*, **strong** and ~~struck~~ text\n\
    - [links](https://github.com/majiayu000/rui) that call a handler\n\
    - code blocks:\n\n\
    ```rust\nlet reply = cx.app().get(reply);\n```";

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
        let theme = Theme::dark();

        div()
            .w(640.0)
            .h(420.0)
            .bg(theme.colors.surface)
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
            .child(
                markdown(shown)
                    .theme(theme)
                    .on_link(|url| println!("link clicked: {url}")),
            )
    }
}

fn main() {
    App::new().run_view(StreamingReply::default());
}
