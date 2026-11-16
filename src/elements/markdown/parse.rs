//! Markdown source to blocks, cached per source and extended in place when a
//! source grows at its end.

use pulldown_cmark::{Alignment, Event, Options, Parser, Tag, TagEnd};
use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::sync::Arc;

/// Sources kept parsed: the documents a window shows at once, such as a chat's
/// messages.
const CACHE_CAPACITY: usize = 128;

/// How deep quotes and lists nest. One nested deeper is read as part of the
/// one at this depth, its text kept: every level is a level of elements, which
/// are built, laid out and painted by recursion, and a line of a few thousand
/// `>` would run the thread out of stack.
pub(super) const MAX_NESTING: usize = 16;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct InlineStyle {
    pub strong: bool,
    pub emphasis: bool,
    pub strikethrough: bool,
    pub code: bool,
}

/// Bytes of an [`Inline`]'s text in one style.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Run {
    pub bytes: Range<usize>,
    pub style: InlineStyle,
    pub link: Option<Arc<str>>,
}

/// A paragraph's text and its styled runs, which cover it in order.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Inline {
    pub text: Arc<str>,
    pub runs: Vec<Run>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Item {
    /// `Some(checked)` for a task list item.
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Block {
    Paragraph(Inline),
    Heading(u8, Inline),
    Code(Arc<str>),
    Quote(Vec<Block>),
    /// `start` is the first number of an ordered list.
    List {
        start: Option<u64>,
        items: Vec<Item>,
    },
    Rule,
    Table {
        alignments: Vec<Alignment>,
        head: Vec<Inline>,
        rows: Vec<Vec<Inline>>,
    },
    Image {
        url: Arc<str>,
        alt: String,
    },
}

/// A parsed source: its top-level blocks, each with the byte its source
/// construct starts at.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Document {
    pub blocks: Vec<(usize, Arc<Block>)>,
    /// Link reference definitions can resolve links in earlier blocks, so a
    /// document with any is parsed again in full when it grows.
    has_references: bool,
}

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

/// Builds the inline text of one paragraph, heading or cell.
#[derive(Default)]
struct InlineBuilder {
    text: String,
    runs: Vec<Run>,
    style: InlineStyle,
    /// Depths of nested emphasis, strong and strikethrough.
    depth: [u8; 3],
    links: Vec<Arc<str>>,
}

impl InlineBuilder {
    fn push(&mut self, text: &str, code: bool) {
        if text.is_empty() {
            return;
        }
        let style = InlineStyle { code, ..self.style };
        let link = self.links.last().cloned();
        let start = self.text.len();
        self.text.push_str(text);
        let end = self.text.len();
        match self.runs.last_mut() {
            Some(run) if run.style == style && run.link == link => run.bytes.end = end,
            _ => self.runs.push(Run {
                bytes: start..end,
                style,
                link,
            }),
        }
    }

    fn nest(&mut self, which: usize, open: bool) {
        let depth = &mut self.depth[which];
        *depth = if open {
            depth.saturating_add(1)
        } else {
            depth.saturating_sub(1)
        };
        self.style.emphasis = self.depth[0] > 0;
        self.style.strong = self.depth[1] > 0;
        self.style.strikethrough = self.depth[2] > 0;
    }

    fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    /// The text built so far; styles and links stay open.
    fn take(&mut self) -> Inline {
        self.runs.shrink_to_fit();
        Inline {
            text: Arc::from(std::mem::take(&mut self.text)),
            runs: std::mem::take(&mut self.runs),
        }
    }
}

/// An open container block.
enum Frame {
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Item>,
    },
    Item(Item),
    Table {
        alignments: Vec<Alignment>,
        head: Vec<Inline>,
        rows: Vec<Vec<Inline>>,
        row: Vec<Inline>,
    },
}

