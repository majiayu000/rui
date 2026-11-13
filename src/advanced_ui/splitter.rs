use crate::advanced_ui::slider::format_number;
use crate::advanced_ui::state::{require_finite, require_non_empty};
use crate::advanced_ui::tokens::Theme;
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityNumericValue, AccessibilityRole,
};
use crate::core::action::{
    ACCESSIBILITY_DECREMENT_ACTION, ACCESSIBILITY_INCREMENT_ACTION, ActionId, ActionOutcome,
    StandardAction,
};
use crate::core::color::Color;
use crate::core::event::{Cursor, KeyCode, KeyEvent, Modifiers, MouseButton, ScrollEvent};
use crate::core::geometry::{Bounds, Edges, Point};
use crate::core::style::{Corners, Style};
use crate::core::text_editing::{TextInputCommand, TextInputEvent, TextInputSnapshot};
use crate::elements::element::StableId;
use crate::elements::element::{
    AnyElement, Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    dispatch_action_to_children,
};
use crate::renderer::Primitive;
use crate::renderer::text::TextMeasureCache;
use taffy::prelude::*;

const DIVIDER: f32 = 6.0;
const KEY_STEP: f32 = 10.0;

type SplitterChangeHandler = Box<dyn Fn(f32)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SplitAxis {
    /// Panes side by side, divided by a vertical bar.
    #[default]
    Horizontal,
    /// Panes stacked, divided by a horizontal bar.
    Vertical,
}

/// Two panes divided by a bar that drags to resize them. `position` is the
/// size of the first pane; the second takes the rest. Give the pane content
/// `w_full()` / `h_full()` to fill its pane.
pub struct Splitter {
    id: StableId,
    label: String,
    axis: SplitAxis,
    position: f32,
    min: f32,
    max: f32,
    disabled: bool,
    dragging: bool,
    /// Where in the divider the drag grabbed it, along the axis.
    grab: f32,
    hovered: bool,
    /// The splitter's length along the axis when it last painted.
    extent: f32,
    theme: Theme,
    style: Style,
    children: Vec<AnyElement>,
    nodes: Vec<NodeId>,
    on_change: Option<SplitterChangeHandler>,
}

