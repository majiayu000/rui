//! List element for rendering ordered and unordered lists

use crate::core::ElementId;
use crate::core::action::{ActionId, ActionOutcome};
use crate::core::color::Color;
use crate::core::geometry::Bounds;
use crate::core::style::{Display, FlexDirection, Style};
use crate::core::text_editing::{TextInputCommand, TextInputEvent};
use crate::elements::element::{
    AnyElement, Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    style_to_taffy,
};
use crate::elements::text::{FontWeight, TextAlign};
use crate::renderer::Primitive;
use crate::renderer::text::TextMeasureCache;
use smallvec::SmallVec;
use taffy::prelude::*;

/// Style of list markers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListStyle {
    /// Unordered list with bullet points
    #[default]
    Bullet,
    /// Ordered list with numbers (1, 2, 3, ...)
    Numbered,
    /// Ordered list with lowercase letters (a, b, c, ...)
    LowercaseAlpha,
    /// Ordered list with uppercase letters (A, B, C, ...)
    UppercaseAlpha,
    /// Ordered list with lowercase roman numerals (i, ii, iii, ...)
    LowercaseRoman,
    /// Ordered list with uppercase roman numerals (I, II, III, ...)
    UppercaseRoman,
    /// No marker
    None,
}

impl ListStyle {
    /// Generate the marker text for a given index (0-based)
    pub fn marker(&self, index: usize) -> String {
        match self {
            ListStyle::Bullet => "\u{2022}".to_string(), // bullet character
            ListStyle::Numbered => format!("{}.", index + 1),
            ListStyle::LowercaseAlpha => format!("{}.", Self::to_alpha(index, false)),
            ListStyle::UppercaseAlpha => format!("{}.", Self::to_alpha(index, true)),
            ListStyle::LowercaseRoman => format!("{}.", Self::to_roman(index + 1, false)),
            ListStyle::UppercaseRoman => format!("{}.", Self::to_roman(index + 1, true)),
            ListStyle::None => String::new(),
        }
    }

    /// Convert index to alphabetical marker (0 -> a, 1 -> b, ..., 25 -> z, 26 -> aa, ...)
    fn to_alpha(index: usize, uppercase: bool) -> String {
        let base = if uppercase { b'A' } else { b'a' };
        let mut result = String::new();
        let mut n = index;

        loop {
            result.insert(0, (base + (n % 26) as u8) as char);
            if n < 26 {
                break;
            }
            n = n / 26 - 1;
        }

        result
    }

    /// Convert number to roman numerals
    fn to_roman(mut num: usize, uppercase: bool) -> String {
        let numerals = if uppercase {
            [
                ("M", 1000),
                ("CM", 900),
                ("D", 500),
                ("CD", 400),
                ("C", 100),
                ("XC", 90),
                ("L", 50),
                ("XL", 40),
                ("X", 10),
                ("IX", 9),
                ("V", 5),
                ("IV", 4),
                ("I", 1),
            ]
        } else {
            [
                ("m", 1000),
                ("cm", 900),
                ("d", 500),
                ("cd", 400),
                ("c", 100),
                ("xc", 90),
                ("l", 50),
                ("xl", 40),
                ("x", 10),
                ("ix", 9),
                ("v", 5),
                ("iv", 4),
                ("i", 1),
            ]
        };

        let mut result = String::new();
        for (symbol, value) in numerals.iter() {
            while num >= *value {
                result.push_str(symbol);
                num -= value;
            }
        }
        result
    }
}

/// A single item in a list
pub struct ListItem {
    id: Option<ElementId>,
    content: AnyElement,
    style: Style,
    layout_node: Option<NodeId>,
    content_node: Option<NodeId>,
}

impl ListItem {
    /// Create a new list item with the given content
    pub fn new(content: impl Into<AnyElement>) -> Self {
        Self {
            id: None,
            content: content.into(),
            style: Style::new(),
            layout_node: None,
            content_node: None,
        }
    }

    /// Set the element ID
    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        self
    }
}