struct Builder {
    /// Byte of the source the parsed text starts at.
    base: usize,
    blocks: Vec<(usize, Arc<Block>)>,
    /// Where the top-level construct being read starts.
    top_start: usize,
    stack: Vec<Frame>,
    /// Open quotes, lists and items past [`MAX_NESTING`], which have no frame.
    flattened: usize,
    inline: InlineBuilder,
    code: Option<String>,
    /// Destination and alt text of an image being read.
    image: Option<(Arc<str>, String)>,
}

impl Builder {
    fn push_block(&mut self, block: Block) {
        match self.stack.last_mut() {
            None => self.blocks.push((self.top_start, Arc::new(block))),
            Some(Frame::Quote(blocks)) | Some(Frame::Item(Item { blocks, .. })) => {
                blocks.push(block)
            }
            // Lists hold only items and tables only cells.
            Some(Frame::List { .. }) | Some(Frame::Table { .. }) => {}
        }
    }

    /// Open the quote, list or item `frame`, unless it is nested too deep to
    /// have one: its blocks are then those of the frame it is in.
    fn open(&mut self, frame: Frame) {
        let nested = |frame: &Frame| matches!(frame, Frame::Quote(_) | Frame::List { .. });
        // An item has a frame whenever its list does.
        let too_deep =
            nested(&frame) && self.stack.iter().filter(|open| nested(open)).count() >= MAX_NESTING;
        if self.flattened > 0 || too_deep {
            self.flattened += 1;
        } else {
            self.stack.push(frame);
        }
    }

    /// Whether the quote, list or item ending has a frame to pop.
    fn close(&mut self) -> bool {
        let framed = self.flattened == 0;
        self.flattened = self.flattened.saturating_sub(1);
        framed
    }

    /// End the text read so far as a paragraph, or as a heading of `level`.
    fn flush(&mut self, heading: Option<u8>) {
        if self.inline.is_empty() {
            self.inline = InlineBuilder {
                style: self.inline.style,
                depth: self.inline.depth,
                links: std::mem::take(&mut self.inline.links),
                ..InlineBuilder::default()
            };
            return;
        }
        let inline = self.inline.take();
        self.push_block(match heading {
            Some(level) => Block::Heading(level, inline),
            None => Block::Paragraph(inline),
        });
    }

    fn in_table(&self) -> bool {
        matches!(self.stack.last(), Some(Frame::Table { .. }))
    }

    fn text(&mut self, text: &str) {
        if let Some(code) = &mut self.code {
            code.push_str(text);
        } else if let Some((_, alt)) = &mut self.image {
            alt.push_str(text);
        } else {
            self.inline.push(text, false);
        }
    }

    fn event(&mut self, event: Event, range: Range<usize>, depth: &mut usize) {
        if *depth == 0 {
            self.top_start = self.base + range.start;
        }
        match event {
            Event::Start(tag) => {
                *depth += 1;
                self.start(tag);
            }
            Event::End(tag) => {
                *depth = depth.saturating_sub(1);
                self.end(tag);
            }
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => self.text(&text),
            Event::Code(code) => {
                if self.image.is_some() {
                    self.text(&code);
                } else {
                    self.inline.push(&code, true);
                }
            }
            Event::InlineMath(math) | Event::DisplayMath(math) => self.text(&math),
            Event::FootnoteReference(label) => self.text(&format!("[^{label}]")),
            Event::SoftBreak => self.text(" "),
            Event::HardBreak => self.text("\n"),
            Event::Rule => {
                self.flush(None);
                self.push_block(Block::Rule);
            }
            Event::TaskListMarker(checked) => {
                // The frame of an item without one is another item's.
                if self.flattened == 0
                    && let Some(Frame::Item(item)) = self.stack.last_mut()
                {
                    item.task = Some(checked);
                }
            }
        }
    }

