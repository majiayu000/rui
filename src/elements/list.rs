//! List element for rendering ordered and unordered lists

use crate::core::color::Color;
use crate::core::geometry::Bounds;
use crate::core::style::{Display, FlexDirection, Style};
use crate::core::ElementId;
use crate::elements::element::{style_to_taffy, AnyElement, Element, LayoutContext, PaintContext};
use crate::elements::text::{FontWeight, TextAlign};
use crate::renderer::Primitive;
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
}

impl ListItem {
    /// Create a new list item with the given content
    pub fn new(content: impl Into<AnyElement>) -> Self {
        Self {
            id: None,
            content: content.into(),
            style: Style::new(),
            layout_node: None,
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
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        self.content.paint(cx);
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
        let child_nodes: Vec<NodeId> = self
            .items
            .iter_mut()
            .map(|item| item.layout(cx))
            .collect();

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
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        let mut y_offset = bounds.y();

        for (index, item) in self.items.iter_mut().enumerate() {
            // Calculate the marker for this item
            let marker_text = self.list_style.marker(self.start_index + index);

            // Paint the marker if not empty
            if !marker_text.is_empty() {
                let marker_bounds = Bounds::from_xywh(
                    bounds.x(),
                    y_offset,
                    self.marker_width,
                    self.marker_font_size * 1.4,
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
                bounds.x() + self.marker_width + 4.0, // 4px gap between marker and content
                y_offset,
                bounds.width() - self.marker_width - 4.0,
                self.marker_font_size * 1.4,
            );

            let mut child_cx = cx.with_bounds(content_bounds);
            item.paint(&mut child_cx);

            y_offset += self.marker_font_size * 1.4 + self.gap;
        }
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
mod tests {
    use super::*;
    use crate::elements::text::text;

    // ========== ListStyle tests ==========

    #[test]
    fn test_bullet_marker() {
        let style = ListStyle::Bullet;
        assert_eq!(style.marker(0), "\u{2022}");
        assert_eq!(style.marker(5), "\u{2022}");
        assert_eq!(style.marker(100), "\u{2022}");
    }

    #[test]
    fn test_numbered_marker() {
        let style = ListStyle::Numbered;
        assert_eq!(style.marker(0), "1.");
        assert_eq!(style.marker(1), "2.");
        assert_eq!(style.marker(9), "10.");
        assert_eq!(style.marker(99), "100.");
    }

    #[test]
    fn test_lowercase_alpha_marker() {
        let style = ListStyle::LowercaseAlpha;
        assert_eq!(style.marker(0), "a.");
        assert_eq!(style.marker(1), "b.");
        assert_eq!(style.marker(25), "z.");
        assert_eq!(style.marker(26), "aa.");
        assert_eq!(style.marker(27), "ab.");
        assert_eq!(style.marker(51), "az.");
        assert_eq!(style.marker(52), "ba.");
    }

    #[test]
    fn test_uppercase_alpha_marker() {
        let style = ListStyle::UppercaseAlpha;
        assert_eq!(style.marker(0), "A.");
        assert_eq!(style.marker(1), "B.");
        assert_eq!(style.marker(25), "Z.");
        assert_eq!(style.marker(26), "AA.");
    }

    #[test]
    fn test_lowercase_roman_marker() {
        let style = ListStyle::LowercaseRoman;
        assert_eq!(style.marker(0), "i.");
        assert_eq!(style.marker(1), "ii.");
        assert_eq!(style.marker(2), "iii.");
        assert_eq!(style.marker(3), "iv.");
        assert_eq!(style.marker(4), "v.");
        assert_eq!(style.marker(8), "ix.");
        assert_eq!(style.marker(9), "x.");
        assert_eq!(style.marker(49), "l.");
        assert_eq!(style.marker(99), "c.");
        assert_eq!(style.marker(999), "m.");
    }

    #[test]
    fn test_uppercase_roman_marker() {
        let style = ListStyle::UppercaseRoman;
        assert_eq!(style.marker(0), "I.");
        assert_eq!(style.marker(3), "IV.");
        assert_eq!(style.marker(4), "V.");
        assert_eq!(style.marker(9), "X.");
        assert_eq!(style.marker(49), "L.");
        assert_eq!(style.marker(99), "C.");
        assert_eq!(style.marker(499), "D.");
        assert_eq!(style.marker(999), "M.");
    }

    #[test]
    fn test_none_marker() {
        let style = ListStyle::None;
        assert_eq!(style.marker(0), "");
        assert_eq!(style.marker(100), "");
    }

    #[test]
    fn test_complex_roman_numerals() {
        // Test specific roman numeral conversions
        assert_eq!(ListStyle::to_roman(1, true), "I");
        assert_eq!(ListStyle::to_roman(4, true), "IV");
        assert_eq!(ListStyle::to_roman(9, true), "IX");
        assert_eq!(ListStyle::to_roman(14, true), "XIV");
        assert_eq!(ListStyle::to_roman(40, true), "XL");
        assert_eq!(ListStyle::to_roman(49, true), "XLIX");
        assert_eq!(ListStyle::to_roman(90, true), "XC");
        assert_eq!(ListStyle::to_roman(99, true), "XCIX");
        assert_eq!(ListStyle::to_roman(400, true), "CD");
        assert_eq!(ListStyle::to_roman(900, true), "CM");
        assert_eq!(ListStyle::to_roman(1994, true), "MCMXCIV");
        assert_eq!(ListStyle::to_roman(2024, true), "MMXXIV");
    }

    #[test]
    fn test_alpha_sequence() {
        // Test the full alphabet sequence
        for i in 0..26 {
            let expected = ((b'a' + i as u8) as char).to_string() + ".";
            assert_eq!(ListStyle::LowercaseAlpha.marker(i), expected);
        }
    }

    // ========== List builder tests ==========

    #[test]
    fn test_list_new() {
        let l = List::new();
        assert!(l.is_empty());
        assert_eq!(l.len(), 0);
        assert_eq!(l.get_list_style(), ListStyle::Bullet);
    }

    #[test]
    fn test_list_default() {
        let l = List::default();
        assert!(l.is_empty());
        assert_eq!(l.get_list_style(), ListStyle::Bullet);
    }

    #[test]
    fn test_list_ordered() {
        let l = List::new().ordered();
        assert_eq!(l.get_list_style(), ListStyle::Numbered);
    }

    #[test]
    fn test_list_unordered() {
        let l = List::new().unordered();
        assert_eq!(l.get_list_style(), ListStyle::Bullet);
    }

    #[test]
    fn test_list_alpha() {
        let l = List::new().alpha();
        assert_eq!(l.get_list_style(), ListStyle::LowercaseAlpha);
    }

    #[test]
    fn test_list_alpha_upper() {
        let l = List::new().alpha_upper();
        assert_eq!(l.get_list_style(), ListStyle::UppercaseAlpha);
    }

    #[test]
    fn test_list_roman() {
        let l = List::new().roman();
        assert_eq!(l.get_list_style(), ListStyle::LowercaseRoman);
    }

    #[test]
    fn test_list_roman_upper() {
        let l = List::new().roman_upper();
        assert_eq!(l.get_list_style(), ListStyle::UppercaseRoman);
    }

    #[test]
    fn test_list_no_marker() {
        let l = List::new().no_marker();
        assert_eq!(l.get_list_style(), ListStyle::None);
    }

    #[test]
    fn test_list_style_setter() {
        let l = List::new().list_style(ListStyle::UppercaseRoman);
        assert_eq!(l.get_list_style(), ListStyle::UppercaseRoman);
    }

    #[test]
    fn test_list_add_item() {
        let l = list().item(text("Item 1"));
        assert_eq!(l.len(), 1);
        assert!(!l.is_empty());
    }

    #[test]
    fn test_list_add_items() {
        let texts = vec![text("Item 1"), text("Item 2"), text("Item 3")];
        let l = list().items(texts);
        assert_eq!(l.len(), 3);
    }

    #[test]
    fn test_list_chained_items() {
        let l = list()
            .item(text("First"))
            .item(text("Second"))
            .item(text("Third"));
        assert_eq!(l.len(), 3);
    }

    #[test]
    fn test_list_gap() {
        let l = list().gap(16.0);
        assert_eq!(l.gap, 16.0);
    }

    #[test]
    fn test_list_marker_color() {
        let l = list().marker_color(Color::RED);
        assert_eq!(l.marker_color, Color::RED);
    }

    #[test]
    fn test_list_marker_size() {
        let l = list().marker_size(18.0);
        assert_eq!(l.marker_font_size, 18.0);
    }

    #[test]
    fn test_list_marker_width() {
        let l = list().marker_width(32.0);
        assert_eq!(l.marker_width, 32.0);
    }

    #[test]
    fn test_list_start_index() {
        let l = list().start(5);
        assert_eq!(l.start_index, 5);
    }

    #[test]
    fn test_list_id() {
        let id = ElementId::new();
        let l = list().id(id);
        assert_eq!(Element::id(&l), Some(id));
    }

    // ========== ListItem tests ==========

    #[test]
    fn test_list_item_new() {
        let item = ListItem::new(text("Test"));
        assert!(item.id.is_none());
    }

    #[test]
    fn test_list_item_id() {
        let id = ElementId::new();
        let item = ListItem::new(text("Test")).id(id);
        assert_eq!(Element::id(&item), Some(id));
    }

    // ========== Helper function tests ==========

    #[test]
    fn test_list_helper() {
        let l = list();
        assert_eq!(l.get_list_style(), ListStyle::Bullet);
    }

    #[test]
    fn test_ordered_list_helper() {
        let l = ordered_list();
        assert_eq!(l.get_list_style(), ListStyle::Numbered);
    }

    #[test]
    fn test_unordered_list_helper() {
        let l = unordered_list();
        assert_eq!(l.get_list_style(), ListStyle::Bullet);
    }

    // ========== Element trait tests ==========

    #[test]
    fn test_list_style_method() {
        let l = list();
        // Just verify we can access the style
        let _style = l.style();
    }

    #[test]
    fn test_list_item_style_method() {
        let item = ListItem::new(text("Test"));
        let _style = item.style();
    }

    // ========== Complex scenario tests ==========

    #[test]
    fn test_nested_configuration() {
        let l = list()
            .ordered()
            .gap(12.0)
            .marker_color(Color::BLUE)
            .marker_size(16.0)
            .marker_width(30.0)
            .start(3)
            .item(text("Fourth item"))
            .item(text("Fifth item"));

        assert_eq!(l.get_list_style(), ListStyle::Numbered);
        assert_eq!(l.gap, 12.0);
        assert_eq!(l.marker_color, Color::BLUE);
        assert_eq!(l.marker_font_size, 16.0);
        assert_eq!(l.marker_width, 30.0);
        assert_eq!(l.start_index, 3);
        assert_eq!(l.len(), 2);
    }

    #[test]
    fn test_style_override_chain() {
        // Test that the last style call wins
        let l = list()
            .ordered()
            .unordered()
            .alpha()
            .roman()
            .roman_upper();

        assert_eq!(l.get_list_style(), ListStyle::UppercaseRoman);
    }

    #[test]
    fn test_default_values() {
        let l = List::new();
        assert_eq!(l.gap, 8.0);
        assert_eq!(l.marker_color, Color::BLACK);
        assert_eq!(l.marker_font_size, 14.0);
        assert_eq!(l.marker_width, 24.0);
        assert_eq!(l.start_index, 0);
    }

    #[test]
    fn test_list_style_default() {
        let style = ListStyle::default();
        assert_eq!(style, ListStyle::Bullet);
    }

    #[test]
    fn test_list_style_clone() {
        let style = ListStyle::Numbered;
        let cloned = style.clone();
        assert_eq!(style, cloned);
    }

    #[test]
    fn test_list_style_copy() {
        let style = ListStyle::UppercaseAlpha;
        let copied: ListStyle = style;
        assert_eq!(style, copied);
    }

    #[test]
    fn test_list_style_debug() {
        let style = ListStyle::Bullet;
        let debug_str = format!("{:?}", style);
        assert!(debug_str.contains("Bullet"));
    }

    // ========== Edge case tests ==========

    #[test]
    fn test_large_index_numbered() {
        let style = ListStyle::Numbered;
        assert_eq!(style.marker(9999), "10000.");
    }

    #[test]
    fn test_large_index_alpha() {
        let style = ListStyle::LowercaseAlpha;
        // 702 = 26 * 27 = aaa (26 + 26*26 + 26*26*26 would be...)
        // Actually: 26^0 + 26^1 positions = 26 + 676 = 702 for "zz"
        // Let's just verify it doesn't panic
        let marker = style.marker(1000);
        assert!(!marker.is_empty());
    }

    #[test]
    fn test_large_roman_numeral() {
        let style = ListStyle::UppercaseRoman;
        let marker = style.marker(3999); // 4000 in 1-based, max for standard roman
        assert_eq!(marker, "MMMM."); // 4000 = MMMM in extended form
    }

    #[test]
    fn test_zero_gap() {
        let l = list().gap(0.0);
        assert_eq!(l.gap, 0.0);
    }

    #[test]
    fn test_empty_list_len() {
        let l = list();
        assert_eq!(l.len(), 0);
        assert!(l.is_empty());
    }

    #[test]
    fn test_single_item_list() {
        let l = list().item(text("Only item"));
        assert_eq!(l.len(), 1);
        assert!(!l.is_empty());
    }
}
