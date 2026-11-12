//! Div element - the primary container element

use crate::core::ElementId;
use crate::core::color::Color;
use crate::core::event::MouseButton;
use crate::core::geometry::{Bounds, Edges, Point, Size};
use crate::core::style::{
    AlignItems, Background, BorderStyle, Corners, Dimension as StyleDimension, Display,
    FlexDirection, JustifyContent, Overflow, Position, Shadow, Style,
};
use crate::core::text_editing::{TextInputCommand, TextInputEvent};
use crate::elements::element::{
    AnyElement, Capabilities, Element, EventContext, LayoutContext, PaintContext, PointerEvent,
    PointerEventKind, StableId, dispatch_action_to_children, style_to_taffy,
};
use crate::elements::text::{FontWeight, TextStyleRefinement};
use crate::renderer::Primitive;
use crate::renderer::text::TextMeasureCache;
use smallvec::SmallVec;
use std::any::Any;
use taffy::prelude::*;

/// A flexible container element (like HTML div)
pub struct Div {
    id: Option<ElementId>,
    style: Style,
    text_style: TextStyleRefinement,
    children: SmallVec<[AnyElement; 4]>,
    on_click: Option<Box<dyn Fn()>>,
    on_hover: Option<Box<dyn Fn(bool)>>,
    on_context_menu: Option<Box<dyn Fn(Point)>>,
    /// Drag source and drop target handlers, when it has any.
    drag: Option<Box<drag::DragHandlers>>,
    actions: actions::ActionHandlers,
    hovered: bool,
    layout_node: Option<NodeId>,
    child_nodes: SmallVec<[NodeId; 4]>,
}