impl Element for ListItem {
    fn id(&self) -> Option<ElementId> {
        self.id
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        // Layout the content
        let content_node = self.content.layout(cx);

        // Create a container for the list item
        let mut item_style = style_to_taffy(&self.style);
        item_style.display = taffy::Display::Flex;
        item_style.flex_direction = taffy::FlexDirection::Row;
        item_style.align_items = Some(taffy::AlignItems::FlexStart);

        let node = cx
            .taffy
            .new_with_children(item_style, &[content_node])
            .expect("Failed to create list item layout node");

        self.layout_node = Some(node);
        self.content_node = Some(content_node);
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        if let Some(content_node) = self.content_node {
            let content_bounds = cx.child_bounds(content_node).unwrap_or(bounds);
            let mut child_cx = cx.with_bounds(content_bounds);
            self.content.paint(&mut child_cx);
        } else {
            self.content.paint(cx);
        }
    }

    fn refresh_text_geometry(&mut self, text_measurer: &mut TextMeasureCache) {
        self.content.refresh_text_geometry(text_measurer);
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        if let Some(content_node) = self.content_node {
            let content_bounds = cx.child_bounds(content_node).unwrap_or(cx.bounds());
            let mut child_cx = cx.with_bounds(content_bounds);
            self.content.handle_pointer_event(&mut child_cx, event)
        } else {
            self.content.handle_pointer_event(cx, event)
        }
    }

    fn handle_scroll_event(
        &mut self,
        cx: &mut EventContext,
        event: &crate::core::event::ScrollEvent,
    ) -> bool {
        if let Some(content_node) = self.content_node {
            let content_bounds = cx.child_bounds(content_node).unwrap_or(cx.bounds());
            let mut child_cx = cx.with_bounds(content_bounds);
            self.content.handle_scroll_event(&mut child_cx, event)
        } else {
            self.content.handle_scroll_event(cx, event)
        }
    }

    fn handle_key_event(
        &mut self,
        cx: &mut EventContext,
        event: &crate::core::event::KeyEvent,
    ) -> bool {
        self.content.handle_key_event(cx, event)
    }

    fn dispatch_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        if let Some(content_node) = self.content_node {
            let content_bounds = cx.child_bounds(content_node).unwrap_or(cx.bounds());
            let mut child_cx = cx.with_bounds(content_bounds);
            self.content.dispatch_action(&mut child_cx, action)
        } else {
            self.content.dispatch_action(cx, action)
        }
    }

    fn handle_text_input_event(&mut self, cx: &mut EventContext, event: &TextInputEvent) -> bool {
        if let Some(content_node) = self.content_node {
            let content_bounds = cx.child_bounds(content_node).unwrap_or(cx.bounds());
            let mut child_cx = cx.with_bounds(content_bounds);
            self.content.handle_text_input_event(&mut child_cx, event)
        } else {
            self.content.handle_text_input_event(cx, event)
        }
    }

    fn handle_text_input_command(
        &mut self,
        cx: &mut EventContext,
        command: &TextInputCommand,
    ) -> bool {
        if let Some(content_node) = self.content_node {
            let bounds = cx.child_bounds(content_node).unwrap_or(cx.bounds());
            self.content
                .handle_text_input_command(&mut cx.with_bounds(bounds), command)
        } else {
            self.content.handle_text_input_command(cx, command)
        }
    }

    fn handle_window_event(&mut self, event: &crate::core::event::Event) -> bool {
        self.content.handle_window_event(event)
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id == Some(id) || self.content.contains_id(id)
    }
}

/// A list element that renders ordered or unordered lists
pub struct List {
    id: Option<ElementId>,
    style: Style,
    list_style: ListStyle,
    items: SmallVec<[ListItem; 8]>,
    gap: f32,
    marker_color: Color,
    marker_font_size: f32,
    marker_width: f32,
    start_index: usize,
    layout_node: Option<NodeId>,
    child_nodes: SmallVec<[NodeId; 8]>,
}

