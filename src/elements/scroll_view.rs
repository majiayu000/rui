//! ScrollView element for scrollable content

use crate::core::color::Color;
use crate::core::geometry::{Bounds, Edges, Point, Size};
use crate::core::style::{Corners, Overflow, Style};
use crate::core::ElementId;
use crate::elements::element::{style_to_taffy, AnyElement, Element, LayoutContext, PaintContext};
use crate::renderer::Primitive;
use smallvec::SmallVec;
use taffy::prelude::*;

/// Scroll direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollDirection {
    #[default]
    Vertical,
    Horizontal,
    Both,
}

/// Scrollbar visibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarVisibility {
    #[default]
    Auto,      // Show when content overflows
    Always,    // Always show
    Never,     // Never show (but still scrollable)
    Hover,     // Show on hover
}

/// Scroll state
#[derive(Debug, Clone, Default)]
pub struct ScrollState {
    pub offset_x: f32,
    pub offset_y: f32,
    pub content_size: Size,
    pub viewport_size: Size,
    pub is_scrolling: bool,
    pub scrollbar_hovered: bool,
    pub scrollbar_dragging: bool,
}

impl ScrollState {
    pub fn max_scroll_x(&self) -> f32 {
        (self.content_size.width - self.viewport_size.width).max(0.0)
    }

    pub fn max_scroll_y(&self) -> f32 {
        (self.content_size.height - self.viewport_size.height).max(0.0)
    }

    pub fn scroll_to(&mut self, x: f32, y: f32) {
        self.offset_x = x.clamp(0.0, self.max_scroll_x());
        self.offset_y = y.clamp(0.0, self.max_scroll_y());
    }

    pub fn scroll_by(&mut self, dx: f32, dy: f32) {
        self.scroll_to(self.offset_x + dx, self.offset_y + dy);
    }

    pub fn scroll_to_top(&mut self) {
        self.offset_y = 0.0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.offset_y = self.max_scroll_y();
    }

    pub fn can_scroll_up(&self) -> bool {
        self.offset_y > 0.0
    }

    pub fn can_scroll_down(&self) -> bool {
        self.offset_y < self.max_scroll_y()
    }

    pub fn can_scroll_left(&self) -> bool {
        self.offset_x > 0.0
    }

    pub fn can_scroll_right(&self) -> bool {
        self.offset_x < self.max_scroll_x()
    }

    /// Calculate scrollbar thumb position and size (0-1 range)
    pub fn scrollbar_y(&self) -> (f32, f32) {
        if self.content_size.height <= self.viewport_size.height {
            return (0.0, 1.0); // Full size, no scroll
        }
        let ratio = self.viewport_size.height / self.content_size.height;
        let thumb_size = ratio.clamp(0.1, 1.0);
        let scroll_ratio = self.offset_y / self.max_scroll_y();
        let thumb_pos = scroll_ratio * (1.0 - thumb_size);
        (thumb_pos, thumb_size)
    }

    pub fn scrollbar_x(&self) -> (f32, f32) {
        if self.content_size.width <= self.viewport_size.width {
            return (0.0, 1.0);
        }
        let ratio = self.viewport_size.width / self.content_size.width;
        let thumb_size = ratio.clamp(0.1, 1.0);
        let scroll_ratio = self.offset_x / self.max_scroll_x();
        let thumb_pos = scroll_ratio * (1.0 - thumb_size);
        (thumb_pos, thumb_size)
    }
}

/// ScrollView component
pub struct ScrollView {
    id: Option<ElementId>,
    direction: ScrollDirection,
    scrollbar_visibility: ScrollbarVisibility,
    style: Style,
    state: ScrollState,
    children: SmallVec<[AnyElement; 4]>,
    scrollbar_width: f32,
    on_scroll: Option<Box<dyn Fn(f32, f32)>>,
    layout_node: Option<NodeId>,
}

