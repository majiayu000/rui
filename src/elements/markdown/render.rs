//! Parsed Markdown blocks to elements.

use super::parse::{Block, Inline, Item};
use crate::advanced_ui::Theme;
use crate::core::color::Color;
use crate::core::geometry::Size;
use crate::elements::text::{FontWeight, LinkHandler, TextAlign};
use crate::elements::{
    AnyElement, Div, StyledText, div, image, shared_styled_text, span_range, text,
};
use pulldown_cmark::Alignment;
use std::cell::RefCell;
use std::collections::HashMap;

/// The monospace family code is drawn in, where the platform has a known one.
#[cfg(target_os = "macos")]
const CODE_FONT: Option<&str> = Some("SF Mono");
#[cfg(windows)]
const CODE_FONT: Option<&str> = Some("Consolas");
#[cfg(not(any(target_os = "macos", windows)))]
const CODE_FONT: Option<&str> = None;

/// Heading sizes, from `#` to `######`, relative to body text.
const HEADING_SCALE: [f32; 6] = [1.75, 1.5, 1.25, 1.1, 1.0, 0.9];
/// Height of an image whose size is unknown, such as a remote one, in body
/// text sizes.
const IMAGE_HEIGHT: f32 = 12.0;
/// Local image sizes kept read; past this many the record starts over.
const IMAGE_SIZES_KEPT: usize = 256;

thread_local! {
    static IMAGE_SIZES: RefCell<HashMap<String, Option<Size>>> = RefCell::default();
}

/// The pixel size of the local image file `url`, read from its header the
/// first time. `None` for a remote image or one that cannot be read.
fn image_size(url: &str) -> Option<Size> {
    let path = url.strip_prefix("file://").unwrap_or(url);
    if path.contains("://") {
        return None;
    }
    IMAGE_SIZES.with(|sizes| {
        let mut sizes = sizes.borrow_mut();
        if let Some(size) = sizes.get(path) {
            return *size;
        }
        let size = ::image::ImageReader::open(path)
            .and_then(|reader| reader.with_guessed_format())
            .ok()
            .and_then(|reader| reader.into_dimensions().ok())
            .map(|(width, height)| Size::new(width as f32, height as f32));
        if sizes.len() >= IMAGE_SIZES_KEPT {
            sizes.clear();
        }
        sizes.insert(path.to_string(), size);
        size
    })
}

/// Colors and sizes the blocks are drawn with, from the theme.
pub(super) struct Look {
    pub size: f32,
    pub line_height: f32,
    pub text: Color,
    pub muted: Color,
    pub link: Color,
    pub code_background: Color,
    pub surface: Color,
    pub border: Color,
    pub accent: Color,
    pub on_accent: Color,
    pub radius: f32,
    pub on_link: Option<LinkHandler>,
}

impl Look {
    pub fn new(theme: &Theme, size: f32, on_link: Option<LinkHandler>) -> Self {
        let colors = &theme.colors;
        Self {
            size,
            line_height: theme.typography.line_height,
            text: colors.text,
            muted: colors.text.with_alpha(0.72),
            link: colors.outline.rest.foreground,
            code_background: colors.surface_muted,
            surface: colors.surface,
            border: colors.border,
            accent: colors.primary.rest.background,
            on_accent: colors.text_on_accent,
            radius: theme.radius.control,
            on_link,
        }
    }

    fn gap(&self) -> f32 {
        self.size * 0.75
    }

    fn line_box(&self) -> f32 {
        self.size * self.line_height
    }

    fn paragraph(&self, inline: &Inline) -> StyledText {
        let mut has_link = false;
        let spans = inline.runs.iter().map(|run| {
            let mut span = span_range(run.bytes.clone());
            let style = run.style;
            if style.strong {
                span = span.bold();
            }
            if style.emphasis {
                span = span.italic();
            }
            if style.strikethrough {
                span = span.strikethrough();
            }
            if style.code {
                span = span.background(self.code_background);
                if let Some(font) = CODE_FONT {
                    span = span.font(font);
                }
            }
            if let Some(link) = &run.link {
                has_link = true;
                span = span.color(self.link).underline().link(link.clone());
            }
            span
        });
        let paragraph = shared_styled_text(inline.text.clone(), spans.collect::<Vec<_>>());
        match &self.on_link {
            Some(handler) if has_link => paragraph.on_link_rc(handler.clone()),
            _ => paragraph,
        }
    }

