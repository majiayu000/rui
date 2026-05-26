//! Ergonomic UI building blocks layered on the existing element system.

use crate::core::ElementId;
use crate::core::color::Color;
use crate::core::geometry::{Edges, Size};
use crate::core::style::{Shadow, Style};
use crate::elements::element::{
    AnyElement, Element, EventContext, LayoutContext, PaintContext, PointerEvent,
};
use crate::elements::text::{FontWeight, TextAlign};
use crate::elements::{Div, Text as RawText, div, text as raw_text};
use taffy::prelude::NodeId;

macro_rules! impl_div_wrapper_element {
    ($wrapper:ty) => {
        impl Element for $wrapper {
            fn id(&self) -> Option<ElementId> {
                Element::id(&self.inner)
            }

            fn style(&self) -> &Style {
                Element::style(&self.inner)
            }

            fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
                self.inner.layout(cx)
            }

            fn paint(&mut self, cx: &mut PaintContext) {
                self.inner.paint(cx);
            }

            fn handle_pointer_event(
                &mut self,
                cx: &mut EventContext,
                event: &PointerEvent,
            ) -> bool {
                self.inner.handle_pointer_event(cx, event)
            }

            fn handle_scroll_event(
                &mut self,
                cx: &mut EventContext,
                event: &crate::core::event::ScrollEvent,
            ) -> bool {
                self.inner.handle_scroll_event(cx, event)
            }

            fn handle_key_event(
                &mut self,
                cx: &mut EventContext,
                event: &crate::core::event::KeyEvent,
            ) -> bool {
                self.inner.handle_key_event(cx, event)
            }

            fn handle_window_event(&mut self, event: &crate::core::event::Event) -> bool {
                self.inner.handle_window_event(event)
            }

            fn children(&self) -> &[AnyElement] {
                Element::children(&self.inner)
            }
        }
    };
}

/// Main-axis alignment for [`Flex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainAxisAlignment {
    #[default]
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
}

/// Cross-axis alignment for [`Flex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrossAxisAlignment {
    Start,
    End,
    Center,
    #[default]
    Stretch,
}

/// Semantic container wrapper around the existing `Div` element.
pub struct Container {
    inner: Div,
}

impl Container {
    pub fn new() -> Self {
        Self { inner: div() }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.inner = self.inner.id(id);
        self
    }

    pub fn size(mut self, size: impl Into<Size>) -> Self {
        self.inner = self.inner.size(size);
        self
    }

    pub fn w(mut self, width: f32) -> Self {
        self.inner = self.inner.w(width);
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        self.inner = self.inner.h(height);
        self
    }

    pub fn min_w(mut self, width: f32) -> Self {
        self.inner = self.inner.min_w(width);
        self
    }

    pub fn min_h(mut self, height: f32) -> Self {
        self.inner = self.inner.min_h(height);
        self
    }

    pub fn max_w(mut self, width: f32) -> Self {
        self.inner = self.inner.max_w(width);
        self
    }

    pub fn max_h(mut self, height: f32) -> Self {
        self.inner = self.inner.max_h(height);
        self
    }

    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.inner = self.inner.flex_grow(grow);
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.inner = self.inner.p(padding);
        self
    }

    pub fn padding_edges(mut self, padding: impl Into<Edges>) -> Self {
        let padding = padding.into();
        self.inner = self
            .inner
            .pt(padding.top)
            .pr(padding.right)
            .pb(padding.bottom)
            .pl(padding.left);
        self
    }

    pub fn margin(mut self, margin: f32) -> Self {
        self.inner = self.inner.m(margin);
        self
    }

    pub fn background(mut self, color: impl Into<Color>) -> Self {
        self.inner = self.inner.bg(color);
        self
    }

    pub fn border(mut self, width: f32, color: impl Into<Color>) -> Self {
        self.inner = self.inner.border(width, color);
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.inner = self.inner.rounded(radius);
        self
    }

    pub fn shadow(mut self, shadow: Shadow) -> Self {
        self.inner = self.inner.shadow(shadow);
        self
    }

    pub fn child(mut self, child: impl Into<AnyElement>) -> Self {
        self.inner = self.inner.child(child);
        self
    }

    pub fn children<I, E>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<AnyElement>,
    {
        self.inner = self.inner.children(children);
        self
    }

    pub fn into_div(self) -> Div {
        self.inner
    }
}

