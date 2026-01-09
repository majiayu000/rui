//! Div element - the primary container element

use crate::core::color::Color;
use crate::core::geometry::{Bounds, Edges, Point, Size};
use crate::core::style::{
    AlignItems, Background, BorderStyle, Corners, Display, FlexDirection, JustifyContent,
    Overflow, Position, Shadow, Style,
};
use crate::core::ElementId;
use crate::elements::element::{style_to_taffy, AnyElement, Element, LayoutContext, PaintContext};
use crate::renderer::Primitive;
use smallvec::SmallVec;
use taffy::prelude::*;

/// A flexible container element (like HTML div)
pub struct Div {
    id: Option<ElementId>,
    style: Style,
    children: SmallVec<[AnyElement; 4]>,
    on_click: Option<Box<dyn Fn()>>,
    on_hover: Option<Box<dyn Fn(bool)>>,
    layout_node: Option<NodeId>,
}

impl Div {
    pub fn new() -> Self {
        Self {
            id: None,
            style: Style::new(),
            children: SmallVec::new(),
            on_click: None,
            on_hover: None,
            layout_node: None,
        }
    }

    // Identity
    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        self
    }

    // Size
    pub fn size(mut self, size: impl Into<Size>) -> Self {
        let s = size.into();
        self.style.width = Some(s.width);
        self.style.height = Some(s.height);
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

    pub fn w_full(mut self) -> Self {
        self.style.width = Some(f32::INFINITY); // Will be constrained by parent
        self.style.flex_grow = 1.0;
        self
    }

    pub fn h_full(mut self) -> Self {
        self.style.height = Some(f32::INFINITY);
        self.style.flex_grow = 1.0;
        self
    }

    pub fn min_w(mut self, width: f32) -> Self {
        self.style.min_width = Some(width);
        self
    }

    pub fn min_h(mut self, height: f32) -> Self {
        self.style.min_height = Some(height);
        self
    }

    pub fn max_w(mut self, width: f32) -> Self {
        self.style.max_width = Some(width);
        self
    }

    pub fn max_h(mut self, height: f32) -> Self {
        self.style.max_height = Some(height);
        self
    }

    // Flex properties
    pub fn flex(mut self) -> Self {
        self.style.display = Display::Flex;
        self
    }

    pub fn flex_row(mut self) -> Self {
        self.style.display = Display::Flex;
        self.style.flex_direction = FlexDirection::Row;
        self
    }

    pub fn flex_col(mut self) -> Self {
        self.style.display = Display::Flex;
        self.style.flex_direction = FlexDirection::Column;
        self
    }

    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.style.flex_grow = grow;
        self
    }

    pub fn flex_shrink(mut self, shrink: f32) -> Self {
        self.style.flex_shrink = shrink;
        self
    }

    pub fn gap(mut self, gap: f32) -> Self {
        self.style.gap = gap;
        self
    }

    // Alignment
    pub fn justify_start(mut self) -> Self {
        self.style.justify_content = JustifyContent::FlexStart;
        self
    }

    pub fn justify_end(mut self) -> Self {
        self.style.justify_content = JustifyContent::FlexEnd;
        self
    }

    pub fn justify_center(mut self) -> Self {
        self.style.justify_content = JustifyContent::Center;
        self
    }

    pub fn justify_between(mut self) -> Self {
        self.style.justify_content = JustifyContent::SpaceBetween;
        self
    }

    pub fn justify_around(mut self) -> Self {
        self.style.justify_content = JustifyContent::SpaceAround;
        self
    }

    pub fn items_start(mut self) -> Self {
        self.style.align_items = AlignItems::FlexStart;
        self
    }

    pub fn items_end(mut self) -> Self {
        self.style.align_items = AlignItems::FlexEnd;
        self
    }

    pub fn items_center(mut self) -> Self {
        self.style.align_items = AlignItems::Center;
        self
    }

    pub fn items_stretch(mut self) -> Self {
        self.style.align_items = AlignItems::Stretch;
        self
    }

    // Spacing
    pub fn p(mut self, padding: f32) -> Self {
        self.style.padding = Edges::all(padding);
        self
    }

    pub fn px(mut self, padding: f32) -> Self {
        self.style.padding.left = padding;
        self.style.padding.right = padding;
        self
    }

    pub fn py(mut self, padding: f32) -> Self {
        self.style.padding.top = padding;
        self.style.padding.bottom = padding;
        self
    }

    pub fn pt(mut self, padding: f32) -> Self {
        self.style.padding.top = padding;
        self
    }

    pub fn pb(mut self, padding: f32) -> Self {
        self.style.padding.bottom = padding;
        self
    }

    pub fn pl(mut self, padding: f32) -> Self {
        self.style.padding.left = padding;
        self
    }

    pub fn pr(mut self, padding: f32) -> Self {
        self.style.padding.right = padding;
        self
    }

    pub fn m(mut self, margin: f32) -> Self {
        self.style.margin = Edges::all(margin);
        self
    }

    pub fn mx(mut self, margin: f32) -> Self {
        self.style.margin.left = margin;
        self.style.margin.right = margin;
        self
    }

    pub fn my(mut self, margin: f32) -> Self {
        self.style.margin.top = margin;
        self.style.margin.bottom = margin;
        self
    }

    // Background
    pub fn bg(mut self, color: impl Into<Color>) -> Self {
        self.style.background = Background::Solid(color.into());
        self
    }

    pub fn bg_gradient(mut self, start: impl Into<Color>, end: impl Into<Color>, angle: f32) -> Self {
        self.style.background = Background::linear_gradient(start, end, angle);
        self
    }

    // Border
    pub fn border(mut self, width: f32, color: impl Into<Color>) -> Self {
        self.style.border = BorderStyle::new(width, color.into());
        self
    }

    pub fn border_color(mut self, color: impl Into<Color>) -> Self {
        self.style.border.color = color.into();
        self
    }

    pub fn border_width(mut self, width: f32) -> Self {
        self.style.border.width = Edges::all(width);
        self
    }

    pub fn rounded(mut self, radius: f32) -> Self {
        self.style.border.radius = Corners::all(radius);
        self
    }

    pub fn rounded_t(mut self, radius: f32) -> Self {
        self.style.border.radius.top_left = radius;
        self.style.border.radius.top_right = radius;
        self
    }

    pub fn rounded_b(mut self, radius: f32) -> Self {
        self.style.border.radius.bottom_left = radius;
        self.style.border.radius.bottom_right = radius;
        self
    }

    pub fn rounded_full(mut self) -> Self {
        self.style.border.radius = Corners::all(9999.0);
        self
    }

    // Shadow
    pub fn shadow(mut self, shadow: Shadow) -> Self {
        self.style.shadow = Some(shadow);
        self
    }

    pub fn shadow_sm(mut self) -> Self {
        self.style.shadow = Some(Shadow::new(0.0, 1.0, 2.0, Color::rgba(0.0, 0.0, 0.0, 0.1)));
        self
    }

    pub fn shadow_md(mut self) -> Self {
        self.style.shadow = Some(Shadow::new(0.0, 4.0, 6.0, Color::rgba(0.0, 0.0, 0.0, 0.1)));
        self
    }

    pub fn shadow_lg(mut self) -> Self {
        self.style.shadow = Some(Shadow::new(0.0, 10.0, 15.0, Color::rgba(0.0, 0.0, 0.0, 0.1)));
        self
    }

    // Opacity
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.style.opacity = opacity;
        self
    }

    // Overflow
    pub fn overflow_hidden(mut self) -> Self {
        self.style.overflow_x = Overflow::Hidden;
        self.style.overflow_y = Overflow::Hidden;
        self
    }

    pub fn overflow_scroll(mut self) -> Self {
        self.style.overflow_x = Overflow::Scroll;
        self.style.overflow_y = Overflow::Scroll;
        self
    }

    // Position
    pub fn absolute(mut self) -> Self {
        self.style.position = Position::Absolute;
        self
    }

    pub fn relative(mut self) -> Self {
        self.style.position = Position::Relative;
        self
    }

    // Children
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

    // Events
    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    pub fn on_hover(mut self, handler: impl Fn(bool) + 'static) -> Self {
        self.on_hover = Some(Box::new(handler));
        self
    }
}