impl Splitter {
    pub fn new(
        label: impl Into<String>,
        first: impl Into<AnyElement>,
        second: impl Into<AnyElement>,
    ) -> Self {
        let label = label.into();
        require_non_empty(&label, "splitter accessibility label must not be empty");
        let style = Style::new();

        Self {
            id: StableId::new(),
            label,
            axis: SplitAxis::default(),
            position: 200.0,
            min: 0.0,
            max: f32::INFINITY,
            disabled: false,
            dragging: false,
            grab: 0.0,
            hovered: false,
            extent: f32::INFINITY,
            theme: Theme::default(),
            style,
            children: vec![first.into(), second.into()],
            nodes: Vec::new(),
            on_change: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = StableId::explicit(id);
        self
    }

    pub fn axis(mut self, axis: SplitAxis) -> Self {
        self.axis = axis;
        self
    }

    /// The size of the first pane.
    pub fn position(mut self, position: f32) -> Self {
        require_finite(position, "splitter position must be finite");
        self.position = position;
        self
    }

    /// The smallest and largest the first pane may be.
    pub fn limits(mut self, min: f32, max: f32) -> Self {
        assert!(
            min.is_finite() && min >= 0.0 && min <= max,
            "splitter limits must satisfy 0 <= min <= max"
        );
        self.min = min;
        self.max = max;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.style.width = Some(width);
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.style.height = Some(height);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        if disabled {
            self.dragging = false;
            self.hovered = false;
        }
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    pub fn on_change(mut self, handler: impl Fn(f32) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    pub fn current_position(&self) -> f32 {
        self.clamp(self.position)
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    pub(crate) fn reconcile_transient_state(&mut self, previous: &Self) {
        if !self.disabled {
            self.dragging = previous.dragging;
            self.grab = previous.grab;
            self.hovered = previous.hovered;
        }
        self.extent = previous.extent;
    }

    fn largest(&self) -> f32 {
        self.max.min((self.extent - DIVIDER).max(self.min))
    }

    fn clamp(&self, position: f32) -> f32 {
        position.clamp(self.min, self.largest())
    }

    fn along(&self, point: Point) -> f32 {
        match self.axis {
            SplitAxis::Horizontal => point.x,
            SplitAxis::Vertical => point.y,
        }
    }

    fn start(&self, bounds: Bounds) -> f32 {
        match self.axis {
            SplitAxis::Horizontal => bounds.x(),
            SplitAxis::Vertical => bounds.y(),
        }
    }

    fn divider_bounds(&self, bounds: Bounds) -> Bounds {
        let at = self.current_position();
        match self.axis {
            SplitAxis::Horizontal => {
                Bounds::from_xywh(bounds.x() + at, bounds.y(), DIVIDER, bounds.height())
            }
            SplitAxis::Vertical => {
                Bounds::from_xywh(bounds.x(), bounds.y() + at, bounds.width(), DIVIDER)
            }
        }
    }

    fn cursor(&self) -> Cursor {
        match self.axis {
            SplitAxis::Horizontal => Cursor::ResizeEW,
            SplitAxis::Vertical => Cursor::ResizeNS,
        }
    }

    fn set_position(&mut self, position: f32, cx: &EventContext) -> bool {
        if self.disabled {
            return false;
        }
        let position = self.clamp(position);
        if position != self.position {
            self.position = position;
            if let Some(handler) = &self.on_change {
                handler(position);
            }
            cx.request_redraw();
        }
        true
    }

    fn step(&mut self, delta: f32, cx: &EventContext) -> ActionOutcome {
        if self.set_position(self.current_position() + delta, cx) {
            ActionOutcome::handled("advanced_ui.splitter")
        } else {
            ActionOutcome::Ignored
        }
    }

    fn pane_bounds(&self, bounds: Bounds, child: impl Fn(NodeId) -> Option<Bounds>) -> [Bounds; 2] {
        // Nodes: first content, second content, first pane, divider, second pane.
        let pane = |index: usize| self.nodes.get(index).and_then(|node| child(*node));
        [pane(2).unwrap_or(bounds), pane(4).unwrap_or(bounds)]
    }

    fn content_node(&self, index: usize) -> Option<NodeId> {
        self.nodes.get(index).copied()
    }
}

impl Element for Splitter {
    fn id(&self) -> Option<ElementId> {
        Some(self.id.get())
    }

    fn stable_id_mut(&mut self) -> Option<&mut StableId> {
        Some(&mut self.id)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let contents: Vec<NodeId> = self
            .children
            .iter_mut()
            .map(|child| child.layout(cx))
            .collect();
        let horizontal = self.axis == SplitAxis::Horizontal;
        let at = self.current_position();
        let along = |length: Dimension| {
            if horizontal {
                Size {
                    width: length,
                    height: Dimension::Percent(1.0),
                }
            } else {
                Size {
                    width: Dimension::Percent(1.0),
                    height: length,
                }
            }
        };
        let pane_style = |size: Size<Dimension>, grow: f32| taffy::Style {
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            size,
            flex_grow: grow,
            flex_shrink: if grow > 0.0 { 1.0 } else { 0.0 },
            flex_basis: if grow > 0.0 {
                Dimension::Length(0.0)
            } else {
                Dimension::Auto
            },
            min_size: Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            overflow: taffy::Point {
                x: taffy::Overflow::Hidden,
                y: taffy::Overflow::Hidden,
            },
            ..Default::default()
        };
        let fail = |err| panic!("failed to create splitter layout node: {err}");
        let first = cx
            .new_with_children(
                pane_style(along(Dimension::Length(at)), 0.0),
                &contents[..1],
            )
            .unwrap_or_else(fail);
        let divider = cx
            .new_leaf(pane_style(along(Dimension::Length(DIVIDER)), 0.0))
            .unwrap_or_else(fail);
        let second = cx
            .new_with_children(pane_style(along(Dimension::Auto), 1.0), &contents[1..])
            .unwrap_or_else(fail);
        // Fill the parent unless given a size.
        let mut style = crate::elements::element::style_to_taffy(&self.style);
        let fill = |set: Option<f32>| set.map_or(Dimension::Percent(1.0), Dimension::Length);
        style.size = Size {
            width: fill(self.style.width),
            height: fill(self.style.height),
        };
        style.display = Display::Flex;
        style.flex_direction = if horizontal {
            FlexDirection::Row
        } else {
            FlexDirection::Column
        };
        self.nodes = vec![contents[0], contents[1], first, divider, second];
        cx.new_with_children(style, &[first, divider, second])
            .unwrap_or_else(fail)
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        self.extent = match self.axis {
            SplitAxis::Horizontal => bounds.width(),
            SplitAxis::Vertical => bounds.height(),
        };
        let panes = self.pane_bounds(bounds, |node| cx.child_bounds(node));
        for (index, pane) in panes.into_iter().enumerate() {
            let mut pane_cx = cx.with_bounds(pane);
            let content = self
                .content_node(index)
                .and_then(|node| pane_cx.child_bounds(node))
                .unwrap_or(pane);
            self.children[index].paint(&mut pane_cx.with_bounds(content));
        }

        let divider = self.divider_bounds(bounds);
        cx.register_hit_region(self.id.get(), divider);
        let focused = cx.is_focused(self.id.get()) && !self.disabled;
        let color = if focused {
            self.theme.state.focus_ring
        } else if self.dragging || self.hovered {
            self.theme.colors.primary.rest.background
        } else {
            self.theme.colors.border
        };
        cx.paint(Primitive::Quad {
            bounds: divider,
            background: self.theme.colors.surface.to_rgba(),
            border_color: Color::TRANSPARENT.to_rgba(),
            border_widths: Edges::ZERO,
            corner_radii: Corners::ZERO,
        });
        let line = match self.axis {
            SplitAxis::Horizontal => Bounds::from_xywh(
                divider.x() + DIVIDER / 2.0 - 1.0,
                divider.y(),
                2.0,
                divider.height(),
            ),
            SplitAxis::Vertical => Bounds::from_xywh(
                divider.x(),
                divider.y() + DIVIDER / 2.0 - 1.0,
                divider.width(),
                2.0,
            ),
        };
        cx.paint(Primitive::Quad {
            bounds: line,
            background: color.to_rgba(),
            border_color: Color::TRANSPARENT.to_rgba(),
            border_widths: Edges::ZERO,
            corner_radii: Corners::ZERO,
        });
    }

    fn refresh_text_geometry(&mut self, text_measurer: &mut TextMeasureCache) {
        for child in &mut self.children {
            child.refresh_text_geometry(text_measurer);
        }
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let position = self.current_position();
        let mut node = AccessibilityNode::label_required(
            self.id.get(),
            AccessibilityRole::Splitter,
            &self.label,
        )?
        .value_required(format_number(position))?
        .with_numeric_value(AccessibilityNumericValue::new(
            f64::from(position),
            f64::from(self.min),
            f64::from(self.largest()),
        ))
        .with_enabled(!self.disabled)
        .with_focused(cx.a11y_has_focus(self.id.get()));
        if !self.disabled {
            node = node.with_actions([
                AccessibilityAction::Increment,
                AccessibilityAction::Decrement,
            ]);
        }
        Ok(Some(node))
    }

    fn accessibility_nodes(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Vec<AccessibilityNode>, AccessibilityError> {
        let mut nodes = self.children[0].accessibility_nodes(cx)?;
        nodes.extend(self.accessibility(cx)?);
        nodes.extend(self.children[1].accessibility_nodes(cx)?);
        Ok(nodes)
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let bounds = cx.bounds();
        let divider = self.divider_bounds(bounds);
        let on_divider = cx.with_bounds(divider).contains_pointer(event.position);
        match event.kind {
            PointerEventKind::Move if self.dragging && event.button == Some(MouseButton::Left) => {
                cx.set_cursor(self.cursor());
                let at = self.along(event.position) - self.start(bounds) - self.grab;
                return self.set_position(at, cx);
            }
            PointerEventKind::Down
                if on_divider && !self.disabled && event.button == Some(MouseButton::Left) =>
            {
                self.dragging = true;
                self.grab = self.along(event.position) - self.start(divider);
                cx.request_focus(Some(self.id.get()));
                cx.request_redraw();
                return true;
            }
            PointerEventKind::Up if self.dragging => {
                self.dragging = false;
                cx.request_redraw();
                return true;
            }
            PointerEventKind::Move => {
                self.dragging = false;
                let hovered = on_divider && !self.disabled;
                if hovered != self.hovered {
                    self.hovered = hovered;
                    cx.request_redraw();
                }
                if hovered {
                    cx.set_cursor(self.cursor());
                }
            }
            _ => {}
        }

        let panes = self.pane_bounds(bounds, |node| cx.child_bounds(node));
        let is_move = matches!(event.kind, PointerEventKind::Move);
        let mut handled = false;
        for index in (0..2).rev() {
            let mut pane_cx = cx.with_bounds(panes[index]);
            let content = self
                .content_node(index)
                .and_then(|node| pane_cx.child_bounds(node))
                .unwrap_or(panes[index]);
            handled |=
                self.children[index].handle_pointer_event(&mut pane_cx.with_bounds(content), event);
            if handled && !is_move {
                break;
            }
        }
        handled
    }

    fn handle_scroll_event(&mut self, cx: &mut EventContext, event: &ScrollEvent) -> bool {
        let panes = self.pane_bounds(cx.bounds(), |node| cx.child_bounds(node));
        (0..2).rev().any(|index| {
            let mut pane_cx = cx.with_bounds(panes[index]);
            let content = self
                .content_node(index)
                .and_then(|node| pane_cx.child_bounds(node))
                .unwrap_or(panes[index]);
            self.children[index].handle_scroll_event(&mut pane_cx.with_bounds(content), event)
        })
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        if cx.is_focused(Some(self.id.get())) && event.modifiers == Modifiers::none() {
            let target = match event.key {
                KeyCode::Home => Some(self.min),
                KeyCode::End => Some(self.largest()),
                _ => None,
            };
            if let Some(target) = target {
                return self.set_position(target, cx);
            }
        }
        self.children
            .iter_mut()
            .rev()
            .any(|child| child.handle_key_event(cx, event))
    }

    fn dispatch_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        let outcome = self.handle_action(cx, action);
        if outcome.is_handled() {
            return outcome;
        }
        dispatch_action_to_children(&mut self.children, cx, action)
    }

    fn handle_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        if !cx.is_focused(Some(self.id.get())) {
            return ActionOutcome::Ignored;
        }
        let horizontal = self.axis == SplitAxis::Horizontal;
        match action {
            ActionId::Standard(StandardAction::MoveLeft) if horizontal => self.step(-KEY_STEP, cx),
            ActionId::Standard(StandardAction::MoveRight) if horizontal => self.step(KEY_STEP, cx),
            ActionId::Standard(StandardAction::MoveUp) if !horizontal => self.step(-KEY_STEP, cx),
            ActionId::Standard(StandardAction::MoveDown) if !horizontal => self.step(KEY_STEP, cx),
            ActionId::Custom(name) if name == ACCESSIBILITY_INCREMENT_ACTION => {
                self.step(KEY_STEP, cx)
            }
            ActionId::Custom(name) if name == ACCESSIBILITY_DECREMENT_ACTION => {
                self.step(-KEY_STEP, cx)
            }
            _ => ActionOutcome::Ignored,
        }
    }

    fn handle_text_input_event(&mut self, cx: &mut EventContext, event: &TextInputEvent) -> bool {
        self.children
            .iter_mut()
            .rev()
            .any(|child| child.handle_text_input_event(cx, event))
    }

    fn handle_text_input_command(
        &mut self,
        cx: &mut EventContext,
        command: &TextInputCommand,
    ) -> bool {
        if let Some(focused) = cx.focused_id()
            && let Some(child) = self
                .children
                .iter_mut()
                .find(|child| child.contains_id(focused))
        {
            return child.handle_text_input_command(cx, command);
        }
        false
    }

    fn text_input_snapshot(&self, focused: ElementId) -> Option<TextInputSnapshot> {
        self.children
            .iter()
            .find_map(|child| child.text_input_snapshot(focused))
    }

    fn children(&self) -> &[AnyElement] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut [AnyElement] {
        &mut self.children
    }
}

pub fn splitter(
    label: impl Into<String>,
    first: impl Into<AnyElement>,
    second: impl Into<AnyElement>,
) -> Splitter {
    Splitter::new(label, first, second)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::div;
    use std::cell::RefCell;
    use std::rc::Rc;
    use taffy::TaffyTree;

    #[test]
    fn advanced_ui_splitter_keys_move_within_limits() {
        let id = ElementId::new();
        let changes = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&changes);
        let mut split = Splitter::new("Sidebar", div(), div())
            .id(id)
            .position(100.0)
            .limits(80.0, 120.0)
            .on_change(move |at| sink.borrow_mut().push(at));
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = Some(id);
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 400.0, 200.0),
            &taffy,
            &mut focused,
        );

        let right = ActionId::Standard(StandardAction::MoveRight);
        assert!(split.handle_action(&mut cx, &right).is_handled());
        assert!(split.handle_action(&mut cx, &right).is_handled());
        assert!(split.handle_action(&mut cx, &right).is_handled());
        assert_eq!(split.current_position(), 120.0);
        let up = ActionId::Standard(StandardAction::MoveUp);
        assert!(!split.handle_action(&mut cx, &up).is_handled());
        assert!(split.handle_key_event(&mut cx, &KeyEvent::new(KeyCode::Home, Modifiers::none())));
        assert_eq!(split.current_position(), 80.0);
        assert_eq!(&*changes.borrow(), &[110.0, 120.0, 80.0]);
    }

