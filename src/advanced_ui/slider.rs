use crate::advanced_ui::state::{InteractionState, require_finite, require_non_empty};
use crate::advanced_ui::tokens::{ControlSize, ControlState, ControlVariant, Theme};
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
use crate::core::event::{KeyCode, KeyEvent, Modifiers, MouseButton};
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Corners, Style};
use crate::elements::element::StableId;
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    style_to_taffy,
};
use crate::renderer::Primitive;
use taffy::prelude::*;

const DEFAULT_WIDTH: f32 = 200.0;
const TRACK_HEIGHT: f32 = 4.0;

type SliderChangeHandler = Box<dyn Fn(f32)>;

/// A horizontal slider picking a number in `min..=max`, snapped to `step`.
pub struct Slider {
    id: StableId,
    label: String,
    value: f32,
    min: f32,
    max: f32,
    step: f32,
    width: f32,
    size: ControlSize,
    theme: Theme,
    state: InteractionState,
    dragging: bool,
    style: Style,
    on_change: Option<SliderChangeHandler>,
}

impl Slider {
    /// A slider from 0 to 100 in steps of 1. `label` names it for assistive
    /// technology; it is not drawn.
    pub fn new(label: impl Into<String>, value: f32) -> Self {
        let label = label.into();
        require_non_empty(&label, "slider accessibility label must not be empty");
        require_finite(value, "slider value must be finite");

        Self {
            id: StableId::new(),
            label,
            value,
            min: 0.0,
            max: 100.0,
            step: 1.0,
            width: DEFAULT_WIDTH,
            size: ControlSize::default(),
            theme: Theme::default(),
            state: InteractionState::default(),
            dragging: false,
            style: Style::new(),
            on_change: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = StableId::explicit(id);
        self
    }

    pub fn value(mut self, value: f32) -> Self {
        require_finite(value, "slider value must be finite");
        self.value = value;
        self
    }

    pub fn range(mut self, min: f32, max: f32) -> Self {
        require_finite(min, "slider range must be finite");
        require_finite(max, "slider range must be finite");
        assert!(min < max, "slider min must be less than max");
        self.min = min;
        self.max = max;
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        require_finite(step, "slider step must be positive");
        assert!(step > 0.0, "slider step must be positive");
        self.step = step;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        require_finite(width, "slider width must be finite");
        self.width = width.max(0.0);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.set_disabled(disabled);
        if disabled {
            self.dragging = false;
        }
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.set_read_only(read_only);
        if read_only {
            self.dragging = false;
        }
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
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

    pub fn current_value(&self) -> f32 {
        self.snap(self.value)
    }

    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    pub fn interaction_state(&self) -> InteractionState {
        self.state
    }

    /// Keep the drag and hover of the slider this one replaces, so a drag
    /// survives the render its own `on_change` causes.
    pub(crate) fn reconcile_transient_state(&mut self, previous: &Self) {
        if self.state.can_activate() {
            self.dragging = previous.dragging;
            self.state.set_hovered(previous.state.hovered());
            self.state.set_pressed(previous.state.pressed());
        }
    }

    fn snap(&self, value: f32) -> f32 {
        let steps = ((value.clamp(self.min, self.max) - self.min) / self.step).round();
        (self.min + steps * self.step).clamp(self.min, self.max)
    }

    fn fraction(&self) -> f32 {
        (self.current_value() - self.min) / (self.max - self.min)
    }

    fn thumb_extent(&self) -> f32 {
        self.theme.indicator_extent(self.size)
    }

    fn value_at(&self, bounds: Bounds, x: f32) -> f32 {
        let radius = self.thumb_extent() / 2.0;
        let travel = (bounds.width() - radius * 2.0).max(1.0);
        let fraction = ((x - bounds.x() - radius) / travel).clamp(0.0, 1.0);
        self.min + fraction * (self.max - self.min)
    }

    fn page_step(&self) -> f32 {
        self.step.max((self.max - self.min) / 10.0)
    }

    fn set_value(&mut self, value: f32, cx: &EventContext) -> bool {
        if !self.state.can_activate() {
            return false;
        }
        let value = self.snap(value);
        if value != self.current_value() {
            self.value = value;
            if let Some(handler) = &self.on_change {
                handler(value);
            }
            cx.request_redraw();
        }
        true
    }

    fn handled(&mut self, value: f32, cx: &EventContext) -> ActionOutcome {
        if self.set_value(value, cx) {
            ActionOutcome::handled("advanced_ui.slider")
        } else {
            ActionOutcome::Ignored
        }
    }
}

impl Element for Slider {
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
        let mut style = style_to_taffy(&self.style);
        style.size.width = Dimension::Length(self.width);
        style.size.height = Dimension::Length(self.theme.control_height(self.size));
        cx.new_leaf(style)
            .unwrap_or_else(|err| panic!("failed to create advanced slider layout node: {err}"))
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        cx.register_hit_region(self.id.get(), bounds);

        let mut state: ControlState = self.state.into();
        state.focused = cx.is_focused(self.id.get()) && !state.disabled;
        let colors = self.theme.control_colors(ControlVariant::Primary, state);
        let extent = self.thumb_extent();
        let radius = extent / 2.0;
        let center_y = bounds.y() + bounds.height() / 2.0;
        let track = Bounds::from_xywh(
            bounds.x() + radius,
            center_y - TRACK_HEIGHT / 2.0,
            (bounds.width() - extent).max(0.0),
            TRACK_HEIGHT,
        );
        paint_bar(cx, track, self.theme.colors.progress_track);
        let thumb_x = track.x() + track.width() * self.fraction();
        let fill = Bounds::from_xywh(track.x(), track.y(), thumb_x - track.x(), TRACK_HEIGHT);
        paint_bar(cx, fill, colors.background);

        let thumb = Bounds::from_xywh(thumb_x - radius, center_y - radius, extent, extent);
        cx.paint(Primitive::Quad {
            bounds: thumb,
            background: self.theme.surface_color_for_state(state).to_rgba(),
            border_color: self
                .theme
                .state_border_color(state, colors.background)
                .to_rgba(),
            border_widths: Edges::all(if state.focused { 2.0 } else { 1.0 }),
            corner_radii: Corners::all(radius),
        });
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let mut node = AccessibilityNode::label_required(
            self.id.get(),
            AccessibilityRole::Slider,
            &self.label,
        )?
        .value_required(format_number(self.current_value()))?
        .with_numeric_value(AccessibilityNumericValue::new(
            f64::from(self.current_value()),
            f64::from(self.min),
            f64::from(self.max),
        ))
        .with_enabled(!self.state.disabled())
        .with_read_only(self.state.read_only())
        .with_focused(cx.a11y_has_focus(self.id.get()));
        if self.state.can_activate() {
            node = node.with_actions([
                AccessibilityAction::Increment,
                AccessibilityAction::Decrement,
            ]);
        }
        Ok(Some(node))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let inside = cx.contains_pointer(event.position);
        match event.kind {
            PointerEventKind::Move => {
                if self.dragging && event.button == Some(MouseButton::Left) {
                    let value = self.value_at(cx.bounds(), event.position.x);
                    return self.set_value(value, cx);
                }
                self.dragging = false;
                self.state.update_hover_inside(inside, cx);
                false
            }
            PointerEventKind::Down => {
                if event.button != Some(MouseButton::Left) || !self.state.press_inside(inside, cx) {
                    return false;
                }
                cx.request_focus(Some(self.id.get()));
                self.dragging = true;
                let value = self.value_at(cx.bounds(), event.position.x);
                self.set_value(value, cx)
            }
            PointerEventKind::Up => {
                let was_dragging = std::mem::take(&mut self.dragging);
                self.state.release_inside(inside, cx);
                was_dragging
            }
        }
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        if !cx.is_focused(Some(self.id.get())) || event.modifiers != Modifiers::none() {
            return false;
        }
        let value = match event.key {
            KeyCode::PageUp => self.current_value() + self.page_step(),
            KeyCode::PageDown => self.current_value() - self.page_step(),
            KeyCode::Home => self.min,
            KeyCode::End => self.max,
            _ => return false,
        };
        self.set_value(value, cx)
    }

    fn handle_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        if !cx.is_focused(Some(self.id.get())) {
            return ActionOutcome::Ignored;
        }
        match action {
            ActionId::Standard(StandardAction::MoveRight | StandardAction::MoveUp) => {
                self.handled(self.current_value() + self.step, cx)
            }
            ActionId::Standard(StandardAction::MoveLeft | StandardAction::MoveDown) => {
                self.handled(self.current_value() - self.step, cx)
            }
            ActionId::Custom(name) if name == ACCESSIBILITY_INCREMENT_ACTION => {
                self.handled(self.current_value() + self.step, cx)
            }
            ActionId::Custom(name) if name == ACCESSIBILITY_DECREMENT_ACTION => {
                self.handled(self.current_value() - self.step, cx)
            }
            _ => ActionOutcome::Ignored,
        }
    }
}