impl List {
    /// Create a new list with default settings
    pub fn new() -> Self {
        Self {
            id: None,
            style: Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                ..Style::new()
            },
            list_style: ListStyle::default(),
            items: SmallVec::new(),
            gap: 8.0,
            marker_color: Color::BLACK,
            marker_font_size: 14.0,
            marker_width: 24.0,
            start_index: 0,
            layout_node: None,
            child_nodes: SmallVec::new(),
        }
    }

    /// Set the element ID
    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the list style (bullet, numbered, etc.)
    pub fn list_style(mut self, style: ListStyle) -> Self {
        self.list_style = style;
        self
    }

    /// Make this an ordered (numbered) list
    pub fn ordered(mut self) -> Self {
        self.list_style = ListStyle::Numbered;
        self
    }

    /// Make this an unordered (bullet) list
    pub fn unordered(mut self) -> Self {
        self.list_style = ListStyle::Bullet;
        self
    }

    /// Use lowercase alphabetical markers
    pub fn alpha(mut self) -> Self {
        self.list_style = ListStyle::LowercaseAlpha;
        self
    }

    /// Use uppercase alphabetical markers
    pub fn alpha_upper(mut self) -> Self {
        self.list_style = ListStyle::UppercaseAlpha;
        self
    }

    /// Use lowercase roman numeral markers
    pub fn roman(mut self) -> Self {
        self.list_style = ListStyle::LowercaseRoman;
        self
    }

    /// Use uppercase roman numeral markers
    pub fn roman_upper(mut self) -> Self {
        self.list_style = ListStyle::UppercaseRoman;
        self
    }

    /// Hide list markers
    pub fn no_marker(mut self) -> Self {
        self.list_style = ListStyle::None;
        self
    }

    /// Add a single item to the list
    pub fn item(mut self, item: impl Into<AnyElement>) -> Self {
        self.items.push(ListItem::new(item));
        self
    }

    /// Add multiple items to the list
    pub fn items<I, E>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<AnyElement>,
    {
        for item in items {
            self.items.push(ListItem::new(item));
        }
        self
    }

    /// Set the gap between list items
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// Set the marker color
    pub fn marker_color(mut self, color: impl Into<Color>) -> Self {
        self.marker_color = color.into();
        self
    }

    /// Set the marker font size
    pub fn marker_size(mut self, size: f32) -> Self {
        self.marker_font_size = size;
        self
    }

    /// Set the width reserved for markers
    pub fn marker_width(mut self, width: f32) -> Self {
        self.marker_width = width;
        self
    }

    /// Set the starting index for ordered lists (0-based internally, displayed as 1-based)
    pub fn start(mut self, index: usize) -> Self {
        self.start_index = index;
        self
    }

    /// Get the number of items in the list
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if the list is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get the current list style
    pub fn get_list_style(&self) -> ListStyle {
        self.list_style
    }
}

impl Default for List {
    fn default() -> Self {
        Self::new()
    }
}

impl Element for List {
    fn id(&self) -> Option<ElementId> {
        self.id
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        // Layout each item
        let child_nodes: Vec<NodeId> = self.items.iter_mut().map(|item| item.layout(cx)).collect();

        // Create the list container style
        let mut taffy_style = style_to_taffy(&self.style);
        taffy_style.display = taffy::Display::Flex;
        taffy_style.flex_direction = taffy::FlexDirection::Column;
        taffy_style.gap = taffy::Size {
            width: LengthPercentage::Length(0.0),
            height: LengthPercentage::Length(self.gap),
        };

        let node = cx
            .taffy
            .new_with_children(taffy_style, &child_nodes)
            .expect("Failed to create list layout node");

        self.layout_node = Some(node);
        self.child_nodes = SmallVec::from_vec(child_nodes);
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();

        for (index, (item, node)) in self
            .items
            .iter_mut()
            .zip(self.child_nodes.iter().copied())
            .enumerate()
        {
            let item_bounds = cx.child_bounds(node).unwrap_or(bounds);
            // Calculate the marker for this item
            let marker_text = self.list_style.marker(self.start_index + index);

            // Paint the marker if not empty
            if !marker_text.is_empty() {
                let marker_bounds = Bounds::from_xywh(
                    item_bounds.x(),
                    item_bounds.y(),
                    self.marker_width,
                    item_bounds.height().max(self.marker_font_size * 1.4),
                );

                cx.paint(Primitive::Text {
                    bounds: marker_bounds,
                    content: marker_text,
                    color: self.marker_color.to_rgba(),
                    font_size: self.marker_font_size,
                    font_weight: FontWeight::Regular.to_value(),
                    font_family: None,
                    line_height: 1.4,
                    align: TextAlign::Right,
                });
            }

            // Paint the item content with offset for the marker
            let content_bounds = Bounds::from_xywh(
                item_bounds.x() + self.marker_width + 4.0, // 4px gap between marker and content
                item_bounds.y(),
                (item_bounds.width() - self.marker_width - 4.0).max(0.0),
                item_bounds.height(),
            );

            let mut child_cx = cx.with_bounds(content_bounds);
            item.paint(&mut child_cx);
        }
    }