    pub fn blocks<'a>(&self, blocks: impl IntoIterator<Item = &'a Block>) -> Div {
        div()
            .flex_col()
            .gap(self.gap())
            .children(blocks.into_iter().map(|block| self.block(block)))
    }

    fn block(&self, block: &Block) -> AnyElement {
        match block {
            Block::Paragraph(inline) => self.paragraph(inline).into(),
            Block::Heading(level, inline) => {
                let scale = HEADING_SCALE[usize::from(level.clamp(&1, &6) - 1)];
                div()
                    .text_size(self.size * scale)
                    .font_weight(if *level <= 2 {
                        FontWeight::Bold
                    } else {
                        FontWeight::Semibold
                    })
                    .child(self.paragraph(inline))
                    .into()
            }
            Block::Code(code) => {
                // Styled text places each line's leading spaces, where a text
                // primitive starts at its first glyph's ink.
                let mut run = span_range(0..code.len()).size(self.size * 0.9);
                if let Some(font) = CODE_FONT {
                    run = run.font(font);
                }
                let code = shared_styled_text(code.clone(), [run]);
                div()
                    .bg(self.code_background)
                    .rounded(self.radius)
                    .px(self.size)
                    .py(self.size * 0.75)
                    .child(code)
                    .into()
            }
            Block::Quote(blocks) => div()
                .flex_row()
                .gap(self.size * 0.75)
                .child(div().w(3.0).rounded(1.5).bg(self.border))
                .child(
                    self.blocks(blocks)
                        .w(0.0)
                        .flex_grow(1.0)
                        .text_color(self.muted),
                )
                .into(),
            Block::List { start, items } => div()
                .flex_col()
                .gap(self.size * 0.35)
                .children(items.iter().enumerate().map(|(index, item)| {
                    let marker = start.map(|start| format!("{}.", start + index as u64));
                    self.item(item, marker)
                }))
                .into(),
            Block::Rule => div().h(1.0).bg(self.border).into(),
            Block::Table {
                alignments,
                head,
                rows,
            } => {
                let row = |cells: &[Inline], background: Color, header: bool| {
                    div()
                        .flex_row()
                        .gap(1.0)
                        .children(cells.iter().enumerate().map(|(column, cell)| {
                            let align = match alignments.get(column) {
                                Some(Alignment::Center) => TextAlign::Center,
                                Some(Alignment::Right) => TextAlign::Right,
                                _ => TextAlign::Left,
                            };
                            let mut cell_div = div()
                                .flex_col()
                                .w(0.0)
                                .flex_grow(1.0)
                                .bg(background)
                                .px(self.size * 0.6)
                                .py(self.size * 0.35)
                                .child(self.paragraph(cell).align(align));
                            if header {
                                cell_div = cell_div.font_weight(FontWeight::Semibold);
                            }
                            cell_div
                        }))
                };
                div()
                    .flex_col()
                    .gap(1.0)
                    .p(1.0)
                    .bg(self.border)
                    .rounded(self.radius)
                    .overflow_hidden()
                    .child(row(head, self.code_background, true))
                    .children(rows.iter().map(|cells| row(cells, self.surface, false)))
                    .into()
            }
            Block::Image { url, .. } => {
                let picture = image(url.to_string()).placeholder(self.code_background);
                match image_size(url) {
                    Some(size) => picture.natural_size(size).into(),
                    None => picture.h(self.size * IMAGE_HEIGHT).contain().into(),
                }
            }
        }
    }

    fn item(&self, item: &Item, number: Option<String>) -> Div {
        let side = self.size * 0.9;
        let (marker, top): (AnyElement, f32) = match item.task {
            Some(checked) => {
                let mut check = div()
                    .w(side)
                    .h(side)
                    .rounded(side * 0.25)
                    .border(1.0, if checked { self.accent } else { self.border })
                    .flex()
                    .items_center()
                    .justify_center();
                // A border is drawn only with a background.
                check = check.bg(Color::TRANSPARENT);
                if checked {
                    check = check.bg(self.accent).child(
                        text("\u{2713}")
                            .size(side * 0.8)
                            .line_height(1.0)
                            .color(self.on_accent),
                    );
                }
                (check.into(), (self.line_box() - side) * 0.5)
            }
            None => (
                text(number.unwrap_or_else(|| "\u{2022}".to_string()))
                    .color(self.muted)
                    .into(),
                0.0,
            ),
        };
        div()
            .flex_row()
            .gap(self.size * 0.5)
            .child(
                div()
                    .min_w(self.size * 1.25)
                    .pt(top)
                    .flex_row()
                    .justify_end()
                    .child(marker),
            )
            .child(
                self.blocks(&item.blocks)
                    .gap(self.size * 0.35)
                    .w(0.0)
                    .flex_grow(1.0),
            )
    }
}
