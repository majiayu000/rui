//! The editor cases of `compare_frame`: a 10 000-line code view with a gutter,
//! per-token colors and the cursor's line highlighted, the same on both sides.

use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use rui::elements::element::Element;
use rui::elements::{VirtualListHandle, shared_styled_text, span_range, virtual_list};
use rui::{Color, div, text};

const EDITOR_LINES: usize = 10_000;
pub(super) const EDITOR_LINE_HEIGHT: f32 = 20.0;
const EDITOR_FONT_SIZE: f32 = 13.0;
const EDITOR_FONT: &str = "SF Mono";
const GUTTER_WIDTH: f32 = 64.0;
/// The first line on screen when an editor case starts.
pub(super) const EDITOR_TOP: usize = 5_000;
/// The cursor's line, counted from the first line on screen.
pub(super) const CURSOR_ROW: usize = 10;
const KEYWORDS: [&str; 8] = ["fn", "let", "pub", "if", "match", "return", "for", "impl"];
const IDENTS: [&str; 12] = [
    "frame", "layout", "glyph", "scene", "cursor", "buffer", "row", "offset", "width", "atlas",
    "span", "view",
];

#[derive(Clone, Copy)]
pub(super) enum Token {
    Plain,
    Keyword,
    Ident,
    Number,
    Str,
    Comment,
}

impl Token {
    fn color(self) -> u32 {
        match self {
            Token::Plain => 0xabb2bf,
            Token::Keyword => 0xc678dd,
            Token::Ident => 0xe5c07b,
            Token::Number => 0xd19a66,
            Token::Str => 0x98c379,
            Token::Comment => 0x7f848e,
        }
    }
}

/// Line `index` of the code view as colored tokens, the same on both sides:
/// a comment every ninth line, otherwise a statement of eleven tokens.
fn code_line(index: usize) -> Vec<(String, Token)> {
    let indent = " ".repeat(index % 4 * 4);
    let ident = |n: usize| IDENTS[(index * 7 + n * 5) % IDENTS.len()];
    if index.is_multiple_of(9) {
        let words = (0..4).map(ident).collect::<Vec<_>>().join(" ");
        return vec![(format!("{indent}// {words}"), Token::Comment)];
    }
    vec![
        (indent, Token::Plain),
        (KEYWORDS[index % KEYWORDS.len()].to_string(), Token::Keyword),
        (" ".to_string(), Token::Plain),
        (ident(0).to_string(), Token::Ident),
        (" = ".to_string(), Token::Plain),
        (ident(1).to_string(), Token::Ident),
        ("(".to_string(), Token::Plain),
        ((index * 31 % 1000).to_string(), Token::Number),
        (", ".to_string(), Token::Plain),
        (format!("\"{}\"", ident(2)), Token::Str),
        (");".to_string(), Token::Plain),
    ]
}

/// The one-character edit of frame `turn`: the line's first identifier, or its
/// comment, ends in a letter that changes every frame.
pub(super) fn edit_line(line: &mut [(String, Token)], turn: usize) {
    let letter = char::from(b'a' + (turn % 26) as u8);
    let token = match line.get_mut(3) {
        Some(token) => token,
        None => &mut line[0],
    };
    token.0.pop();
    token.0.push(letter);
}

/// A line as an editor keeps it: one string and its tokens as byte ranges.
pub(super) struct CodeLine {
    text: Arc<str>,
    tokens: Vec<(Range<usize>, Token)>,
}

impl CodeLine {
    pub(super) fn new(tokens: &[(String, Token)]) -> Self {
        let mut text = String::new();
        let tokens = tokens
            .iter()
            .map(|(content, token)| {
                let start = text.len();
                text.push_str(content);
                (start..text.len(), *token)
            })
            .collect();
        Self {
            text: text.into(),
            tokens,
        }
    }
}

/// The document as tokens, which edits change, and as the lines drawn from them.
pub(super) struct Document {
    pub(super) tokens: Vec<Vec<(String, Token)>>,
    pub(super) lines: Vec<CodeLine>,
}

pub(super) type SharedDocument = Rc<RefCell<Document>>;

/// A 10 000-line code view: a gutter of line numbers, per-token colors as styled
/// runs in a monospace font, and the cursor's line highlighted.
pub(super) fn code_view(
    handle: &VirtualListHandle,
    document: &SharedDocument,
    cursor: usize,
) -> impl Element {
    let document = Rc::clone(document);
    let lines = virtual_list(handle, EDITOR_LINES, EDITOR_LINE_HEIGHT, move |index| {
        let line = &document.borrow().lines[index];
        let spans = line
            .tokens
            .iter()
            .map(|(bytes, token)| span_range(bytes.clone()).color(Color::hex(token.color())));
        div()
            .w_full()
            .h(EDITOR_LINE_HEIGHT)
            .flex_row()
            .bg(Color::hex(if index == cursor {
                0x2c313c
            } else {
                0x21252b
            }))
            .child(
                div()
                    .w(GUTTER_WIDTH)
                    .pr(12.0)
                    .flex_row()
                    .justify_end()
                    .child(text((index + 1).to_string()).color(Color::hex(0x636d83))),
            )
            .child(shared_styled_text(Arc::clone(&line.text), spans))
    })
    .w_full()
    .h_full();
    div()
        .w_full()
        .h_full()
        .font_family(EDITOR_FONT)
        .text_size(EDITOR_FONT_SIZE)
        .line_height(1.5)
        .child(lines)
}

/// The first line on screen at frame `turn` of `editor_scroll`, wrapping before the end.
pub(super) fn scrolled_top(turn: usize) -> usize {
    EDITOR_TOP + turn % (EDITOR_LINES - EDITOR_TOP - 100)
}

pub(super) fn document() -> SharedDocument {
    let tokens: Vec<_> = (0..EDITOR_LINES).map(code_line).collect();
    let lines = tokens.iter().map(|line| CodeLine::new(line)).collect();
    Rc::new(RefCell::new(Document { tokens, lines }))
}