impl Div {
    pub fn new() -> Self {
        Self {
            id: None,
            style: Style::new(),
            text_style: TextStyleRefinement::default(),
            children: SmallVec::new(),
            on_click: None,
            on_hover: None,
            on_context_menu: None,
            drag: None,
            actions: Vec::new(),
            hovered: false,
            layout_node: None,
            child_nodes: SmallVec::new(),
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
        self.style.set_width_dimension(s.width);
        self.style.set_height_dimension(s.height);
        self
    }

    pub fn w(mut self, width: f32) -> Self {
        self.style.set_width_dimension(width);
        self
    }

    pub fn width(mut self, width: impl Into<StyleDimension>) -> Self {
        self.style.set_width_dimension(width);
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        self.style.set_height_dimension(height);
        self
    }

    pub fn height(mut self, height: impl Into<StyleDimension>) -> Self {
        self.style.set_height_dimension(height);
        self
    }

    pub fn w_percent(self, percent: f32) -> Self {
        self.width(StyleDimension::percent(percent))
    }

    pub fn h_percent(self, percent: f32) -> Self {
        self.height(StyleDimension::percent(percent))
    }

    pub fn w_auto(self) -> Self {
        self.width(StyleDimension::auto())
    }

    pub fn h_auto(self) -> Self {
        self.height(StyleDimension::auto())
    }

    pub fn w_full(mut self) -> Self {
        self.style.set_width_dimension(StyleDimension::fill());
        self.style.flex_grow = 1.0;
        self
    }

    pub fn h_full(mut self) -> Self {
        self.style.set_height_dimension(StyleDimension::fill());
        self.style.flex_grow = 1.0;
        self
    }

    pub fn min_w(mut self, width: f32) -> Self {
        self.style.set_min_width_dimension(width);
        self
    }

    pub fn min_w_percent(mut self, percent: f32) -> Self {
        self.style
            .set_min_width_dimension(StyleDimension::percent(percent));
        self
    }

    pub fn min_h(mut self, height: f32) -> Self {
        self.style.set_min_height_dimension(height);
        self
    }

    pub fn min_h_percent(mut self, percent: f32) -> Self {
        self.style
            .set_min_height_dimension(StyleDimension::percent(percent));
        self
    }

    pub fn max_w(mut self, width: f32) -> Self {
        self.style.set_max_width_dimension(width);
        self
    }

    pub fn max_w_percent(mut self, percent: f32) -> Self {
        self.style
            .set_max_width_dimension(StyleDimension::percent(percent));
        self
    }

    pub fn max_h(mut self, height: f32) -> Self {
        self.style.set_max_height_dimension(height);
        self
    }

    pub fn max_h_percent(mut self, percent: f32) -> Self {
        self.style
            .set_max_height_dimension(StyleDimension::percent(percent));
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

    pub fn bg_gradient(
        mut self,
        start: impl Into<Color>,
        end: impl Into<Color>,
        angle: f32,
    ) -> Self {
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
        self.style.shadow = Some(Shadow::new(
            0.0,
            10.0,
            15.0,
            Color::rgba(0.0, 0.0, 0.0, 0.1),
        ));
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

    // Text style inherited by descendant text that does not set its own
    pub fn text_color(mut self, color: impl Into<Color>) -> Self {
        self.text_style.color = Some(color.into());
        self
    }

    pub fn text_size(mut self, size: f32) -> Self {
        self.text_style.font_size = Some(size);
        self
    }

    pub fn font_weight(mut self, weight: FontWeight) -> Self {
        self.text_style.font_weight = Some(weight);
        self
    }

    pub fn font_family(mut self, family: impl Into<String>) -> Self {
        self.text_style.font_family = Some(family.into());
        self
    }

    pub fn line_height(mut self, height: f32) -> Self {
        self.text_style.line_height = Some(height);
        self
    }

    // Events
    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    /// Called with the position of a right press inside, which neither clicks
    /// nor drags: show a menu there with `cx.show_context_menu`.
    pub fn on_context_menu(mut self, handler: impl Fn(Point) + 'static) -> Self {
        self.on_context_menu = Some(Box::new(handler));
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

    fn stable_id_mut(&mut self) -> Option<&mut StableId> {
        self.drag.as_mut().map(|drag| &mut drag.identity)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn capabilities(&self) -> Capabilities {
        // Hover and click state change what handlers see, never layout or
        // paint. Without handlers, its hover state is seen by nothing; a press
        // still clears focus, so presses are always delivered.
        Capabilities {
            pure: true,
            pointer_inert: self.on_click.is_none()
                && self.on_hover.is_none()
                && self.on_context_menu.is_none()
                && self.drag.is_none(),
        }
    }

    fn same_props(&self, previous: &dyn Element) -> bool {
        let Some(previous) = (previous as &dyn Any).downcast_ref::<Div>() else {
            return false;
        };
        // Handlers cannot be compared, so a div with any keeps the new instance.
        !self.has_handlers()
            && !previous.has_handlers()
            && self.id == previous.id
            && self.style == previous.style
            && self.text_style == previous.text_style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        // Layout children first
        let children = &mut self.children;
        let child_nodes: Vec<NodeId> = cx.with_text_style(&self.text_style, |cx| {
            children.iter_mut().map(|child| child.layout(cx)).collect()
        });

        // Create this node
        let taffy_style = style_to_taffy(&self.style);
        let node = cx
            .new_with_children(taffy_style, &child_nodes)
            .expect("Failed to create layout node");

        self.layout_node = Some(node);
        self.child_nodes = SmallVec::from_vec(child_nodes);
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
                    border_color: self.style.border.color.to_rgba(),
                    border_widths: self.style.border.width,
                    corner_radii: self.style.border.radius,
                });
            }
            Background::RadialGradient { inner, outer } => {
                cx.paint(Primitive::RadialGradient {
                    bounds,
                    inner: inner.to_rgba(),
                    outer: outer.to_rgba(),
                    border_color: self.style.border.color.to_rgba(),
                    border_widths: self.style.border.width,
                    corner_radii: self.style.border.radius,
                });
            }
        }

        // Paint children
        // Note: In a full implementation, we'd get child bounds from the layout tree
        for (child, node) in self
            .children
            .iter_mut()
            .zip(self.child_nodes.iter().copied())
        {
            let child_bounds = cx.child_bounds(node).unwrap_or(bounds);
            let mut child_cx = cx.with_bounds(child_bounds);
            child.paint(&mut child_cx);
        }
    }

    fn refresh_text_geometry(&mut self, text_measurer: &mut TextMeasureCache) {
        for child in &mut self.children {
            child.refresh_text_geometry(text_measurer);
        }
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let is_move = matches!(event.kind, PointerEventKind::Move);
        let is_press = matches!(event.kind, PointerEventKind::Down);
        // A release that ends a drag drops instead of clicking.
        let dragging = cx.is_dragging();
        let mut handled = false;

        for (child, node) in self
            .children
            .iter_mut()
            .zip(self.child_nodes.iter().copied())
            .rev()
        {
            // As `AnyElement::handle_pointer_event` would, without the bounds.
            if !is_press && child.pointer_inert() {
                continue;
            }
            let child_bounds = cx.child_bounds(node).unwrap_or(cx.bounds());
            let mut child_cx = cx.with_bounds(child_bounds);
            let child_handled = child.handle_pointer_event(&mut child_cx, event);
            if !is_move && child_handled {
                handled = true;
                break;
            }
        }

        let inside = cx.contains_pointer(event.position);

        if is_move && inside != self.hovered {
            self.hovered = inside;
            if let Some(handler) = &self.on_hover {
                handler(inside);
            }
        }

        if !handled && matches!(event.kind, PointerEventKind::Down) && !inside {
            cx.clear_focus();
        }

        if event.button == Some(MouseButton::Right) {
            if !handled && is_press && inside && let Some(handler) = &self.on_context_menu {
                handler(event.position);
                handled = true;
            }
            return handled;
        }

        if let Some(drag) = &mut self.drag
            && drag.handle_pointer_event(cx, event, inside, handled)
        {
            handled = true;
        }

        if !handled
            && matches!(event.kind, PointerEventKind::Up)
            && inside
            && !dragging
            && let Some(handler) = &self.on_click
        {
            handler();
            handled = true;
        }

        handled
    }

    fn handle_scroll_event(
        &mut self,
        cx: &mut EventContext,
        event: &crate::core::event::ScrollEvent,
    ) -> bool {
        let focused = cx.focused_id();
        let mut handled = false;

        for (child, node) in self
            .children
            .iter_mut()
            .zip(self.child_nodes.iter().copied())
            .rev()
        {
            let child_bounds = cx.child_bounds(node).unwrap_or(cx.bounds());
            let mut child_cx = cx.with_bounds(child_bounds);

            if let Some(focused) = focused {
                if child.id() == Some(focused) {
                    handled = child.handle_scroll_event(&mut child_cx, event);
                    break;
                }
            }

            if child.handle_scroll_event(&mut child_cx, event) {
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
            for child in self.children.iter_mut().rev() {
                if child.id() == Some(focused) {
                    return child.handle_key_event(cx, event);
                }
            }
        }

        for child in self.children.iter_mut().rev() {
            if child.handle_key_event(cx, event) {
                return true;
            }
        }

        false
    }

    fn dispatch_action(
        &mut self,
        cx: &mut EventContext,
        action: &crate::core::action::ActionId,
    ) -> crate::core::action::ActionOutcome {
        let outcome = dispatch_action_to_children(&mut self.children, cx, action);
        if outcome.is_handled() {
            return outcome;
        }
        self.run_action(action)
    }

    fn handle_text_input_event(&mut self, cx: &mut EventContext, event: &TextInputEvent) -> bool {
        if let Some(focused) = cx.focused_id() {
            for child in self.children.iter_mut().rev() {
                if child.id() == Some(focused) {
                    return child.handle_text_input_event(cx, event);
                }
            }
        }

        for child in self.children.iter_mut().rev() {
            if child.handle_text_input_event(cx, event) {
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
            for child in self.children.iter_mut().rev() {
                if child.contains_id(focused) {
                    return child.handle_text_input_command(cx, command);
                }
            }
        }
        self.children
            .iter_mut()
            .rev()
            .any(|child| child.handle_text_input_command(cx, command))
    }

    fn handle_window_event(&mut self, event: &crate::core::event::Event) -> bool {
        for child in self.children.iter_mut().rev() {
            if child.handle_window_event(event) {
                return true;
            }
        }
        false
    }

    fn children(&self) -> &[AnyElement] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut [AnyElement] {
        &mut self.children
    }
}

/// Create a new Div element
pub fn div() -> Div {
    Div::new()
}

mod actions;
mod drag;
#[cfg(test)]
mod tests;