    #[test]
    fn advanced_ui_splitter_accessibility_reports_position_and_limits() {
        let split = Splitter::new("Sidebar", div(), div())
            .position(150.0)
            .limits(100.0, 300.0);
        let nodes = split
            .accessibility_nodes(&AccessibilityContext::default())
            .expect("splitter nodes");
        let node = nodes
            .iter()
            .find(|node| node.a11y_role() == AccessibilityRole::Splitter)
            .expect("splitter node");

        assert_eq!(node.a11y_value(), Some("150"));
        let numeric = node.a11y_numeric_value().expect("numeric value");
        assert_eq!((numeric.min(), numeric.max()), (100.0, 300.0));
        assert!(
            node.a11y_actions()
                .contains(&AccessibilityAction::Increment)
        );
    }

    #[test]
    fn advanced_ui_splitter_disabled_does_not_move() {
        let id = ElementId::new();
        let mut split = Splitter::new("Sidebar", div(), div())
            .id(id)
            .position(100.0)
            .disabled(true);
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = Some(id);
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 400.0, 200.0),
            &taffy,
            &mut focused,
        );

        let right = ActionId::Standard(StandardAction::MoveRight);
        assert!(!split.handle_action(&mut cx, &right).is_handled());
        assert_eq!(split.current_position(), 100.0);
    }
}