    fn refresh_text_geometry(&mut self, text_measurer: &mut TextMeasureCache) {
        for item in &mut self.items {
            item.refresh_text_geometry(text_measurer);
        }
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let is_move = matches!(event.kind, PointerEventKind::Move);
        let mut handled = false;

        for index in (0..self.items.len()).rev() {
            let Some(node) = self.child_nodes.get(index).copied() else {
                continue;
            };
            let item = &mut self.items[index];
            let item_bounds = cx.child_bounds(node).unwrap_or(cx.bounds());
            let mut item_cx = cx.with_bounds(item_bounds);
            let item_handled = item.handle_pointer_event(&mut item_cx, event);
            if !is_move && item_handled {
                handled = true;
                break;
            }
        }

        handled
    }

    fn handle_scroll_event(
        &mut self,
        cx: &mut EventContext,
        event: &crate::core::event::ScrollEvent,
    ) -> bool {
        let mut handled = false;
        for index in (0..self.items.len()).rev() {
            let Some(node) = self.child_nodes.get(index).copied() else {
                continue;
            };
            let item = &mut self.items[index];
            let item_bounds = cx.child_bounds(node).unwrap_or(cx.bounds());
            let mut item_cx = cx.with_bounds(item_bounds);
            if item.handle_scroll_event(&mut item_cx, event) {
                handled = true;
                break;
            }
        }
        handled
    }

    fn handle_key_event(
        &mut self,
        cx: &mut EventContext,
        event: &crate::core::event::KeyEvent,
    ) -> bool {
        if let Some(focused) = cx.focused_id() {
            for item in self.items.iter_mut().rev() {
                if item.id == Some(focused) {
                    return item.handle_key_event(cx, event);
                }
            }
        }

        for item in self.items.iter_mut().rev() {
            if item.handle_key_event(cx, event) {
                return true;
            }
        }

        false
    }

    fn dispatch_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        if let Some(focused) = cx.focused_id() {
            for item in self.items.iter_mut().rev() {
                if item.contains_id(focused) {
                    let outcome = item.dispatch_action(cx, action);
                    if outcome.is_handled() {
                        return outcome;
                    }
                }
            }
        }

        for item in self.items.iter_mut().rev() {
            let outcome = item.dispatch_action(cx, action);
            if outcome.is_handled() {
                return outcome;
            }
        }

        ActionOutcome::Ignored
    }

    fn handle_text_input_event(&mut self, cx: &mut EventContext, event: &TextInputEvent) -> bool {
        if let Some(focused) = cx.focused_id() {
            for item in self.items.iter_mut().rev() {
                if item.contains_id(focused) && item.handle_text_input_event(cx, event) {
                    return true;
                }
            }
        }

        for item in self.items.iter_mut().rev() {
            if item.handle_text_input_event(cx, event) {
                return true;
            }
        }

        false
    }

    fn handle_text_input_command(
        &mut self,
        cx: &mut EventContext,
        command: &TextInputCommand,
    ) -> bool {
        if let Some(focused) = cx.focused_id() {
            for item in self.items.iter_mut().rev() {
                if item.contains_id(focused) {
                    return item.handle_text_input_command(cx, command);
                }
            }
        }
        self.items
            .iter_mut()
            .rev()
            .any(|item| item.handle_text_input_command(cx, command))
    }

    fn handle_window_event(&mut self, event: &crate::core::event::Event) -> bool {
        for item in self.items.iter_mut().rev() {
            if item.handle_window_event(event) {
                return true;
            }
        }
        false
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id == Some(id) || self.items.iter().any(|item| item.contains_id(id))
    }
}

/// Create a new List element
pub fn list() -> List {
    List::new()
}

/// Create a new ordered (numbered) list
pub fn ordered_list() -> List {
    List::new().ordered()
}

/// Create a new unordered (bullet) list
pub fn unordered_list() -> List {
    List::new().unordered()
}

#[cfg(test)]
mod tests;