    fn start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph | Tag::Heading { .. } | Tag::HtmlBlock => self.flush(None),
            Tag::CodeBlock(_) => {
                self.flush(None);
                self.code = Some(String::new());
            }
            Tag::BlockQuote(_) => {
                self.flush(None);
                self.open(Frame::Quote(Vec::new()));
            }
            Tag::List(start) => {
                self.flush(None);
                self.open(Frame::List {
                    start,
                    items: Vec::new(),
                });
            }
            Tag::Item => {
                self.flush(None);
                self.open(Frame::Item(Item {
                    task: None,
                    blocks: Vec::new(),
                }));
            }
            Tag::Table(alignments) => {
                self.flush(None);
                self.stack.push(Frame::Table {
                    alignments,
                    head: Vec::new(),
                    rows: Vec::new(),
                    row: Vec::new(),
                });
            }
            Tag::Emphasis => self.inline.nest(0, true),
            Tag::Strong => self.inline.nest(1, true),
            Tag::Strikethrough => self.inline.nest(2, true),
            Tag::Link { dest_url, .. } => self.inline.links.push(Arc::from(&*dest_url)),
            Tag::Image { dest_url, .. } => {
                if !self.in_table() {
                    self.flush(None);
                }
                self.image = Some((Arc::from(&*dest_url), String::new()));
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::HtmlBlock => self.flush(None),
            TagEnd::Heading(level) => self.flush(Some(level as u8)),
            TagEnd::CodeBlock => {
                let mut code = self.code.take().unwrap_or_default();
                code.truncate(code.trim_end_matches('\n').len());
                self.push_block(Block::Code(Arc::from(code)));
            }
            TagEnd::BlockQuote(_) => {
                self.flush(None);
                if self.close()
                    && let Some(Frame::Quote(blocks)) = self.stack.pop()
                {
                    self.push_block(Block::Quote(blocks));
                }
            }
            TagEnd::Item => {
                self.flush(None);
                if self.close()
                    && let Some(Frame::Item(item)) = self.stack.pop()
                    && let Some(Frame::List { items, .. }) = self.stack.last_mut()
                {
                    items.push(item);
                }
            }
            TagEnd::List(_) => {
                if self.close()
                    && let Some(Frame::List { start, items }) = self.stack.pop()
                {
                    self.push_block(Block::List { start, items });
                }
            }
            TagEnd::TableCell => {
                let cell = self.inline.take();
                if let Some(Frame::Table { row, .. }) = self.stack.last_mut() {
                    row.push(cell);
                }
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                if let Some(Frame::Table {
                    head, rows, row, ..
                }) = self.stack.last_mut()
                {
                    let cells = std::mem::take(row);
                    if matches!(tag, TagEnd::TableHead) {
                        *head = cells;
                    } else {
                        rows.push(cells);
                    }
                }
            }
            TagEnd::Table => {
                if let Some(Frame::Table {
                    alignments,
                    head,
                    rows,
                    ..
                }) = self.stack.pop()
                {
                    self.push_block(Block::Table {
                        alignments,
                        head,
                        rows,
                    });
                }
            }
            TagEnd::Emphasis => self.inline.nest(0, false),
            TagEnd::Strong => self.inline.nest(1, false),
            TagEnd::Strikethrough => self.inline.nest(2, false),
            TagEnd::Link => {
                self.inline.links.pop();
            }
            TagEnd::Image => {
                if let Some((url, alt)) = self.image.take() {
                    if self.in_table() {
                        self.inline.push(&alt, false);
                    } else {
                        self.push_block(Block::Image { url, alt });
                    }
                }
            }
            _ => {}
        }
    }
}

/// Parse `source`, whose first byte is byte `base` of the whole document.
fn parse_from(source: &str, base: usize) -> Document {
    let mut builder = Builder {
        base,
        blocks: Vec::new(),
        top_start: base,
        stack: Vec::new(),
        flattened: 0,
        inline: InlineBuilder::default(),
        code: None,
        image: None,
    };
    let mut depth = 0;
    let mut events = Parser::new_ext(source, options()).into_offset_iter();
    for (event, range) in events.by_ref() {
        builder.event(event, range, &mut depth);
    }
    builder.flush(None);
    Document {
        blocks: builder.blocks,
        has_references: events.reference_definitions().iter().next().is_some(),
    }
}

