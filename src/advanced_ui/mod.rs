//! Ergonomic UI building blocks layered on the existing element system.

mod button;
mod checkbox;
mod progress_bar;
mod scrollable;
mod segmented_control;
mod state;
mod text_field;
mod tokens;
mod toolbar;
mod tooltip;

use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityContext, AccessibilityError, AccessibilityNode, AccessibilityRole,
};
use crate::core::color::Color;
use crate::core::event::Cursor;
use crate::core::geometry::{Edges, Point, Size};
use crate::core::style::{Dimension, Shadow, Style};
use crate::elements::element::{
    AnyElement, Element, EventContext, LayoutContext, PaintContext, PointerEvent,
};
use crate::elements::text::{FontWeight, TextAlign};
use crate::elements::{Div, Text as RawText, div, text as raw_text};
use taffy::prelude::NodeId;

pub use button::{Button, button};
pub use checkbox::{Checkbox, checkbox};
pub use progress_bar::{ProgressBar, progress_bar};
pub use scrollable::{Scrollable, scrollable};
pub use segmented_control::{SegmentedControl, SegmentedOption, segmented_control};
pub use state::{
    IndexedInteractionRelease, IndexedInteractionState, InteractionRelease, InteractionState,
    require_finite, require_finite_non_negative, require_non_empty, validation_border_color,
};
pub use text_field::{TextField, text_field};
pub use tokens::{
    ControlColors, ControlSize, ControlState, ControlVariant, ControlVariantPalette, Theme,
    ThemeColors, ThemeDensity, ThemeMode, ThemeRadius, ThemeSpacing, ThemeStateTokens,
    ThemeTypography,
};
pub use toolbar::{Toolbar, toolbar};
pub use tooltip::{Tooltip, tooltip};

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

    pub fn width(mut self, width: impl Into<Dimension>) -> Self {
        self.inner = self.inner.width(width);
        self
    }

    pub fn w_percent(mut self, percent: f32) -> Self {
        self.inner = self.inner.w_percent(percent);
        self
    }

    pub fn w_full(mut self) -> Self {
        self.inner = self.inner.w_full();
        self
    }

    pub fn w_auto(mut self) -> Self {
        self.inner = self.inner.w_auto();
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        self.inner = self.inner.h(height);
        self
    }

    pub fn height(mut self, height: impl Into<Dimension>) -> Self {
        self.inner = self.inner.height(height);
        self
    }

    pub fn h_percent(mut self, percent: f32) -> Self {
        self.inner = self.inner.h_percent(percent);
        self
    }

    pub fn h_full(mut self) -> Self {
        self.inner = self.inner.h_full();
        self
    }

    pub fn h_auto(mut self) -> Self {
        self.inner = self.inner.h_auto();
        self
    }

    pub fn min_w(mut self, width: f32) -> Self {
        self.inner = self.inner.min_w(width);
        self
    }

    pub fn min_w_percent(mut self, percent: f32) -> Self {
        self.inner = self.inner.min_w_percent(percent);
        self
    }

    pub fn min_h(mut self, height: f32) -> Self {
        self.inner = self.inner.min_h(height);
        self
    }

    pub fn min_h_percent(mut self, percent: f32) -> Self {
        self.inner = self.inner.min_h_percent(percent);
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

    pub fn max_w_percent(mut self, percent: f32) -> Self {
        self.inner = self.inner.max_w_percent(percent);
        self
    }

    pub fn max_h_percent(mut self, percent: f32) -> Self {
        self.inner = self.inner.max_h_percent(percent);
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

    pub fn width(mut self, width: impl Into<Dimension>) -> Self {
        self.inner = self.inner.width(width);
        self
    }

    pub fn w_percent(mut self, percent: f32) -> Self {
        self.inner = self.inner.w_percent(percent);
        self
    }

    pub fn w_full(mut self) -> Self {
        self.inner = self.inner.w_full();
        self
    }

    pub fn w_auto(mut self) -> Self {
        self.inner = self.inner.w_auto();
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        self.inner = self.inner.h(height);
        self
    }

    pub fn height(mut self, height: impl Into<Dimension>) -> Self {
        self.inner = self.inner.height(height);
        self
    }

    pub fn h_percent(mut self, percent: f32) -> Self {
        self.inner = self.inner.h_percent(percent);
        self
    }

    pub fn h_full(mut self) -> Self {
        self.inner = self.inner.h_full();
        self
    }

    pub fn h_auto(mut self) -> Self {
        self.inner = self.inner.h_auto();
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
    id: ElementId,
    content: String,
}

impl Text {
    pub fn new(content: impl Into<String>) -> Self {
        let content = content.into();
        let id = ElementId::new();
        Self {
            inner: raw_text(content.clone()).id(id),
            id,
            content,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.inner = self.inner.id(id);
        self.id = id;
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
        Some(self.id)
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

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        if self.content.trim().is_empty() {
            return Ok(None);
        }

        let node = AccessibilityNode::new(self.id, AccessibilityRole::Text)
            .with_label(self.content.clone())
            .with_focused(cx.a11y_has_focus(self.id));
        Ok(Some(node))
    }
}

/// Pointer hover wrapper with enter, move, leave, and cursor intent.
pub struct Hoverable {
    id: ElementId,
    inner: AnyElement,
    hovered: bool,
    cursor: Option<Cursor>,
    on_enter: Option<Box<dyn Fn()>>,
    on_move: Option<Box<dyn Fn(Point)>>,
    on_leave: Option<Box<dyn Fn()>>,
}

impl Hoverable {
    pub fn new(child: impl Into<AnyElement>) -> Self {
        Self {
            id: ElementId::new(),
            inner: child.into(),
            hovered: false,
            cursor: None,
            on_enter: None,
            on_move: None,
            on_leave: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self
    }

    pub fn cursor(mut self, cursor: Cursor) -> Self {
        self.cursor = Some(cursor);
        self
    }

    pub fn on_enter(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_enter = Some(Box::new(handler));
        self
    }

    pub fn on_move(mut self, handler: impl Fn(Point) + 'static) -> Self {
        self.on_move = Some(Box::new(handler));
        self
    }

    pub fn on_leave(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_leave = Some(Box::new(handler));
        self
    }
}

impl Element for Hoverable {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        self.inner.style()
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.inner.layout(cx)
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        cx.register_hit_region(self.id, cx.bounds());
        self.inner.paint(cx);
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let inside = cx.bounds().contains(event.position);

        if matches!(event.kind, crate::elements::element::PointerEventKind::Move) {
            if inside {
                if let Some(cursor) = self.cursor {
                    cx.set_cursor(cursor);
                }
                if !self.hovered {
                    self.hovered = true;
                    if let Some(handler) = &self.on_enter {
                        handler();
                    }
                    cx.request_redraw();
                }
                if let Some(handler) = &self.on_move {
                    handler(event.position);
                }
            } else if self.hovered {
                self.hovered = false;
                if let Some(handler) = &self.on_leave {
                    handler();
                }
                cx.request_redraw();
            }
        }

        let hit_target = cx.hit_target();
        let previous_hit_target = cx.previous_hit_target();
        cx.set_hit_target(None);
        cx.set_previous_hit_target(None);
        let handled = self.inner.handle_pointer_event(cx, event);
        cx.set_hit_target(hit_target);
        cx.set_previous_hit_target(previous_hit_target);
        handled
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

    fn contains_id(&self, id: ElementId) -> bool {
        self.id == id || self.inner.contains_id(id)
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

pub fn hoverable(child: impl Into<AnyElement>) -> Hoverable {
    Hoverable::new(child)
}