impl ScrollView {
    pub fn new() -> Self {
        let mut style = Style::new();
        style.overflow_x = Overflow::Hidden;
        style.overflow_y = Overflow::Scroll;

        Self {
            id: None,
            direction: ScrollDirection::default(),
            scrollbar_visibility: ScrollbarVisibility::default(),
            style,
            state: ScrollState::default(),
            children: SmallVec::new(),
            scrollbar_width: 8.0,
            on_scroll: None,
            layout_node: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn direction(mut self, direction: ScrollDirection) -> Self {
        self.direction = direction;
        match direction {
            ScrollDirection::Vertical => {
                self.style.overflow_x = Overflow::Hidden;
                self.style.overflow_y = Overflow::Scroll;
            }
            ScrollDirection::Horizontal => {
                self.style.overflow_x = Overflow::Scroll;
                self.style.overflow_y = Overflow::Hidden;
            }
            ScrollDirection::Both => {
                self.style.overflow_x = Overflow::Scroll;
                self.style.overflow_y = Overflow::Scroll;
            }
        }
        self
    }

    pub fn vertical(self) -> Self {
        self.direction(ScrollDirection::Vertical)
    }

    pub fn horizontal(self) -> Self {
        self.direction(ScrollDirection::Horizontal)
    }

    pub fn both(self) -> Self {
        self.direction(ScrollDirection::Both)
    }

    pub fn scrollbar(mut self, visibility: ScrollbarVisibility) -> Self {
        self.scrollbar_visibility = visibility;
        self
    }

    pub fn scrollbar_always(self) -> Self {
        self.scrollbar(ScrollbarVisibility::Always)
    }

    pub fn scrollbar_never(self) -> Self {
        self.scrollbar(ScrollbarVisibility::Never)
    }

    pub fn scrollbar_width(mut self, width: f32) -> Self {
        self.scrollbar_width = width;
        self
    }

    pub fn w(mut self, width: f32) -> Self {
        self.style.width = Some(width);
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        self.style.height = Some(height);
        self
    }

    pub fn size(mut self, size: impl Into<Size>) -> Self {
        let s = size.into();
        self.style.width = Some(s.width);
        self.style.height = Some(s.height);
        self
    }

    pub fn bg(mut self, color: impl Into<Color>) -> Self {
        self.style.background = crate::core::style::Background::Solid(color.into());
        self
    }

    pub fn child(mut self, child: impl Into<AnyElement>) -> Self {
        self.children.push(child.into());
        self
    }

    pub fn children<I, E>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<AnyElement>,
    {
        self.children.extend(children.into_iter().map(Into::into));
        self
    }

    pub fn on_scroll(mut self, handler: impl Fn(f32, f32) + 'static) -> Self {
        self.on_scroll = Some(Box::new(handler));
        self
    }

    fn should_show_scrollbar(&self) -> (bool, bool) {
        let show_y = match self.scrollbar_visibility {
            ScrollbarVisibility::Always => true,
            ScrollbarVisibility::Never => false,
            ScrollbarVisibility::Auto | ScrollbarVisibility::Hover => {
                self.state.content_size.height > self.state.viewport_size.height
            }
        };
        let show_x = match self.scrollbar_visibility {
            ScrollbarVisibility::Always => true,
            ScrollbarVisibility::Never => false,
            ScrollbarVisibility::Auto | ScrollbarVisibility::Hover => {
                self.state.content_size.width > self.state.viewport_size.width
            }
        };
        match self.direction {
            ScrollDirection::Vertical => (false, show_y),
            ScrollDirection::Horizontal => (show_x, false),
            ScrollDirection::Both => (show_x, show_y),
        }
    }
}

impl Default for ScrollView {
    fn default() -> Self {
        Self::new()
    }
}

impl Element for ScrollView {
    fn id(&self) -> Option<ElementId> {
        self.id
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let child_nodes: Vec<NodeId> = self
            .children
            .iter_mut()
            .map(|child| child.layout(cx))
            .collect();

        let style = style_to_taffy(&self.style);
        let node = cx
            .taffy
            .new_with_children(style, &child_nodes)
            .expect("Failed to create scroll view layout node");

        self.layout_node = Some(node);
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();

        // Update viewport size
        self.state.viewport_size = bounds.size;

        // Paint background if any
        if let crate::core::style::Background::Solid(color) = &self.style.background {
            cx.paint(Primitive::Quad {
                bounds,
                background: color.to_rgba(),
                border_color: crate::core::color::Rgba::TRANSPARENT,
                border_widths: Edges::ZERO,
                corner_radii: Corners::ZERO,
            });
        }

        // Push clipping mask
        cx.scene.push_layer(bounds);

        // Paint children with scroll offset
        for child in &mut self.children {
            let child_bounds = Bounds::from_xywh(
                bounds.x() - self.state.offset_x,
                bounds.y() - self.state.offset_y,
                bounds.width(),
                bounds.height(),
            );
            let mut child_cx = cx.with_bounds(child_bounds);
            child.paint(&mut child_cx);
        }

        // Pop clipping mask
        cx.scene.pop_layer();

        // Paint scrollbars
        let (show_x, show_y) = self.should_show_scrollbar();

        if show_y {
            let (thumb_pos, thumb_size) = self.state.scrollbar_y();
            let track_height = bounds.height() - if show_x { self.scrollbar_width } else { 0.0 };
            let track_x = bounds.max_x() - self.scrollbar_width;
            let track_y = bounds.y();

            // Track
            cx.paint(Primitive::Quad {
                bounds: Bounds::from_xywh(track_x, track_y, self.scrollbar_width, track_height),
                background: Color::hex(0x000000).with_alpha(0.05).to_rgba(),
                border_color: crate::core::color::Rgba::TRANSPARENT,
                border_widths: Edges::ZERO,
                corner_radii: Corners::all(self.scrollbar_width / 2.0),
            });

            // Thumb
            let thumb_y = track_y + thumb_pos * track_height;
            let thumb_height = thumb_size * track_height;
            let thumb_color = if self.state.scrollbar_dragging {
                Color::hex(0x000000).with_alpha(0.5)
            } else if self.state.scrollbar_hovered {
                Color::hex(0x000000).with_alpha(0.3)
            } else {
                Color::hex(0x000000).with_alpha(0.2)
            };

            cx.paint(Primitive::Quad {
                bounds: Bounds::from_xywh(track_x + 1.0, thumb_y, self.scrollbar_width - 2.0, thumb_height),
                background: thumb_color.to_rgba(),
                border_color: crate::core::color::Rgba::TRANSPARENT,
                border_widths: Edges::ZERO,
                corner_radii: Corners::all((self.scrollbar_width - 2.0) / 2.0),
            });
        }

        if show_x {
            let (thumb_pos, thumb_size) = self.state.scrollbar_x();
            let track_width = bounds.width() - if show_y { self.scrollbar_width } else { 0.0 };
            let track_x = bounds.x();
            let track_y = bounds.max_y() - self.scrollbar_width;

            // Track
            cx.paint(Primitive::Quad {
                bounds: Bounds::from_xywh(track_x, track_y, track_width, self.scrollbar_width),
                background: Color::hex(0x000000).with_alpha(0.05).to_rgba(),
                border_color: crate::core::color::Rgba::TRANSPARENT,
                border_widths: Edges::ZERO,
                corner_radii: Corners::all(self.scrollbar_width / 2.0),
            });

            // Thumb
            let thumb_x = track_x + thumb_pos * track_width;
            let thumb_width = thumb_size * track_width;
            cx.paint(Primitive::Quad {
                bounds: Bounds::from_xywh(thumb_x, track_y + 1.0, thumb_width, self.scrollbar_width - 2.0),
                background: Color::hex(0x000000).with_alpha(0.2).to_rgba(),
                border_color: crate::core::color::Rgba::TRANSPARENT,
                border_widths: Edges::ZERO,
                corner_radii: Corners::all((self.scrollbar_width - 2.0) / 2.0),
            });
        }
    }

    fn children(&self) -> &[AnyElement] {
        &self.children
    }
}

/// Create a new ScrollView
pub fn scroll_view() -> ScrollView {
    ScrollView::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::text::text;

    // ========== ScrollDirection enum tests ==========

    #[test]
    fn test_scroll_direction_default() {
        let direction = ScrollDirection::default();
        assert_eq!(direction, ScrollDirection::Vertical);
    }

    #[test]
    fn test_scroll_direction_equality() {
        assert_eq!(ScrollDirection::Vertical, ScrollDirection::Vertical);
        assert_eq!(ScrollDirection::Horizontal, ScrollDirection::Horizontal);
        assert_eq!(ScrollDirection::Both, ScrollDirection::Both);
        assert_ne!(ScrollDirection::Vertical, ScrollDirection::Horizontal);
        assert_ne!(ScrollDirection::Vertical, ScrollDirection::Both);
        assert_ne!(ScrollDirection::Horizontal, ScrollDirection::Both);
    }

    #[test]
    fn test_scroll_direction_clone() {
        let direction = ScrollDirection::Horizontal;
        let cloned = direction.clone();
        assert_eq!(direction, cloned);
    }

    #[test]
    fn test_scroll_direction_copy() {
        let direction = ScrollDirection::Both;
        let copied: ScrollDirection = direction;
        assert_eq!(direction, copied);
    }

    #[test]
    fn test_scroll_direction_debug() {
        let debug_v = format!("{:?}", ScrollDirection::Vertical);
        let debug_h = format!("{:?}", ScrollDirection::Horizontal);
        let debug_b = format!("{:?}", ScrollDirection::Both);
        assert!(debug_v.contains("Vertical"));
        assert!(debug_h.contains("Horizontal"));
        assert!(debug_b.contains("Both"));
    }

    // ========== ScrollbarVisibility enum tests ==========

    #[test]
    fn test_scrollbar_visibility_default() {
        let visibility = ScrollbarVisibility::default();
        assert_eq!(visibility, ScrollbarVisibility::Auto);
    }

    #[test]
    fn test_scrollbar_visibility_equality() {
        assert_eq!(ScrollbarVisibility::Auto, ScrollbarVisibility::Auto);
        assert_eq!(ScrollbarVisibility::Always, ScrollbarVisibility::Always);
        assert_eq!(ScrollbarVisibility::Never, ScrollbarVisibility::Never);
        assert_eq!(ScrollbarVisibility::Hover, ScrollbarVisibility::Hover);
        assert_ne!(ScrollbarVisibility::Auto, ScrollbarVisibility::Always);
        assert_ne!(ScrollbarVisibility::Never, ScrollbarVisibility::Hover);
    }

    #[test]
    fn test_scrollbar_visibility_clone() {
        let visibility = ScrollbarVisibility::Always;
        let cloned = visibility.clone();
        assert_eq!(visibility, cloned);
    }

    #[test]
    fn test_scrollbar_visibility_copy() {
        let visibility = ScrollbarVisibility::Never;
        let copied: ScrollbarVisibility = visibility;
        assert_eq!(visibility, copied);
    }

    #[test]
    fn test_scrollbar_visibility_debug() {
        let debug_auto = format!("{:?}", ScrollbarVisibility::Auto);
        let debug_always = format!("{:?}", ScrollbarVisibility::Always);
        let debug_never = format!("{:?}", ScrollbarVisibility::Never);
        let debug_hover = format!("{:?}", ScrollbarVisibility::Hover);
        assert!(debug_auto.contains("Auto"));
        assert!(debug_always.contains("Always"));
        assert!(debug_never.contains("Never"));
        assert!(debug_hover.contains("Hover"));
    }

    // ========== ScrollState tests ==========

    #[test]
    fn test_scroll_state_default() {
        let state = ScrollState::default();
        assert_eq!(state.offset_x, 0.0);
        assert_eq!(state.offset_y, 0.0);
        assert_eq!(state.content_size.width, 0.0);
        assert_eq!(state.content_size.height, 0.0);
        assert_eq!(state.viewport_size.width, 0.0);
        assert_eq!(state.viewport_size.height, 0.0);
        assert!(!state.is_scrolling);
        assert!(!state.scrollbar_hovered);
        assert!(!state.scrollbar_dragging);
    }

    #[test]
    fn test_scroll_state_clone() {
        let mut state = ScrollState::default();
        state.offset_x = 10.0;
        state.offset_y = 20.0;
        state.is_scrolling = true;
        let cloned = state.clone();
        assert_eq!(cloned.offset_x, 10.0);
        assert_eq!(cloned.offset_y, 20.0);
        assert!(cloned.is_scrolling);
    }

    #[test]
    fn test_scroll_state_debug() {
        let state = ScrollState::default();
        let debug_str = format!("{:?}", state);
        assert!(debug_str.contains("ScrollState"));
    }

    // ========== ScrollState::max_scroll tests ==========

    struct MaxScrollTestCase {
        name: &'static str,
        content_size: Size,
        viewport_size: Size,
        expected_max_x: f32,
        expected_max_y: f32,
    }

    #[test]
    fn test_max_scroll_table_driven() {
        let test_cases = vec![
            MaxScrollTestCase {
                name: "content smaller than viewport",
                content_size: Size::new(100.0, 100.0),
                viewport_size: Size::new(200.0, 200.0),
                expected_max_x: 0.0,
                expected_max_y: 0.0,
            },
            MaxScrollTestCase {
                name: "content larger than viewport",
                content_size: Size::new(500.0, 800.0),
                viewport_size: Size::new(200.0, 300.0),
                expected_max_x: 300.0,
                expected_max_y: 500.0,
            },
            MaxScrollTestCase {
                name: "content equals viewport",
                content_size: Size::new(200.0, 200.0),
                viewport_size: Size::new(200.0, 200.0),
                expected_max_x: 0.0,
                expected_max_y: 0.0,
            },
            MaxScrollTestCase {
                name: "zero viewport",
                content_size: Size::new(100.0, 100.0),
                viewport_size: Size::new(0.0, 0.0),
                expected_max_x: 100.0,
                expected_max_y: 100.0,
            },
            MaxScrollTestCase {
                name: "zero content",
                content_size: Size::new(0.0, 0.0),
                viewport_size: Size::new(100.0, 100.0),
                expected_max_x: 0.0,
                expected_max_y: 0.0,
            },
        ];

        for case in test_cases {
            let mut state = ScrollState::default();
            state.content_size = case.content_size;
            state.viewport_size = case.viewport_size;

            assert_eq!(
                state.max_scroll_x(),
                case.expected_max_x,
                "Test case '{}' failed for max_scroll_x",
                case.name
            );
            assert_eq!(
                state.max_scroll_y(),
                case.expected_max_y,
                "Test case '{}' failed for max_scroll_y",
                case.name
            );
        }
    }

    // ========== ScrollState::scroll_to tests ==========

    struct ScrollToTestCase {
        name: &'static str,
        content_size: Size,
        viewport_size: Size,
        target_x: f32,
        target_y: f32,
        expected_x: f32,
        expected_y: f32,
    }

    #[test]
    fn test_scroll_to_table_driven() {
        let test_cases = vec![
            ScrollToTestCase {
                name: "scroll within bounds",
                content_size: Size::new(500.0, 800.0),
                viewport_size: Size::new(200.0, 300.0),
                target_x: 100.0,
                target_y: 200.0,
                expected_x: 100.0,
                expected_y: 200.0,
            },
            ScrollToTestCase {
                name: "scroll beyond max clamped",
                content_size: Size::new(500.0, 800.0),
                viewport_size: Size::new(200.0, 300.0),
                target_x: 1000.0,
                target_y: 1000.0,
                expected_x: 300.0,
                expected_y: 500.0,
            },
            ScrollToTestCase {
                name: "negative scroll clamped to zero",
                content_size: Size::new(500.0, 800.0),
                viewport_size: Size::new(200.0, 300.0),
                target_x: -100.0,
                target_y: -200.0,
                expected_x: 0.0,
                expected_y: 0.0,
            },
            ScrollToTestCase {
                name: "scroll to exact max",
                content_size: Size::new(500.0, 800.0),
                viewport_size: Size::new(200.0, 300.0),
                target_x: 300.0,
                target_y: 500.0,
                expected_x: 300.0,
                expected_y: 500.0,
            },
            ScrollToTestCase {
                name: "scroll to zero",
                content_size: Size::new(500.0, 800.0),
                viewport_size: Size::new(200.0, 300.0),
                target_x: 0.0,
                target_y: 0.0,
                expected_x: 0.0,
                expected_y: 0.0,
            },
        ];

        for case in test_cases {
            let mut state = ScrollState::default();
            state.content_size = case.content_size;
            state.viewport_size = case.viewport_size;
            state.scroll_to(case.target_x, case.target_y);

            assert_eq!(
                state.offset_x, case.expected_x,
                "Test case '{}' failed for offset_x",
                case.name
            );
            assert_eq!(
                state.offset_y, case.expected_y,
                "Test case '{}' failed for offset_y",
                case.name
            );
        }
    }

    // ========== ScrollState::scroll_by tests ==========

    #[test]
    fn test_scroll_by() {
        let mut state = ScrollState::default();
        state.content_size = Size::new(500.0, 800.0);
        state.viewport_size = Size::new(200.0, 300.0);
        state.offset_x = 50.0;
        state.offset_y = 100.0;

        state.scroll_by(25.0, 50.0);
        assert_eq!(state.offset_x, 75.0);
        assert_eq!(state.offset_y, 150.0);
    }

    #[test]
    fn test_scroll_by_negative() {
        let mut state = ScrollState::default();
        state.content_size = Size::new(500.0, 800.0);
        state.viewport_size = Size::new(200.0, 300.0);
        state.offset_x = 50.0;
        state.offset_y = 100.0;

        state.scroll_by(-25.0, -50.0);
        assert_eq!(state.offset_x, 25.0);
        assert_eq!(state.offset_y, 50.0);
    }

    #[test]
    fn test_scroll_by_clamped() {
        let mut state = ScrollState::default();
        state.content_size = Size::new(500.0, 800.0);
        state.viewport_size = Size::new(200.0, 300.0);
        state.offset_x = 50.0;
        state.offset_y = 100.0;

        state.scroll_by(-100.0, -200.0); // Would go negative
        assert_eq!(state.offset_x, 0.0);
        assert_eq!(state.offset_y, 0.0);
    }

    // ========== ScrollState::scroll_to_top/bottom tests ==========

    #[test]
    fn test_scroll_to_top() {
        let mut state = ScrollState::default();
        state.content_size = Size::new(500.0, 800.0);
        state.viewport_size = Size::new(200.0, 300.0);
        state.offset_y = 250.0;

        state.scroll_to_top();
        assert_eq!(state.offset_y, 0.0);
    }

    #[test]
    fn test_scroll_to_bottom() {
        let mut state = ScrollState::default();
        state.content_size = Size::new(500.0, 800.0);
        state.viewport_size = Size::new(200.0, 300.0);
        state.offset_y = 0.0;

        state.scroll_to_bottom();
        assert_eq!(state.offset_y, 500.0);
    }

    #[test]
    fn test_scroll_to_bottom_content_smaller_than_viewport() {
        let mut state = ScrollState::default();
        state.content_size = Size::new(100.0, 100.0);
        state.viewport_size = Size::new(200.0, 300.0);

        state.scroll_to_bottom();
        assert_eq!(state.offset_y, 0.0); // max_scroll_y is 0
    }

    // ========== ScrollState::can_scroll tests ==========

    struct CanScrollTestCase {
        name: &'static str,
        content_size: Size,
        viewport_size: Size,
        offset_x: f32,
        offset_y: f32,
        can_up: bool,
        can_down: bool,
        can_left: bool,
        can_right: bool,
    }

    #[test]
    fn test_can_scroll_table_driven() {
        let test_cases = vec![
            CanScrollTestCase {
                name: "at top-left corner",
                content_size: Size::new(500.0, 800.0),
                viewport_size: Size::new(200.0, 300.0),
                offset_x: 0.0,
                offset_y: 0.0,
                can_up: false,
                can_down: true,
                can_left: false,
                can_right: true,
            },
            CanScrollTestCase {
                name: "at bottom-right corner",
                content_size: Size::new(500.0, 800.0),
                viewport_size: Size::new(200.0, 300.0),
                offset_x: 300.0,
                offset_y: 500.0,
                can_up: true,
                can_down: false,
                can_left: true,
                can_right: false,
            },
            CanScrollTestCase {
                name: "in middle",
                content_size: Size::new(500.0, 800.0),
                viewport_size: Size::new(200.0, 300.0),
                offset_x: 150.0,
                offset_y: 250.0,
                can_up: true,
                can_down: true,
                can_left: true,
                can_right: true,
            },
            CanScrollTestCase {
                name: "content fits viewport",
                content_size: Size::new(100.0, 100.0),
                viewport_size: Size::new(200.0, 300.0),
                offset_x: 0.0,
                offset_y: 0.0,
                can_up: false,
                can_down: false,
                can_left: false,
                can_right: false,
            },
        ];

        for case in test_cases {
            let mut state = ScrollState::default();
            state.content_size = case.content_size;
            state.viewport_size = case.viewport_size;
            state.offset_x = case.offset_x;
            state.offset_y = case.offset_y;

            assert_eq!(
                state.can_scroll_up(),
                case.can_up,
                "Test case '{}' failed for can_scroll_up",
                case.name
            );
            assert_eq!(
                state.can_scroll_down(),
                case.can_down,
                "Test case '{}' failed for can_scroll_down",
                case.name
            );
            assert_eq!(
                state.can_scroll_left(),
                case.can_left,
                "Test case '{}' failed for can_scroll_left",
                case.name
            );
            assert_eq!(
                state.can_scroll_right(),
                case.can_right,
                "Test case '{}' failed for can_scroll_right",
                case.name
            );
        }
    }

    // ========== ScrollState::scrollbar_y tests ==========

    struct ScrollbarYTestCase {
        name: &'static str,
        content_height: f32,
        viewport_height: f32,
        offset_y: f32,
        expected_pos: f32,
        expected_size: f32,
    }

    #[test]
    fn test_scrollbar_y_table_driven() {
        let test_cases = vec![
            ScrollbarYTestCase {
                name: "content fits viewport",
                content_height: 100.0,
                viewport_height: 200.0,
                offset_y: 0.0,
                expected_pos: 0.0,
                expected_size: 1.0,
            },
            ScrollbarYTestCase {
                name: "at top",
                content_height: 1000.0,
                viewport_height: 200.0,
                offset_y: 0.0,
                expected_pos: 0.0,
                expected_size: 0.2,
            },
            ScrollbarYTestCase {
                name: "at bottom",
                content_height: 1000.0,
                viewport_height: 200.0,
                offset_y: 800.0,
                expected_pos: 0.8,
                expected_size: 0.2,
            },
            ScrollbarYTestCase {
                name: "in middle",
                content_height: 1000.0,
                viewport_height: 200.0,
                offset_y: 400.0,
                expected_pos: 0.4,
                expected_size: 0.2,
            },
        ];

        for case in test_cases {
            let mut state = ScrollState::default();
            state.content_size = Size::new(100.0, case.content_height);
            state.viewport_size = Size::new(100.0, case.viewport_height);
            state.offset_y = case.offset_y;

            let (pos, size) = state.scrollbar_y();
            assert!(
                (pos - case.expected_pos).abs() < 0.001,
                "Test case '{}' failed for scrollbar_y pos: expected {}, got {}",
                case.name,
                case.expected_pos,
                pos
            );
            assert!(
                (size - case.expected_size).abs() < 0.001,
                "Test case '{}' failed for scrollbar_y size: expected {}, got {}",
                case.name,
                case.expected_size,
                size
            );
        }
    }

    // ========== ScrollState::scrollbar_x tests ==========

    #[test]
    fn test_scrollbar_x_content_fits() {
        let mut state = ScrollState::default();
        state.content_size = Size::new(100.0, 100.0);
        state.viewport_size = Size::new(200.0, 100.0);

        let (pos, size) = state.scrollbar_x();
        assert_eq!(pos, 0.0);
        assert_eq!(size, 1.0);
    }

    #[test]
    fn test_scrollbar_x_at_start() {
        let mut state = ScrollState::default();
        state.content_size = Size::new(1000.0, 100.0);
        state.viewport_size = Size::new(200.0, 100.0);
        state.offset_x = 0.0;

        let (pos, size) = state.scrollbar_x();
        assert_eq!(pos, 0.0);
        assert!((size - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_scrollbar_x_at_end() {
        let mut state = ScrollState::default();
        state.content_size = Size::new(1000.0, 100.0);
        state.viewport_size = Size::new(200.0, 100.0);
        state.offset_x = 800.0;

        let (pos, size) = state.scrollbar_x();
        assert!((pos - 0.8).abs() < 0.001);
        assert!((size - 0.2).abs() < 0.001);
    }

    #[test]
    fn test_scrollbar_thumb_size_minimum() {
        // When content is much larger than viewport, thumb size should be clamped to 0.1
        let mut state = ScrollState::default();
        state.content_size = Size::new(100.0, 100000.0); // Very large content
        state.viewport_size = Size::new(100.0, 100.0);

        let (_, size) = state.scrollbar_y();
        assert!((size - 0.1).abs() < 0.001); // Clamped to minimum 0.1
    }

    // ========== ScrollView builder tests ==========

    #[test]
    fn test_scroll_view_new() {
        let sv = ScrollView::new();
        assert!(sv.id.is_none());
        assert_eq!(sv.direction, ScrollDirection::Vertical);
        assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Auto);
        assert_eq!(sv.scrollbar_width, 8.0);
        assert!(sv.children.is_empty());
        assert!(sv.on_scroll.is_none());
    }

    #[test]
    fn test_scroll_view_default() {
        let sv = ScrollView::default();
        assert_eq!(sv.direction, ScrollDirection::Vertical);
        assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Auto);
    }

    #[test]
    fn test_scroll_view_id() {
        let id = ElementId::new();
        let sv = ScrollView::new().id(id);
        assert_eq!(sv.id, Some(id));
    }

    // ========== ScrollView direction tests ==========

    struct DirectionTestCase {
        name: &'static str,
        direction: ScrollDirection,
        expected_overflow_x: Overflow,
        expected_overflow_y: Overflow,
    }

    #[test]
    fn test_scroll_view_direction_table_driven() {
        let test_cases = vec![
            DirectionTestCase {
                name: "vertical",
                direction: ScrollDirection::Vertical,
                expected_overflow_x: Overflow::Hidden,
                expected_overflow_y: Overflow::Scroll,
            },
            DirectionTestCase {
                name: "horizontal",
                direction: ScrollDirection::Horizontal,
                expected_overflow_x: Overflow::Scroll,
                expected_overflow_y: Overflow::Hidden,
            },
            DirectionTestCase {
                name: "both",
                direction: ScrollDirection::Both,
                expected_overflow_x: Overflow::Scroll,
                expected_overflow_y: Overflow::Scroll,
            },
        ];

        for case in test_cases {
            let sv = ScrollView::new().direction(case.direction);
            assert_eq!(
                sv.direction, case.direction,
                "Test case '{}' failed for direction",
                case.name
            );
            assert_eq!(
                sv.style.overflow_x, case.expected_overflow_x,
                "Test case '{}' failed for overflow_x",
                case.name
            );
            assert_eq!(
                sv.style.overflow_y, case.expected_overflow_y,
                "Test case '{}' failed for overflow_y",
                case.name
            );
        }
    }

    #[test]
    fn test_scroll_view_vertical() {
        let sv = ScrollView::new().vertical();
        assert_eq!(sv.direction, ScrollDirection::Vertical);
        assert_eq!(sv.style.overflow_x, Overflow::Hidden);
        assert_eq!(sv.style.overflow_y, Overflow::Scroll);
    }

    #[test]
    fn test_scroll_view_horizontal() {
        let sv = ScrollView::new().horizontal();
        assert_eq!(sv.direction, ScrollDirection::Horizontal);
        assert_eq!(sv.style.overflow_x, Overflow::Scroll);
        assert_eq!(sv.style.overflow_y, Overflow::Hidden);
    }

    #[test]
    fn test_scroll_view_both() {
        let sv = ScrollView::new().both();
        assert_eq!(sv.direction, ScrollDirection::Both);
        assert_eq!(sv.style.overflow_x, Overflow::Scroll);
        assert_eq!(sv.style.overflow_y, Overflow::Scroll);
    }

    // ========== ScrollView scrollbar visibility tests ==========

    #[test]
    fn test_scroll_view_scrollbar() {
        let sv = ScrollView::new().scrollbar(ScrollbarVisibility::Hover);
        assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Hover);
    }

    #[test]
    fn test_scroll_view_scrollbar_always() {
        let sv = ScrollView::new().scrollbar_always();
        assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Always);
    }

    #[test]
    fn test_scroll_view_scrollbar_never() {
        let sv = ScrollView::new().scrollbar_never();
        assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Never);
    }