/// `source` parsed again from the start of the last top-level block of
/// `previous`, the document of a prefix of it: appended text can only change
/// the blocks still open at the prefix's end, and those are within the last
/// top-level block. A block not ended by a blank line is parsed again too,
/// since a GitHub table takes in the lines after it as rows, and so is a
/// list, which no blank line ends: an item after one is the list's next, and
/// a line after its last item can be more of that item. `None` when a link
/// reference definition could resolve links before the restart.
pub(super) fn extend(previous: &Document, source: &str) -> Option<Document> {
    if previous.has_references {
        return None;
    }
    // From the start of its line, so an indented code block keeps its indent.
    let line_start = |at: usize| source[..at].rfind('\n').map_or(0, |newline| newline + 1);
    let mut earlier_blocks = previous.blocks.iter().rev();
    let mut start = line_start(earlier_blocks.next().map_or(0, |(start, _)| *start));
    for (earlier, block) in earlier_blocks {
        // The blocks an image splits a paragraph into share its start, which
        // is past its line's start when the paragraph is indented.
        let ended = *earlier < start
            && !matches!(**block, Block::List { .. })
            && source[*earlier..start]
                .lines()
                .skip(1)
                .any(|line| line.trim().is_empty());
        if ended {
            break;
        }
        start = line_start(*earlier);
    }
    let tail = parse_from(&source[start..], start);
    if tail.has_references {
        return None;
    }
    let mut blocks: Vec<_> = previous
        .blocks
        .iter()
        .take_while(|(block_start, _)| *block_start < start)
        .cloned()
        .collect();
    blocks.extend(tail.blocks);
    Some(Document {
        blocks,
        has_references: false,
    })
}

/// `source` parsed in full.
pub(super) fn parse(source: &str) -> Document {
    parse_from(source, 0)
}

struct Entry {
    source: Arc<str>,
    document: Arc<Document>,
    used: u64,
}

#[derive(Default)]
struct Cache {
    entries: Vec<Entry>,
    clock: u64,
}

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::default();
    static PARSES: Cell<ParseCounts> = const { Cell::new(ParseCounts { full: 0, extended: 0 }) };
}

/// Parses this thread has run, for tests and benchmarks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseCounts {
    /// Sources parsed from their first byte.
    pub full: usize,
    /// Sources that grew at the end, parsed from their last block.
    pub extended: usize,
}

/// How many Markdown sources this thread has parsed, by how.
pub fn markdown_parse_counts() -> ParseCounts {
    PARSES.with(Cell::get)
}

fn count(extended: bool) {
    PARSES.with(|counts| {
        let mut value = counts.get();
        if extended {
            value.extended += 1;
        } else {
            value.full += 1;
        }
        counts.set(value);
    });
}

/// The document of `source`, parsed once for every source the cache keeps.
/// A source that extends a kept one, as a streamed reply does, is parsed from
/// the kept one's last block and replaces it.
pub(super) fn document(source: &str) -> Arc<Document> {
    CACHE.with(|cache| {
        let cache = &mut *cache.borrow_mut();
        cache.clock += 1;
        let clock = cache.clock;
        if let Some(entry) = cache
            .entries
            .iter_mut()
            .find(|entry| *entry.source == *source)
        {
            entry.used = clock;
            return Arc::clone(&entry.document);
        }
        let prefix = cache
            .entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                source.len() > entry.source.len() && source.starts_with(&*entry.source)
            })
            .max_by_key(|(_, entry)| entry.source.len())
            .map(|(index, _)| index);
        let extended = prefix.and_then(|index| extend(&cache.entries[index].document, source));
        count(extended.is_some());
        let document = Arc::new(extended.unwrap_or_else(|| parse(source)));
        let entry = Entry {
            source: Arc::from(source),
            document: Arc::clone(&document),
            used: clock,
        };
        match prefix {
            Some(index) => cache.entries[index] = entry,
            None if cache.entries.len() < CACHE_CAPACITY => cache.entries.push(entry),
            None => {
                if let Some(oldest) = cache.entries.iter_mut().min_by_key(|entry| entry.used) {
                    *oldest = entry;
                }
            }
        }
        document
    })
}
