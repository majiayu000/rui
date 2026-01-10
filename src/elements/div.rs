//! Div element - the primary container element

use crate::core::color::Color;
use crate::core::geometry::{Bounds, Edges, Size};
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::style::{AlignItems, Display, FlexDirection, JustifyContent, Overflow, Position};
    use crate::elements::element::Element;

    // ==================== Div::new() and Default ====================

    #[test]
    fn test_div_new() {
        let d = Div::new();
        assert!(d.id.is_none());
        assert!(d.children.is_empty());
        assert!(d.on_click.is_none());
        assert!(d.on_hover.is_none());
        assert!(d.layout_node.is_none());
    }

    #[test]
    fn test_div_default() {
        let d = Div::default();
        assert!(d.id.is_none());
        assert!(d.children.is_empty());
    }

    #[test]
    fn test_div_function() {
        let d = div();
        assert!(d.id.is_none());
        assert!(d.children.is_empty());
    }

    // ==================== Identity ====================

    #[test]
    fn test_div_id() {
        let d = Div::new().id(ElementId::from(123u64));
        assert_eq!(d.id, Some(ElementId::from(123u64)));
    }

    #[test]
    fn test_div_element_trait_id() {
        let d = Div::new().id(ElementId::from(456u64));
        assert_eq!(Element::id(&d), Some(ElementId::from(456u64)));
    }

    #[test]
    fn test_div_element_trait_id_none() {
        let d = Div::new();
        assert_eq!(Element::id(&d), None);
    }

    // ==================== Size Methods ====================

    struct SizeTestCase {
        name: &'static str,
        width: Option<f32>,
        height: Option<f32>,
    }

    #[test]
    fn test_div_size_methods() {
        let test_cases = [
            SizeTestCase { name: "w", width: Some(100.0), height: None },
            SizeTestCase { name: "h", width: None, height: Some(50.0) },
        ];

        for tc in test_cases {
            let mut d = Div::new();
            if let Some(w) = tc.width {
                d = d.w(w);
            }
            if let Some(h) = tc.height {
                d = d.h(h);
            }
            assert_eq!(d.style.width, tc.width, "failed for case: {}", tc.name);
            assert_eq!(d.style.height, tc.height, "failed for case: {}", tc.name);
        }
    }

    #[test]
    fn test_div_size() {
        let d = Div::new().size((200.0, 100.0));
        assert_eq!(d.style.width, Some(200.0));
        assert_eq!(d.style.height, Some(100.0));
    }

    #[test]
    fn test_div_size_from_size_struct() {
        let d = Div::new().size(Size::new(150.0, 75.0));
        assert_eq!(d.style.width, Some(150.0));
        assert_eq!(d.style.height, Some(75.0));
    }

    #[test]
    fn test_div_w() {
        let d = Div::new().w(250.0);
        assert_eq!(d.style.width, Some(250.0));
        assert_eq!(d.style.height, None);
    }

    #[test]
    fn test_div_h() {
        let d = Div::new().h(125.0);
        assert_eq!(d.style.width, None);
        assert_eq!(d.style.height, Some(125.0));
    }

    #[test]
    fn test_div_w_full() {
        let d = Div::new().w_full();
        assert_eq!(d.style.width, Some(f32::INFINITY));
        assert_eq!(d.style.flex_grow, 1.0);
    }

    #[test]
    fn test_div_h_full() {
        let d = Div::new().h_full();
        assert_eq!(d.style.height, Some(f32::INFINITY));
        assert_eq!(d.style.flex_grow, 1.0);
    }

    #[test]
    fn test_div_min_w() {
        let d = Div::new().min_w(50.0);
        assert_eq!(d.style.min_width, Some(50.0));
    }

    #[test]
    fn test_div_min_h() {
        let d = Div::new().min_h(30.0);
        assert_eq!(d.style.min_height, Some(30.0));
    }

    #[test]
    fn test_div_max_w() {
        let d = Div::new().max_w(800.0);
        assert_eq!(d.style.max_width, Some(800.0));
    }

    #[test]
    fn test_div_max_h() {
        let d = Div::new().max_h(600.0);
        assert_eq!(d.style.max_height, Some(600.0));
    }

    // ==================== Flex Properties ====================

    #[test]
    fn test_div_flex() {
        let d = Div::new().flex();
        assert_eq!(d.style.display, Display::Flex);
    }

    #[test]
    fn test_div_flex_row() {
        let d = Div::new().flex_row();
        assert_eq!(d.style.display, Display::Flex);
        assert_eq!(d.style.flex_direction, FlexDirection::Row);
    }

    #[test]
    fn test_div_flex_col() {
        let d = Div::new().flex_col();
        assert_eq!(d.style.display, Display::Flex);
        assert_eq!(d.style.flex_direction, FlexDirection::Column);
    }

    #[test]
    fn test_div_flex_grow() {
        let d = Div::new().flex_grow(2.5);
        assert_eq!(d.style.flex_grow, 2.5);
    }

    #[test]
    fn test_div_flex_shrink() {
        let d = Div::new().flex_shrink(0.5);
        assert_eq!(d.style.flex_shrink, 0.5);
    }

    #[test]
    fn test_div_gap() {
        let d = Div::new().gap(16.0);
        assert_eq!(d.style.gap, 16.0);
    }

    // ==================== Justify Content ====================

    struct JustifyContentTestCase {
        name: &'static str,
        expected: JustifyContent,
    }

    #[test]
    fn test_div_justify_content_methods() {
        let test_cases = [
            JustifyContentTestCase { name: "justify_start", expected: JustifyContent::FlexStart },
            JustifyContentTestCase { name: "justify_end", expected: JustifyContent::FlexEnd },
            JustifyContentTestCase { name: "justify_center", expected: JustifyContent::Center },
            JustifyContentTestCase { name: "justify_between", expected: JustifyContent::SpaceBetween },
            JustifyContentTestCase { name: "justify_around", expected: JustifyContent::SpaceAround },
        ];

        for tc in test_cases {
            let d = match tc.name {
                "justify_start" => Div::new().justify_start(),
                "justify_end" => Div::new().justify_end(),
                "justify_center" => Div::new().justify_center(),
                "justify_between" => Div::new().justify_between(),
                "justify_around" => Div::new().justify_around(),
                _ => unreachable!(),
            };
            assert_eq!(d.style.justify_content, tc.expected, "failed for case: {}", tc.name);
        }
    }

    #[test]
    fn test_div_justify_start() {
        let d = Div::new().justify_start();
        assert_eq!(d.style.justify_content, JustifyContent::FlexStart);
    }

    #[test]
    fn test_div_justify_end() {
        let d = Div::new().justify_end();
        assert_eq!(d.style.justify_content, JustifyContent::FlexEnd);
    }

    #[test]
    fn test_div_justify_center() {
        let d = Div::new().justify_center();
        assert_eq!(d.style.justify_content, JustifyContent::Center);
    }

    #[test]
    fn test_div_justify_between() {
        let d = Div::new().justify_between();
        assert_eq!(d.style.justify_content, JustifyContent::SpaceBetween);
    }

    #[test]
    fn test_div_justify_around() {
        let d = Div::new().justify_around();
        assert_eq!(d.style.justify_content, JustifyContent::SpaceAround);
    }

    // ==================== Align Items ====================

    struct AlignItemsTestCase {
        name: &'static str,
        expected: AlignItems,
    }

    #[test]
    fn test_div_align_items_methods() {
        let test_cases = [
            AlignItemsTestCase { name: "items_start", expected: AlignItems::FlexStart },
            AlignItemsTestCase { name: "items_end", expected: AlignItems::FlexEnd },
            AlignItemsTestCase { name: "items_center", expected: AlignItems::Center },
            AlignItemsTestCase { name: "items_stretch", expected: AlignItems::Stretch },
        ];

        for tc in test_cases {
            let d = match tc.name {
                "items_start" => Div::new().items_start(),
                "items_end" => Div::new().items_end(),
                "items_center" => Div::new().items_center(),
                "items_stretch" => Div::new().items_stretch(),
                _ => unreachable!(),
            };
            assert_eq!(d.style.align_items, tc.expected, "failed for case: {}", tc.name);
        }
    }

    #[test]
    fn test_div_items_start() {
        let d = Div::new().items_start();
        assert_eq!(d.style.align_items, AlignItems::FlexStart);
    }

    #[test]
    fn test_div_items_end() {
        let d = Div::new().items_end();
        assert_eq!(d.style.align_items, AlignItems::FlexEnd);
    }

    #[test]
    fn test_div_items_center() {
        let d = Div::new().items_center();
        assert_eq!(d.style.align_items, AlignItems::Center);
    }

    #[test]
    fn test_div_items_stretch() {
        let d = Div::new().items_stretch();
        assert_eq!(d.style.align_items, AlignItems::Stretch);
    }

    // ==================== Padding ====================

    #[test]
    fn test_div_p() {
        let d = Div::new().p(20.0);
        assert_eq!(d.style.padding, Edges::all(20.0));
    }

    #[test]
    fn test_div_px() {
        let d = Div::new().px(10.0);
        assert_eq!(d.style.padding.left, 10.0);
        assert_eq!(d.style.padding.right, 10.0);
        assert_eq!(d.style.padding.top, 0.0);
        assert_eq!(d.style.padding.bottom, 0.0);
    }

    #[test]
    fn test_div_py() {
        let d = Div::new().py(15.0);
        assert_eq!(d.style.padding.top, 15.0);
        assert_eq!(d.style.padding.bottom, 15.0);
        assert_eq!(d.style.padding.left, 0.0);
        assert_eq!(d.style.padding.right, 0.0);
    }

    #[test]
    fn test_div_pt() {
        let d = Div::new().pt(5.0);
        assert_eq!(d.style.padding.top, 5.0);
    }

    #[test]
    fn test_div_pb() {
        let d = Div::new().pb(8.0);
        assert_eq!(d.style.padding.bottom, 8.0);
    }

    #[test]
    fn test_div_pl() {
        let d = Div::new().pl(12.0);
        assert_eq!(d.style.padding.left, 12.0);
    }

    #[test]
    fn test_div_pr() {
        let d = Div::new().pr(7.0);
        assert_eq!(d.style.padding.right, 7.0);
    }

    #[test]
    fn test_div_padding_combined() {
        let d = Div::new().p(10.0).pt(20.0).pr(5.0);
        assert_eq!(d.style.padding.top, 20.0);
        assert_eq!(d.style.padding.right, 5.0);
        assert_eq!(d.style.padding.bottom, 10.0);
        assert_eq!(d.style.padding.left, 10.0);
    }

    // ==================== Margin ====================

    #[test]
    fn test_div_m() {
        let d = Div::new().m(16.0);
        assert_eq!(d.style.margin, Edges::all(16.0));
    }

    #[test]
    fn test_div_mx() {
        let d = Div::new().mx(24.0);
        assert_eq!(d.style.margin.left, 24.0);
        assert_eq!(d.style.margin.right, 24.0);
        assert_eq!(d.style.margin.top, 0.0);
        assert_eq!(d.style.margin.bottom, 0.0);
    }

    #[test]
    fn test_div_my() {
        let d = Div::new().my(32.0);
        assert_eq!(d.style.margin.top, 32.0);
        assert_eq!(d.style.margin.bottom, 32.0);
        assert_eq!(d.style.margin.left, 0.0);
        assert_eq!(d.style.margin.right, 0.0);
    }

    // ==================== Background ====================

    #[test]
    fn test_div_bg_color() {
        let d = Div::new().bg(Color::RED);
        match d.style.background {
            Background::Solid(color) => assert_eq!(color, Color::RED),
            _ => panic!("Expected solid background"),
        }
    }

    #[test]
    fn test_div_bg_hex() {
        let d = Div::new().bg(Color::hex(0xFF00FF));
        match d.style.background {
            Background::Solid(_) => {}
            _ => panic!("Expected solid background"),
        }
    }

    #[test]
    fn test_div_bg_gradient() {
        let d = Div::new().bg_gradient(Color::RED, Color::BLUE, 45.0);
        match d.style.background {
            Background::LinearGradient { start, end, angle } => {
                assert_eq!(start, Color::RED);
                assert_eq!(end, Color::BLUE);
                assert_eq!(angle, 45.0);
            }
            _ => panic!("Expected linear gradient background"),
        }
    }

    // ==================== Border ====================

    #[test]
    fn test_div_border() {
        let d = Div::new().border(2.0, Color::BLACK);
        assert_eq!(d.style.border.width, Edges::all(2.0));
        assert_eq!(d.style.border.color, Color::BLACK);
    }

    #[test]
    fn test_div_border_color() {
        let d = Div::new().border_color(Color::GREEN);
        assert_eq!(d.style.border.color, Color::GREEN);
    }

    #[test]
    fn test_div_border_width() {
        let d = Div::new().border_width(3.0);
        assert_eq!(d.style.border.width, Edges::all(3.0));
    }

    #[test]
    fn test_div_rounded() {
        let d = Div::new().rounded(8.0);
        assert_eq!(d.style.border.radius, Corners::all(8.0));
    }

    #[test]
    fn test_div_rounded_t() {
        let d = Div::new().rounded_t(12.0);
        assert_eq!(d.style.border.radius.top_left, 12.0);
        assert_eq!(d.style.border.radius.top_right, 12.0);
        assert_eq!(d.style.border.radius.bottom_left, 0.0);
        assert_eq!(d.style.border.radius.bottom_right, 0.0);
    }

    #[test]
    fn test_div_rounded_b() {
        let d = Div::new().rounded_b(10.0);
        assert_eq!(d.style.border.radius.bottom_left, 10.0);
        assert_eq!(d.style.border.radius.bottom_right, 10.0);
        assert_eq!(d.style.border.radius.top_left, 0.0);
        assert_eq!(d.style.border.radius.top_right, 0.0);
    }

    #[test]
    fn test_div_rounded_full() {
        let d = Div::new().rounded_full();
        assert_eq!(d.style.border.radius, Corners::all(9999.0));
    }

    // ==================== Shadow ====================

    #[test]
    fn test_div_shadow() {
        let shadow = Shadow::new(2.0, 4.0, 8.0, Color::rgba(0.0, 0.0, 0.0, 0.2));
        let d = Div::new().shadow(shadow);
        assert!(d.style.shadow.is_some());
        let s = d.style.shadow.unwrap();
        assert_eq!(s.offset_x, 2.0);
        assert_eq!(s.offset_y, 4.0);
        assert_eq!(s.blur_radius, 8.0);
    }

    #[test]
    fn test_div_shadow_sm() {
        let d = Div::new().shadow_sm();
        assert!(d.style.shadow.is_some());
        let s = d.style.shadow.unwrap();
        assert_eq!(s.offset_x, 0.0);
        assert_eq!(s.offset_y, 1.0);
        assert_eq!(s.blur_radius, 2.0);
    }

    #[test]
    fn test_div_shadow_md() {
        let d = Div::new().shadow_md();
        assert!(d.style.shadow.is_some());
        let s = d.style.shadow.unwrap();
        assert_eq!(s.offset_x, 0.0);
        assert_eq!(s.offset_y, 4.0);
        assert_eq!(s.blur_radius, 6.0);
    }

    #[test]
    fn test_div_shadow_lg() {
        let d = Div::new().shadow_lg();
        assert!(d.style.shadow.is_some());
        let s = d.style.shadow.unwrap();
        assert_eq!(s.offset_x, 0.0);
        assert_eq!(s.offset_y, 10.0);
        assert_eq!(s.blur_radius, 15.0);
    }

    #[test]
    fn test_div_no_shadow_by_default() {
        let d = Div::new();
        assert!(d.style.shadow.is_none());
    }

    // ==================== Opacity ====================

    #[test]
    fn test_div_opacity() {
        let d = Div::new().opacity(0.5);
        assert_eq!(d.style.opacity, 0.5);
    }

    #[test]
    fn test_div_opacity_full() {
        let d = Div::new().opacity(1.0);
        assert_eq!(d.style.opacity, 1.0);
    }

    #[test]
    fn test_div_opacity_zero() {
        let d = Div::new().opacity(0.0);
        assert_eq!(d.style.opacity, 0.0);
    }

    // ==================== Overflow ====================

    #[test]
    fn test_div_overflow_hidden() {
        let d = Div::new().overflow_hidden();
        assert_eq!(d.style.overflow_x, Overflow::Hidden);
        assert_eq!(d.style.overflow_y, Overflow::Hidden);
    }

    #[test]
    fn test_div_overflow_scroll() {
        let d = Div::new().overflow_scroll();
        assert_eq!(d.style.overflow_x, Overflow::Scroll);
        assert_eq!(d.style.overflow_y, Overflow::Scroll);
    }

    #[test]
    fn test_div_overflow_default() {
        let d = Div::new();
        assert_eq!(d.style.overflow_x, Overflow::Visible);
        assert_eq!(d.style.overflow_y, Overflow::Visible);
    }

    // ==================== Position ====================

    #[test]
    fn test_div_absolute() {
        let d = Div::new().absolute();
        assert_eq!(d.style.position, Position::Absolute);
    }

    #[test]
    fn test_div_relative() {
        let d = Div::new().relative();
        assert_eq!(d.style.position, Position::Relative);
    }

    #[test]
    fn test_div_position_default() {
        let d = Div::new();
        assert_eq!(d.style.position, Position::Relative);
    }

    // ==================== Children ====================

    #[test]
    fn test_div_child() {
        let child = Div::new().w(50.0);
        let parent = Div::new().child(child);
        assert_eq!(parent.children.len(), 1);
    }

    #[test]
    fn test_div_multiple_children() {
        let parent = Div::new()
            .child(Div::new().w(50.0))
            .child(Div::new().w(60.0))
            .child(Div::new().w(70.0));
        assert_eq!(parent.children.len(), 3);
    }

    #[test]
    fn test_div_children_from_iterator() {
        let child_divs = vec![
            Div::new().w(10.0),
            Div::new().w(20.0),
            Div::new().w(30.0),
        ];
        let parent = Div::new().children(child_divs);
        assert_eq!(parent.children.len(), 3);
    }

    #[test]
    fn test_div_children_empty_iterator() {
        let empty: Vec<Div> = vec![];
        let parent = Div::new().children(empty);
        assert!(parent.children.is_empty());
    }

    #[test]
    fn test_div_children_combined() {
        let parent = Div::new()
            .child(Div::new())
            .children(vec![Div::new(), Div::new()]);
        assert_eq!(parent.children.len(), 3);
    }

    #[test]
    fn test_div_element_trait_children() {
        let parent = Div::new()
            .child(Div::new())
            .child(Div::new());
        let children = <Div as Element>::children(&parent);
        assert_eq!(children.len(), 2);
    }

    // ==================== Event Handlers ====================

    #[test]
    fn test_div_on_click() {
        let d = Div::new().on_click(|| {});
        assert!(d.on_click.is_some());
    }

    #[test]
    fn test_div_on_hover() {
        let d = Div::new().on_hover(|_hovered| {});
        assert!(d.on_hover.is_some());
    }

    #[test]
    fn test_div_no_event_handlers_by_default() {
        let d = Div::new();
        assert!(d.on_click.is_none());
        assert!(d.on_hover.is_none());
    }

    // ==================== Element Trait ====================

    #[test]
    fn test_div_element_trait_style() {
        let d = Div::new().w(100.0).h(50.0);
        let style = d.style();
        assert_eq!(style.width, Some(100.0));
        assert_eq!(style.height, Some(50.0));
    }

    // ==================== Chained Builder Pattern ====================

    #[test]
    fn test_div_chained_builder_comprehensive() {
        let d = div()
            .id(ElementId::from(1u64))
            .w(400.0)
            .h(300.0)
            .min_w(100.0)
            .min_h(50.0)
            .max_w(800.0)
            .max_h(600.0)
            .flex_col()
            .gap(12.0)
            .justify_center()
            .items_center()
            .p(16.0)
            .m(8.0)
            .bg(Color::WHITE)
            .border(1.0, Color::BLACK)
            .rounded(8.0)
            .shadow_md()
            .opacity(0.9)
            .overflow_hidden();

        assert_eq!(d.id, Some(ElementId::from(1u64)));
        assert_eq!(d.style.width, Some(400.0));
        assert_eq!(d.style.height, Some(300.0));
        assert_eq!(d.style.min_width, Some(100.0));
        assert_eq!(d.style.min_height, Some(50.0));
        assert_eq!(d.style.max_width, Some(800.0));
        assert_eq!(d.style.max_height, Some(600.0));
        assert_eq!(d.style.display, Display::Flex);
        assert_eq!(d.style.flex_direction, FlexDirection::Column);
        assert_eq!(d.style.gap, 12.0);
        assert_eq!(d.style.justify_content, JustifyContent::Center);
        assert_eq!(d.style.align_items, AlignItems::Center);
        assert_eq!(d.style.padding, Edges::all(16.0));
        assert_eq!(d.style.margin, Edges::all(8.0));
        assert_eq!(d.style.border.width, Edges::all(1.0));
        assert_eq!(d.style.border.color, Color::BLACK);
        assert_eq!(d.style.border.radius, Corners::all(8.0));
        assert!(d.style.shadow.is_some());
        assert_eq!(d.style.opacity, 0.9);
        assert_eq!(d.style.overflow_x, Overflow::Hidden);
        assert_eq!(d.style.overflow_y, Overflow::Hidden);
    }

    #[test]
    fn test_div_layout_with_children() {
        let parent = div()
            .flex_row()
            .gap(8.0)
            .child(div().w(100.0).h(50.0))
            .child(div().w(100.0).h(50.0))
            .child(div().w(100.0).h(50.0));

        assert_eq!(parent.children.len(), 3);
        assert_eq!(parent.style.flex_direction, FlexDirection::Row);
        assert_eq!(parent.style.gap, 8.0);
    }

    #[test]
    fn test_div_nested_layout() {
        let layout = div()
            .flex_col()
            .child(
                div()
                    .flex_row()
                    .child(div().w(50.0))
                    .child(div().w(50.0))
            )
            .child(
                div()
                    .flex_row()
                    .child(div().w(50.0))
                    .child(div().w(50.0))
            );

        assert_eq!(layout.children.len(), 2);
    }

    // ==================== Style Default Values ====================

    #[test]
    fn test_div_style_default_values() {
        let d = Div::new();
        let style = d.style();

        // Default flex shrink should be 1.0 from Style::new()
        assert_eq!(style.flex_shrink, 1.0);
        // Default opacity should be 1.0 from Style::new()
        assert_eq!(style.opacity, 1.0);
        // Default display is Flex
        assert_eq!(style.display, Display::Flex);
        // Default flex direction is Row
        assert_eq!(style.flex_direction, FlexDirection::Row);
        // Default justify content is FlexStart
        assert_eq!(style.justify_content, JustifyContent::FlexStart);
        // Default align items is Stretch
        assert_eq!(style.align_items, AlignItems::Stretch);
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_div_zero_dimensions() {
        let d = Div::new().w(0.0).h(0.0);
        assert_eq!(d.style.width, Some(0.0));
        assert_eq!(d.style.height, Some(0.0));
    }

    #[test]
    fn test_div_negative_dimensions() {
        // Note: The API allows negative values; validation should happen at layout time
        let d = Div::new().w(-10.0).h(-20.0);
        assert_eq!(d.style.width, Some(-10.0));
        assert_eq!(d.style.height, Some(-20.0));
    }

    #[test]
    fn test_div_large_dimensions() {
        let d = Div::new().w(10000.0).h(10000.0);
        assert_eq!(d.style.width, Some(10000.0));
        assert_eq!(d.style.height, Some(10000.0));
    }

    #[test]
    fn test_div_overwrite_style() {
        let d = Div::new()
            .w(100.0)
            .w(200.0) // Overwrite
            .h(50.0)
            .h(100.0); // Overwrite

        assert_eq!(d.style.width, Some(200.0));
        assert_eq!(d.style.height, Some(100.0));
    }

    #[test]
    fn test_div_flex_grow_after_w_full() {
        let d = Div::new().w_full().flex_grow(2.0);
        // flex_grow should be overwritten
        assert_eq!(d.style.flex_grow, 2.0);
    }

    #[test]
    fn test_div_flex_direction_after_flex_row() {
        let d = Div::new().flex_row().flex_col();
        // Should end up as column
        assert_eq!(d.style.flex_direction, FlexDirection::Column);
    }

    // ==================== Padding and Margin Table-Driven Tests ====================

    struct PaddingTestCase {
        name: &'static str,
        top: f32,
        right: f32,
        bottom: f32,
        left: f32,
    }

    #[test]
    fn test_div_padding_individual() {
        let test_cases = [
            PaddingTestCase { name: "pt only", top: 10.0, right: 0.0, bottom: 0.0, left: 0.0 },
            PaddingTestCase { name: "pr only", top: 0.0, right: 10.0, bottom: 0.0, left: 0.0 },
            PaddingTestCase { name: "pb only", top: 0.0, right: 0.0, bottom: 10.0, left: 0.0 },
            PaddingTestCase { name: "pl only", top: 0.0, right: 0.0, bottom: 0.0, left: 10.0 },
        ];

        for tc in test_cases {
            let mut d = Div::new();
            if tc.top > 0.0 {
                d = d.pt(tc.top);
            }
            if tc.right > 0.0 {
                d = d.pr(tc.right);
            }
            if tc.bottom > 0.0 {
                d = d.pb(tc.bottom);
            }
            if tc.left > 0.0 {
                d = d.pl(tc.left);
            }

            assert_eq!(d.style.padding.top, tc.top, "failed for case: {}", tc.name);
            assert_eq!(d.style.padding.right, tc.right, "failed for case: {}", tc.name);
            assert_eq!(d.style.padding.bottom, tc.bottom, "failed for case: {}", tc.name);
            assert_eq!(d.style.padding.left, tc.left, "failed for case: {}", tc.name);
        }
    }

    // ==================== Shadow Table-Driven Tests ====================

    struct ShadowTestCase {
        name: &'static str,
        offset_y: f32,
        blur_radius: f32,
    }

    #[test]
    fn test_div_shadow_presets() {
        let test_cases = [
            ShadowTestCase { name: "sm", offset_y: 1.0, blur_radius: 2.0 },
            ShadowTestCase { name: "md", offset_y: 4.0, blur_radius: 6.0 },
            ShadowTestCase { name: "lg", offset_y: 10.0, blur_radius: 15.0 },
        ];

        for tc in test_cases {
            let d = match tc.name {
                "sm" => Div::new().shadow_sm(),
                "md" => Div::new().shadow_md(),
                "lg" => Div::new().shadow_lg(),
                _ => unreachable!(),
            };

            let shadow = d.style.shadow.expect(&format!("shadow should be set for {}", tc.name));
            assert_eq!(shadow.offset_x, 0.0, "offset_x for {}", tc.name);
            assert_eq!(shadow.offset_y, tc.offset_y, "offset_y for {}", tc.name);
            assert_eq!(shadow.blur_radius, tc.blur_radius, "blur_radius for {}", tc.name);
        }
    }

    // ==================== Border Radius Table-Driven Tests ====================

    struct BorderRadiusTestCase {
        name: &'static str,
        top_left: f32,
        top_right: f32,
        bottom_right: f32,
        bottom_left: f32,
    }

    #[test]
    fn test_div_border_radius_variants() {
        let test_cases = [
            BorderRadiusTestCase {
                name: "rounded_all",
                top_left: 8.0,
                top_right: 8.0,
                bottom_right: 8.0,
                bottom_left: 8.0,
            },
            BorderRadiusTestCase {
                name: "rounded_t",
                top_left: 12.0,
                top_right: 12.0,
                bottom_right: 0.0,
                bottom_left: 0.0,
            },
            BorderRadiusTestCase {
                name: "rounded_b",
                top_left: 0.0,
                top_right: 0.0,
                bottom_right: 10.0,
                bottom_left: 10.0,
            },
            BorderRadiusTestCase {
                name: "rounded_full",
                top_left: 9999.0,
                top_right: 9999.0,
                bottom_right: 9999.0,
                bottom_left: 9999.0,
            },
        ];

        for tc in test_cases {
            let d = match tc.name {
                "rounded_all" => Div::new().rounded(8.0),
                "rounded_t" => Div::new().rounded_t(12.0),
                "rounded_b" => Div::new().rounded_b(10.0),
                "rounded_full" => Div::new().rounded_full(),
                _ => unreachable!(),
            };

            assert_eq!(d.style.border.radius.top_left, tc.top_left, "top_left for {}", tc.name);
            assert_eq!(d.style.border.radius.top_right, tc.top_right, "top_right for {}", tc.name);
            assert_eq!(d.style.border.radius.bottom_right, tc.bottom_right, "bottom_right for {}", tc.name);
            assert_eq!(d.style.border.radius.bottom_left, tc.bottom_left, "bottom_left for {}", tc.name);
        }
    }
}
