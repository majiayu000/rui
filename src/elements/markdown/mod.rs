//! Markdown rendered with rui's own elements.
//!
//! [`markdown`] draws CommonMark with the GitHub extensions chat replies use:
//! tables, strikethrough and task lists. Paragraphs are [`StyledText`]s, so
//! emphasis, strong, inline code and links are runs of one wrapped paragraph.
//!
//! A source is parsed once: documents are kept by source, and a source that
//! extends a kept one, as a reply streamed token by token does, is parsed again
//! only from the kept one's last top-level block.
//!
//! [`StyledText`]: crate::elements::StyledText

mod parse;
mod render;

pub use parse::{ParseCounts, markdown_parse_counts};

use crate::advanced_ui::Theme;
use crate::core::SharedStr;
use crate::elements::text::{LinkHandler, TextStyle};
use crate::elements::{AnyElement, Div, IntoElement, SelectionScope, selection_scope};
use std::rc::Rc;

/// A Markdown document, drawn as a column of blocks as wide as its container.
/// Make one with [`markdown`].
pub struct Markdown {
    source: SharedStr,
    theme: Theme,
    size: Option<f32>,
    on_link: Option<LinkHandler>,
}

/// Markdown `source` drawn with the default theme.
pub fn markdown(source: impl Into<SharedStr>) -> Markdown {
    Markdown {
        source: source.into(),
        theme: Theme::default(),
        size: None,
        on_link: None,
    }
}

impl Markdown {
    /// Draw with `theme`'s text, surface, border and accent colors.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Body text size; headings and code scale with it. By default the text
    /// size scaled by the theme's text scale.
    pub fn text_size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }

    /// Called with a link's destination when it is clicked.
    pub fn on_link(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_link = Some(Rc::new(handler));
        self
    }

    /// The document in a [`SelectionScope`], so its text can be selected
    /// across blocks and copied, highlighted with the theme's selection color.
    /// Without it, Markdown is not selectable: a scope lays out and paints its
    /// text as cheaply, but collects it again every frame a selection shows.
    pub fn selectable(self) -> SelectionScope {
        let theme = self.theme;
        selection_scope(self.build()).theme(&theme)
    }

    /// The source's blocks as elements.
    pub fn build(self) -> Div {
        let size = self
            .size
            .unwrap_or(TextStyle::default().font_size * self.theme.typography.text_scale);
        let look = render::Look::new(&self.theme, size, self.on_link);
        let document = parse::document(&self.source);
        look.blocks(document.blocks.iter().map(|(_, block)| &**block))
            .w_full()
            .text_color(look.text)
            .text_size(size)
            .line_height(look.line_height)
    }
}

impl IntoElement for Markdown {
    type Element = Div;

    fn into_element(self) -> Div {
        self.build()
    }
}

impl From<Markdown> for AnyElement {
    fn from(markdown: Markdown) -> Self {
        AnyElement::new(markdown.build())
    }
}

#[cfg(test)]
mod tests;