    #[test]
    fn test_scroll_view_scrollbar_width() {
        let sv = ScrollView::new().scrollbar_width(12.0);
        assert_eq!(sv.scrollbar_width, 12.0);
    }

    // ========== ScrollView size tests ==========

    #[test]
    fn test_scroll_view_w() {
        let sv = ScrollView::new().w(300.0);
        assert_eq!(sv.style.width, Some(300.0));
    }

    #[test]
    fn test_scroll_view_h() {
        let sv = ScrollView::new().h(400.0);
        assert_eq!(sv.style.height, Some(400.0));
    }

    #[test]
    fn test_scroll_view_size() {
        let sv = ScrollView::new().size(Size::new(300.0, 400.0));
        assert_eq!(sv.style.width, Some(300.0));
        assert_eq!(sv.style.height, Some(400.0));
    }

    #[test]
    fn test_scroll_view_size_from_tuple() {
        let sv = ScrollView::new().size((250.0, 350.0));
        assert_eq!(sv.style.width, Some(250.0));
        assert_eq!(sv.style.height, Some(350.0));
    }

    // ========== ScrollView background tests ==========

    #[test]
    fn test_scroll_view_bg() {
        let sv = ScrollView::new().bg(Color::RED);
        match sv.style.background {
            crate::core::style::Background::Solid(color) => {
                assert_eq!(color, Color::RED);
            }
            _ => panic!("Expected solid background"),
        }
    }

