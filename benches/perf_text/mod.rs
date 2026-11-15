//! Text fixtures: `text.paragraphs`, 200 wrapped paragraphs one of which
//! changes per frame, and `markdown.stream`, a chat of Markdown replies the
//! first of which grows by a token per frame, as a streamed reply does. The
//! settled replies are found parsed in the cache; the growing one is parsed
//! from its last block.

use rui::{Div, div, text};

/// Paragraph `index` of the wrapped text case: 8 to 127 words.
fn paragraph(index: usize) -> String {
    const WORDS: [&str; 12] = [
        "layout",
        "glyph",
        "shaping",
        "frame",
        "wrapped",
        "paragraph",
        "text",
        "cache",
        "the",
        "of",
        "a",
        "width",
    ];
    (0..8 + (index * 37) % 120)
        .map(|word| WORDS[(index * 7 + word * 3) % WORDS.len()])
        .collect::<Vec<_>>()
        .join(" ")
}

/// 200 wrapped paragraphs; paragraph 3 gets a new suffix each round.
pub fn paragraphs(round: usize) -> Div {
    div()
        .w_full()
        .flex_col()
        .p(16.0)
        .gap(8.0)
        .children((0..200).map(move |index| {
            let mut content = paragraph(index);
            if index == 3 {
                content.push_str(&format!(" revision {round}"));
            }
            text(content).size(14.0).line_height(1.4).wrap()
        }))
}

#[cfg(feature = "markdown")]
pub use markdown_chat::chat;

#[cfg(feature = "markdown")]
mod markdown_chat {
    use rui::{Div, SharedStr, div, markdown};
    use std::sync::{Arc, LazyLock};

    const SETTLED: usize = 12;
    /// Bytes the streamed reply grows by per frame.
    const TOKEN: usize = 6;
    /// The streamed reply starts this far in, about 3 KB.
    const STREAMED_FROM: usize = 3_000;

    /// Reply `index`: a heading, paragraphs of mixed inline styles, lists,
    /// code and a table, about 1 KB.
    fn reply(index: usize) -> String {
        let mut reply = format!("## Answer {index}\n\n");
        for paragraph in 0..3 {
            reply.push_str(&format!(
                "Paragraph {paragraph} explains the **layout pass**, the *glyph cache* and \
                 `TextMeasureCache::measure` in one wrapped run, with a \
                 [link](https://example.com/{index}) and ~~an old claim~~ for reply {index}.\n\n"
            ));
        }
        reply.push_str(
            "- first point about frames\n- second point with **strong** text\n  \
             1. nested step\n  2. another step\n\n",
        );
        reply.push_str(
            "```rust\nfn frame(cx: &mut Context) {\n    cx.layout();\n    cx.paint();\n}\n```\n\n",
        );
        reply.push_str(
            "| Stage | Cost |\n|:------|-----:|\n| layout | 1.2 ms |\n| paint | 0.4 ms |\n\n",
        );
        reply
    }

    static SETTLED_REPLIES: LazyLock<Vec<Arc<str>>> =
        LazyLock::new(|| (0..SETTLED).map(|index| Arc::from(reply(index))).collect());

    static STREAMED: LazyLock<String> =
        LazyLock::new(|| (100..110).map(reply).collect::<Vec<_>>().join(""));

    /// The chat as frame `round` shows it: the streamed reply first, where
    /// the window shows it.
    pub fn chat(round: usize) -> Div {
        let streamed = &STREAMED[..STREAMED_FROM + round * TOKEN];
        div()
            .w_full()
            .flex_col()
            .p(16.0)
            .gap(16.0)
            .child(markdown(streamed.to_string()))
            .children(
                SETTLED_REPLIES
                    .iter()
                    .map(|reply| markdown(SharedStr::new(Arc::clone(reply)))),
            )
    }
}