impl Default for Container {
    fn default() -> Self {
        Self::new()
    }
}

impl_div_wrapper_element!(Container);

/// Flex layout wrapper that maps directly to the existing Taffy-backed `Div`.
pub struct Flex {
    inner: Div,
}

impl Flex {
    pub fn row() -> Self {
        Self {
            inner: div().flex_row(),
        }
    }

    pub fn column() -> Self {
        Self {
            inner: div().flex_col(),
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.inner = self.inner.id(id);
        self
    }

    pub fn size(mut self, size: impl Into<Size>) -> Self {
        self.inner = self.inner.size(size);
        self
    }

    pub fn w(mut self, width: f32) -> Self {
        self.inner = self.inner.w(width);
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        self.inner = self.inner.h(height);
        self
    }

    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.inner = self.inner.flex_grow(grow);
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.inner = self.inner.gap(spacing);
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.inner = self.inner.p(padding);
        self
    }

    pub fn background(mut self, color: impl Into<Color>) -> Self {
        self.inner = self.inner.bg(color);
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.inner = self.inner.rounded(radius);
        self
    }

    pub fn main_axis_alignment(mut self, alignment: MainAxisAlignment) -> Self {
        self.inner = match alignment {
            MainAxisAlignment::Start => self.inner.justify_start(),
            MainAxisAlignment::End => self.inner.justify_end(),
            MainAxisAlignment::Center => self.inner.justify_center(),
            MainAxisAlignment::SpaceBetween => self.inner.justify_between(),
            MainAxisAlignment::SpaceAround => self.inner.justify_around(),
        };
        self
    }

    pub fn cross_axis_alignment(mut self, alignment: CrossAxisAlignment) -> Self {
        self.inner = match alignment {
            CrossAxisAlignment::Start => self.inner.items_start(),
            CrossAxisAlignment::End => self.inner.items_end(),
            CrossAxisAlignment::Center => self.inner.items_center(),
            CrossAxisAlignment::Stretch => self.inner.items_stretch(),
        };
        self
    }

    pub fn child(mut self, child: impl Into<AnyElement>) -> Self {
        self.inner = self.inner.child(child);
        self
    }

    pub fn children<I, E>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<AnyElement>,
    {
        self.inner = self.inner.children(children);
        self
    }

    pub fn into_div(self) -> Div {
        self.inner
    }
}

impl_div_wrapper_element!(Flex);

/// Thin text wrapper for the advanced UI layer.
pub struct Text {
    inner: RawText,
}

impl Text {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            inner: raw_text(content),
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.inner = self.inner.id(id);
        self
    }

    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.inner = self.inner.color(color);
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.inner = self.inner.size(size);
        self
    }

    pub fn weight(mut self, weight: FontWeight) -> Self {
        self.inner = self.inner.weight(weight);
        self
    }

    pub fn medium(mut self) -> Self {
        self.inner = self.inner.medium();
        self
    }

    pub fn semibold(mut self) -> Self {
        self.inner = self.inner.semibold();
        self
    }

    pub fn bold(mut self) -> Self {
        self.inner = self.inner.bold();
        self
    }

    pub fn font(mut self, family: impl Into<String>) -> Self {
        self.inner = self.inner.font(family);
        self
    }

    pub fn line_height(mut self, height: f32) -> Self {
        self.inner = self.inner.line_height(height);
        self
    }

    pub fn align(mut self, align: TextAlign) -> Self {
        self.inner = self.inner.align(align);
        self
    }

    pub fn center(mut self) -> Self {
        self.inner = self.inner.center();
        self
    }

    pub fn right(mut self) -> Self {
        self.inner = self.inner.right();
        self
    }

    pub fn into_text(self) -> RawText {
        self.inner
    }
}

impl Element for Text {
    fn id(&self) -> Option<ElementId> {
        Element::id(&self.inner)
    }

    fn style(&self) -> &Style {
        Element::style(&self.inner)
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.inner.layout(cx)
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        self.inner.paint(cx);
    }
}

pub fn container() -> Container {
    Container::new()
}

pub fn row() -> Flex {
    Flex::row()
}

pub fn column() -> Flex {
    Flex::column()
}

pub fn text(content: impl Into<String>) -> Text {
    Text::new(content)
}