    #[test]
    fn test_scroll_view_bg_hex() {
        let sv = ScrollView::new().bg(Color::hex(0xFF00FF));
        match sv.style.background {
            crate::core::style::Background::Solid(_) => {}
            _ => panic!("Expected solid background"),
        }
    }

    // ========== ScrollView children tests ==========

    #[test]
    fn test_scroll_view_child() {
        let sv = ScrollView::new().child(text("Test"));
        assert_eq!(sv.children.len(), 1);
    }

    #[test]
    fn test_scroll_view_multiple_children() {
        let sv = ScrollView::new()
            .child(text("Child 1"))
            .child(text("Child 2"))
            .child(text("Child 3"));
        assert_eq!(sv.children.len(), 3);
    }

    #[test]
    fn test_scroll_view_children_method() {
        let texts = vec![text("A"), text("B"), text("C"), text("D")];
        let sv = ScrollView::new().children(texts);
        assert_eq!(sv.children.len(), 4);
    }

    #[test]
    fn test_scroll_view_children_trait() {
        let sv = ScrollView::new()
            .child(text("First"))
            .child(text("Second"));
        assert_eq!(Element::children(&sv).len(), 2);
    }

    // ========== ScrollView on_scroll handler tests ==========

    #[test]
    fn test_scroll_view_on_scroll() {
        let sv = ScrollView::new().on_scroll(|_x, _y| {});
        assert!(sv.on_scroll.is_some());
    }

