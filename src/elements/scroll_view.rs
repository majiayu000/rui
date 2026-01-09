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