fn paint_bar(cx: &mut PaintContext, bounds: Bounds, color: Color) {
    cx.paint(Primitive::Quad {
        bounds,
        background: color.to_rgba(),
        border_color: Color::TRANSPARENT.to_rgba(),
        border_widths: Edges::ZERO,
        corner_radii: Corners::all(bounds.height() / 2.0),
    });
}

/// `value` without trailing zeros, as a range control's accessibility value.
pub(crate) fn format_number(value: f32) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn slider(label: impl Into<String>, value: f32) -> Slider {
    Slider::new(label, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geometry::Point;
    use std::cell::RefCell;
    use std::rc::Rc;
    use taffy::TaffyTree;

    fn pointer(kind: PointerEventKind, x: f32) -> PointerEvent {
        PointerEvent {
            kind,
            position: Point::new(x, 12.0),
            button: Some(MouseButton::Left),
        }
    }

    fn recording() -> (Rc<RefCell<Vec<f32>>>, impl Fn(f32) + 'static) {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&changes);
        (changes, move |value| sink.borrow_mut().push(value))
    }

    #[test]
    fn advanced_ui_slider_click_and_drag_follow_the_pointer() {
        let (changes, handler) = recording();
        let mut slider = Slider::new("Volume", 0.0).on_change(handler);
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        // Track runs from x = 8 to x = 208 with a 16pt thumb.
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 216.0, 32.0),
            &taffy,
            &mut focused,
        );

        assert!(slider.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 108.0)));
        assert_eq!(slider.current_value(), 50.0);
        assert!(slider.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Move, 400.0)));
        assert_eq!(slider.current_value(), 100.0);
        assert!(slider.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Up, 400.0)));
        assert!(!slider.is_dragging());
        assert_eq!(&*changes.borrow(), &[50.0, 100.0]);
        assert_eq!(cx.focused_id(), Some(slider.id.get()));
    }

    #[test]
    fn advanced_ui_slider_keys_step_page_and_jump_to_ends() {
        let id = ElementId::new();
        let mut slider = Slider::new("Volume", 50.0)
            .id(id)
            .range(0.0, 200.0)
            .step(5.0);
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = Some(id);
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 216.0, 32.0),
            &taffy,
            &mut focused,
        );

        let right = ActionId::Standard(StandardAction::MoveRight);
        assert!(slider.handle_action(&mut cx, &right).is_handled());
        assert_eq!(slider.current_value(), 55.0);
        let down = ActionId::Standard(StandardAction::MoveDown);
        assert!(slider.handle_action(&mut cx, &down).is_handled());
        assert_eq!(slider.current_value(), 50.0);
        let page_up = KeyEvent::new(KeyCode::PageUp, Modifiers::none());
        assert!(slider.handle_key_event(&mut cx, &page_up));
        assert_eq!(slider.current_value(), 70.0);
        assert!(slider.handle_key_event(&mut cx, &KeyEvent::new(KeyCode::End, Modifiers::none())));
        assert_eq!(slider.current_value(), 200.0);
        assert!(slider.handle_key_event(&mut cx, &KeyEvent::new(KeyCode::Home, Modifiers::none())));
        assert_eq!(slider.current_value(), 0.0);
        let increment = ActionId::custom(ACCESSIBILITY_INCREMENT_ACTION);
        assert!(slider.handle_action(&mut cx, &increment).is_handled());
        assert_eq!(slider.current_value(), 5.0);
    }

    #[test]
    fn advanced_ui_slider_disabled_ignores_pointer_and_keys() {
        let id = ElementId::new();
        let (changes, handler) = recording();
        let mut slider = Slider::new("Volume", 10.0)
            .id(id)
            .disabled(true)
            .on_change(handler);
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = Some(id);
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 216.0, 32.0),
            &taffy,
            &mut focused,
        );

        assert!(!slider.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 200.0)));
        let right = ActionId::Standard(StandardAction::MoveRight);
        assert!(!slider.handle_action(&mut cx, &right).is_handled());
        assert_eq!(slider.current_value(), 10.0);
        assert!(changes.borrow().is_empty());
    }

    #[test]
    fn advanced_ui_slider_accessibility_reports_range_and_actions() {
        let slider = Slider::new("Opacity", 0.25).range(0.0, 1.0).step(0.05);
        let node = slider
            .accessibility(&AccessibilityContext::default())
            .expect("slider node")
            .expect("slider exposes a node");

        assert_eq!(node.a11y_role(), AccessibilityRole::Slider);
        assert_eq!(node.a11y_label(), Some("Opacity"));
        assert_eq!(node.a11y_value(), Some("0.25"));
        let numeric = node.a11y_numeric_value().expect("numeric value");
        assert_eq!((numeric.min(), numeric.max()), (0.0, 1.0));
        assert!(
            node.a11y_actions()
                .contains(&AccessibilityAction::Increment)
        );
        assert!(
            node.a11y_actions()
                .contains(&AccessibilityAction::Decrement)
        );
    }

    #[test]
    fn advanced_ui_slider_snaps_and_clamps_values() {
        assert_eq!(Slider::new("Volume", 42.4).current_value(), 42.0);
        assert_eq!(Slider::new("Volume", 420.0).current_value(), 100.0);
        assert_eq!(Slider::new("Volume", 7.0).step(5.0).current_value(), 5.0);
    }
}