    // ========== ScrollView should_show_scrollbar tests ==========

    struct ShowScrollbarTestCase {
        name: &'static str,
        direction: ScrollDirection,
        visibility: ScrollbarVisibility,
        content_size: Size,
        viewport_size: Size,
        expected_show_x: bool,
        expected_show_y: bool,
    }

    #[test]
    fn test_should_show_scrollbar_table_driven() {
        let test_cases = vec![
            ShowScrollbarTestCase {
                name: "vertical auto no overflow",
                direction: ScrollDirection::Vertical,
                visibility: ScrollbarVisibility::Auto,
                content_size: Size::new(100.0, 100.0),
                viewport_size: Size::new(200.0, 200.0),
                expected_show_x: false,
                expected_show_y: false,
            },
            ShowScrollbarTestCase {
                name: "vertical auto with overflow",
                direction: ScrollDirection::Vertical,
                visibility: ScrollbarVisibility::Auto,
                content_size: Size::new(100.0, 500.0),
                viewport_size: Size::new(200.0, 200.0),
                expected_show_x: false,
                expected_show_y: true,
            },
            ShowScrollbarTestCase {
                name: "horizontal auto with overflow",
                direction: ScrollDirection::Horizontal,
                visibility: ScrollbarVisibility::Auto,
                content_size: Size::new(500.0, 100.0),
                viewport_size: Size::new(200.0, 200.0),
                expected_show_x: true,
                expected_show_y: false,
            },
            ShowScrollbarTestCase {
                name: "both auto with overflow both",
                direction: ScrollDirection::Both,
                visibility: ScrollbarVisibility::Auto,
                content_size: Size::new(500.0, 500.0),
                viewport_size: Size::new(200.0, 200.0),
                expected_show_x: true,
                expected_show_y: true,
            },
            ShowScrollbarTestCase {
                name: "vertical always",
                direction: ScrollDirection::Vertical,
                visibility: ScrollbarVisibility::Always,
                content_size: Size::new(100.0, 100.0),
                viewport_size: Size::new(200.0, 200.0),
                expected_show_x: false,
                expected_show_y: true,
            },
            ShowScrollbarTestCase {
                name: "horizontal always",
                direction: ScrollDirection::Horizontal,
                visibility: ScrollbarVisibility::Always,
                content_size: Size::new(100.0, 100.0),
                viewport_size: Size::new(200.0, 200.0),
                expected_show_x: true,
                expected_show_y: false,
            },
            ShowScrollbarTestCase {
                name: "both always",
                direction: ScrollDirection::Both,
                visibility: ScrollbarVisibility::Always,
                content_size: Size::new(100.0, 100.0),
                viewport_size: Size::new(200.0, 200.0),
                expected_show_x: true,
                expected_show_y: true,
            },
            ShowScrollbarTestCase {
                name: "vertical never",
                direction: ScrollDirection::Vertical,
                visibility: ScrollbarVisibility::Never,
                content_size: Size::new(100.0, 500.0),
                viewport_size: Size::new(200.0, 200.0),
                expected_show_x: false,
                expected_show_y: false,
            },
            ShowScrollbarTestCase {
                name: "both never",
                direction: ScrollDirection::Both,
                visibility: ScrollbarVisibility::Never,
                content_size: Size::new(500.0, 500.0),
                viewport_size: Size::new(200.0, 200.0),
                expected_show_x: false,
                expected_show_y: false,
            },
            ShowScrollbarTestCase {
                name: "vertical hover with overflow",
                direction: ScrollDirection::Vertical,
                visibility: ScrollbarVisibility::Hover,
                content_size: Size::new(100.0, 500.0),
                viewport_size: Size::new(200.0, 200.0),
                expected_show_x: false,
                expected_show_y: true,
            },
        ];

        for case in test_cases {
            let mut sv = ScrollView::new()
                .direction(case.direction)
                .scrollbar(case.visibility);
            sv.state.content_size = case.content_size;
            sv.state.viewport_size = case.viewport_size;

            let (show_x, show_y) = sv.should_show_scrollbar();
            assert_eq!(
                show_x, case.expected_show_x,
                "Test case '{}' failed for show_x",
                case.name
            );
            assert_eq!(
                show_y, case.expected_show_y,
                "Test case '{}' failed for show_y",
                case.name
            );
        }
    }