impl Default for Div {
    fn default() -> Self {
        Self::new()
    }
}

impl Element for Div {
    fn id(&self) -> Option<ElementId> {
        self.id
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        // Layout children first
        let child_nodes: Vec<NodeId> = self
            .children
            .iter_mut()
            .map(|child| child.layout(cx))
            .collect();

        // Create this node
        let taffy_style = style_to_taffy(&self.style);
        let node = cx
            .taffy
            .new_with_children(taffy_style, &child_nodes)
            .expect("Failed to create layout node");

        self.layout_node = Some(node);
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();

        // Paint shadow first (behind the element)
        if let Some(ref shadow) = self.style.shadow {
            let shadow_bounds = Bounds::from_xywh(
                bounds.x() + shadow.offset_x - shadow.spread_radius,
                bounds.y() + shadow.offset_y - shadow.spread_radius,
                bounds.width() + shadow.spread_radius * 2.0,
                bounds.height() + shadow.spread_radius * 2.0,
            );
            cx.paint(Primitive::Shadow {
                bounds: shadow_bounds,
                corner_radii: self.style.border.radius,
                blur_radius: shadow.blur_radius,
                color: shadow.color.to_rgba(),
            });
        }

        // Paint background
        match &self.style.background {
            Background::None => {}
            Background::Solid(color) => {
                cx.paint(Primitive::Quad {
                    bounds,
                    background: color.to_rgba(),
                    border_color: self.style.border.color.to_rgba(),
                    border_widths: self.style.border.width,
                    corner_radii: self.style.border.radius,
                });
            }
            Background::LinearGradient { start, end, angle } => {
                cx.paint(Primitive::LinearGradient {
                    bounds,
                    start: start.to_rgba(),
                    end: end.to_rgba(),
                    angle: *angle,
                    corner_radii: self.style.border.radius,
                });
            }
            Background::RadialGradient { inner, outer } => {
                cx.paint(Primitive::RadialGradient {
                    bounds,
                    inner: inner.to_rgba(),
                    outer: outer.to_rgba(),
                    corner_radii: self.style.border.radius,
                });
            }
        }

        // Paint children
        // Note: In a full implementation, we'd get child bounds from the layout tree
        for child in &mut self.children {
            // For now, just pass through the same bounds
            // A real implementation would compute child bounds from Taffy
            child.paint(cx);
        }
    }

    fn children(&self) -> &[AnyElement] {
        &self.children
    }
}

/// Create a new Div element
pub fn div() -> Div {
    Div::new()
}