    // ========== Element trait tests ==========

    #[test]
    fn test_scroll_view_element_id() {
        let id = ElementId::new();
        let sv = ScrollView::new().id(id);
        assert_eq!(Element::id(&sv), Some(id));
    }

    #[test]
    fn test_scroll_view_element_id_none() {
        let sv = ScrollView::new();
        assert_eq!(Element::id(&sv), None);
    }

    #[test]
    fn test_scroll_view_element_style() {
        let sv = ScrollView::new();
        let _style = Element::style(&sv);
        // Just verify we can access it
    }

    // ========== Helper function tests ==========

    #[test]
    fn test_scroll_view_helper() {
        let sv = scroll_view();
        assert_eq!(sv.direction, ScrollDirection::Vertical);
        assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Auto);
    }

    // ========== Chained builder tests ==========

    #[test]
    fn test_full_builder_chain() {
        let id = ElementId::new();
        let sv = ScrollView::new()
            .id(id)
            .direction(ScrollDirection::Both)
            .scrollbar(ScrollbarVisibility::Always)
            .scrollbar_width(10.0)
            .w(400.0)
            .h(600.0)
            .bg(Color::WHITE)
            .child(text("Content 1"))
            .child(text("Content 2"));

        assert_eq!(sv.id, Some(id));
        assert_eq!(sv.direction, ScrollDirection::Both);
        assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Always);
        assert_eq!(sv.scrollbar_width, 10.0);
        assert_eq!(sv.style.width, Some(400.0));
        assert_eq!(sv.style.height, Some(600.0));
        assert_eq!(sv.children.len(), 2);
    }

    #[test]
    fn test_builder_chain_with_helper() {
        let sv = scroll_view()
            .vertical()
            .scrollbar_never()
            .size((300.0, 400.0));

        assert_eq!(sv.direction, ScrollDirection::Vertical);
        assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Never);
        assert_eq!(sv.style.width, Some(300.0));
        assert_eq!(sv.style.height, Some(400.0));
    }

    #[test]
    fn test_direction_override() {
        let sv = ScrollView::new()
            .vertical()
            .horizontal()
            .both();

        assert_eq!(sv.direction, ScrollDirection::Both);
        assert_eq!(sv.style.overflow_x, Overflow::Scroll);
        assert_eq!(sv.style.overflow_y, Overflow::Scroll);
    }

    #[test]
    fn test_scrollbar_override() {
        let sv = ScrollView::new()
            .scrollbar_always()
            .scrollbar_never()
            .scrollbar(ScrollbarVisibility::Hover);

        assert_eq!(sv.scrollbar_visibility, ScrollbarVisibility::Hover);
    }

    // ========== Default values verification ==========

    #[test]
    fn test_default_scrollbar_width() {
        let sv = ScrollView::new();
        assert_eq!(sv.scrollbar_width, 8.0);
    }

    #[test]
    fn test_default_scroll_state() {
        let sv = ScrollView::new();
        assert_eq!(sv.state.offset_x, 0.0);
        assert_eq!(sv.state.offset_y, 0.0);
        assert!(!sv.state.is_scrolling);
        assert!(!sv.state.scrollbar_hovered);
        assert!(!sv.state.scrollbar_dragging);
    }

    #[test]
    fn test_default_overflow_settings() {
        let sv = ScrollView::new();
        // Default is Vertical direction
        assert_eq!(sv.style.overflow_x, Overflow::Hidden);
        assert_eq!(sv.style.overflow_y, Overflow::Scroll);
    }

    // ========== Edge cases ==========

    #[test]
    fn test_scroll_state_zero_content() {
        let mut state = ScrollState::default();
        state.content_size = Size::new(0.0, 0.0);
        state.viewport_size = Size::new(100.0, 100.0);

        assert_eq!(state.max_scroll_x(), 0.0);
        assert_eq!(state.max_scroll_y(), 0.0);
        assert!(!state.can_scroll_up());
        assert!(!state.can_scroll_down());
        assert!(!state.can_scroll_left());
        assert!(!state.can_scroll_right());
    }

    #[test]
    fn test_scroll_state_zero_viewport() {
        let mut state = ScrollState::default();
        state.content_size = Size::new(100.0, 100.0);
        state.viewport_size = Size::new(0.0, 0.0);

        assert_eq!(state.max_scroll_x(), 100.0);
        assert_eq!(state.max_scroll_y(), 100.0);
    }

    #[test]
    fn test_scroll_state_equal_sizes() {
        let mut state = ScrollState::default();
        state.content_size = Size::new(200.0, 300.0);
        state.viewport_size = Size::new(200.0, 300.0);

        assert_eq!(state.max_scroll_x(), 0.0);
        assert_eq!(state.max_scroll_y(), 0.0);
        assert!(!state.can_scroll_up());
        assert!(!state.can_scroll_down());
        assert!(!state.can_scroll_left());
        assert!(!state.can_scroll_right());
    }

    #[test]
    fn test_empty_scroll_view() {
        let sv = ScrollView::new();
        assert!(sv.children.is_empty());
        assert_eq!(Element::children(&sv).len(), 0);
    }

    #[test]
    fn test_scrollbar_width_zero() {
        let sv = ScrollView::new().scrollbar_width(0.0);
        assert_eq!(sv.scrollbar_width, 0.0);
    }

    #[test]
    fn test_scrollbar_width_large() {
        let sv = ScrollView::new().scrollbar_width(50.0);
        assert_eq!(sv.scrollbar_width, 50.0);
    }

    // ========== Negative value edge cases ==========

    #[test]
    fn test_scroll_to_with_negative_max() {
        let mut state = ScrollState::default();
        state.content_size = Size::new(50.0, 50.0);
        state.viewport_size = Size::new(100.0, 100.0);

        // max_scroll should be 0, not negative
        assert_eq!(state.max_scroll_x(), 0.0);
        assert_eq!(state.max_scroll_y(), 0.0);

        // scroll_to should clamp to 0
        state.scroll_to(100.0, 100.0);
        assert_eq!(state.offset_x, 0.0);
        assert_eq!(state.offset_y, 0.0);
    }

    // ========== Complex workflow tests ==========

    #[test]
    fn test_scroll_state_workflow() {
        let mut state = ScrollState::default();
        state.content_size = Size::new(500.0, 1000.0);
        state.viewport_size = Size::new(200.0, 300.0);

        // Start at top
        assert!(!state.can_scroll_up());
        assert!(state.can_scroll_down());

        // Scroll down
        state.scroll_by(0.0, 100.0);
        assert!(state.can_scroll_up());
        assert!(state.can_scroll_down());

        // Scroll to bottom
        state.scroll_to_bottom();
        assert!(state.can_scroll_up());
        assert!(!state.can_scroll_down());

        // Scroll to top
        state.scroll_to_top();
        assert!(!state.can_scroll_up());
        assert!(state.can_scroll_down());
        assert_eq!(state.offset_y, 0.0);
    }

    #[test]
    fn test_scrollbar_position_workflow() {
        let mut state = ScrollState::default();
        state.content_size = Size::new(200.0, 1000.0);
        state.viewport_size = Size::new(200.0, 200.0);

        // At top: thumb position should be 0
        let (pos, _) = state.scrollbar_y();
        assert_eq!(pos, 0.0);

        // Scroll to middle
        state.scroll_to(0.0, 400.0);
        let (pos, _) = state.scrollbar_y();
        assert!(pos > 0.0 && pos < 1.0);

        // Scroll to bottom
        state.scroll_to_bottom();
        let (pos, size) = state.scrollbar_y();
        // At bottom, pos + size should equal 1.0
        assert!((pos + size - 1.0).abs() < 0.001);
    }
}
