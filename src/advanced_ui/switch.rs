use crate::advanced_ui::state::{InteractionState, require_non_empty};
use crate::advanced_ui::tokens::{ControlSize, ControlState, ControlVariant, Theme};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityRole,
};
use crate::core::action::{ActionId, ActionOutcome, StandardAction};
use crate::core::event::{KeyCode, KeyEvent, Modifiers};
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Corners, Style};
use crate::elements::element::StableId;
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    style_to_taffy,
};
use crate::renderer::Primitive;
use crate::renderer::text::BaseDirection;
use taffy::prelude::*;

/// An on/off toggle with a sliding knob, followed by its label.
pub struct Switch {
    id: StableId,
    label: String,
    on: bool,
    size: ControlSize,
    theme: Theme,
    state: InteractionState,
    style: Style,
    on_change: Option<Box<dyn Fn(bool)>>,
}

impl Switch {
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        require_non_empty(&label, "switch label must not be empty");

        Self {
            id: StableId::new(),
            label,
            on: false,
            size: ControlSize::default(),
            theme: Theme::default(),
            state: InteractionState::default(),
            style: Style::new(),
            on_change: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = StableId::explicit(id);
        self
    }

    pub fn on(mut self, on: bool) -> Self {
        self.on = on;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.set_disabled(disabled);
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.set_read_only(read_only);
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

    pub fn on_change(mut self, handler: impl Fn(bool) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    pub fn interaction_state(&self) -> InteractionState {
        self.state
    }

    fn track_size(&self) -> (f32, f32) {
        let height = self.theme.indicator_extent(self.size) + 4.0;
        (height * 1.75, height)
    }

    fn toggle(&mut self, cx: &EventContext) {
        self.on = !self.on;
        if let Some(handler) = &self.on_change {
            handler(self.on);
        }
        let state = if self.on { "on" } else { "off" };
        cx.announce_accessibility_action(self.id.get(), format!("{} {state}", self.label));
        cx.request_redraw();
    }
}

impl Element for Switch {
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
        let label = self.theme.measure_label(
            cx,
            &self.label,
            self.size,
            self.theme.typography.label_weight,
        );
        let mut style = style_to_taffy(&self.style);
        style.size.width =
            Dimension::Length(self.track_size().0 + self.theme.control_gap() + label);
        style.size.height = Dimension::Length(self.theme.control_height(self.size));
        cx.new_leaf(style)
            .unwrap_or_else(|err| panic!("failed to create advanced switch layout node: {err}"))
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        cx.register_hit_region(self.id.get(), bounds);

        let mut state: ControlState = self.state.into();
        state.selected = self.on;
        state.focused = cx.is_focused(self.id.get()) && !state.disabled;
        let colors = self.theme.control_colors(ControlVariant::Primary, state);
        let (width, height) = self.track_size();
        let track = Bounds::from_xywh(
            bounds.x(),
            bounds.y() + (bounds.height() - height) / 2.0,
            width,
            height,
        );
        let track_color = if self.on {
            colors.background
        } else {
            self.theme.colors.progress_track
        };
        cx.paint(Primitive::Quad {
            bounds: track,
            background: track_color.to_rgba(),
            border_color: self.theme.state_border_color(state, track_color).to_rgba(),
            border_widths: Edges::all(if state.focused { 2.0 } else { 1.0 }),
            corner_radii: Corners::all(height / 2.0),
        });

        let knob = height - 4.0;
        let knob_x = if self.on {
            track.x() + width - knob - 2.0
        } else {
            track.x() + 2.0
        };
        cx.paint(Primitive::Quad {
            bounds: Bounds::from_xywh(knob_x, track.y() + 2.0, knob, knob),
            background: self.theme.surface_color_for_state(state).to_rgba(),
            border_color: self.theme.colors.border.to_rgba(),
            border_widths: Edges::all(1.0),
            corner_radii: Corners::all(knob / 2.0),
        });

        let gap = self.theme.control_gap();
        let mut label_color = self.theme.colors.text;
        if state.disabled {
            label_color = label_color.with_alpha(self.theme.state.disabled_opacity);
        } else if state.read_only {
            label_color = label_color.with_alpha(self.theme.state.read_only_opacity);
        }
        cx.paint(Primitive::Text {
            bounds: Bounds::from_xywh(
                bounds.x() + width + gap,
                bounds.y(),
                (bounds.width() - width - gap).max(0.0),
                bounds.height(),
            ),
            content: self.label.clone().into(),
            color: label_color.to_rgba(),
            font_size: self.theme.text_size(self.size),
            font_weight: self.theme.typography.label_weight,
            font_family: None,
            line_height: self.theme.typography.line_height,
            align: crate::elements::text::TextAlign::Left,
            direction: BaseDirection::Auto,
        });
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let mut node = AccessibilityNode::label_required(
            self.id.get(),
            AccessibilityRole::Switch,
            &self.label,
        )?
        .value_required(if self.on { "on" } else { "off" })?
        .with_checked(self.on)
        .with_enabled(!self.state.disabled())
        .with_read_only(self.state.read_only())
        .with_focused(cx.a11y_has_focus(self.id.get()));
        if self.state.can_activate() {
            node = node.with_action(AccessibilityAction::Activate);
        }
        Ok(Some(node))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let inside = cx.contains_pointer(event.position);
        match event.kind {
            PointerEventKind::Move => {
                self.state.update_hover_inside(inside, cx);
                false
            }
            PointerEventKind::Down => {
                let pressed = self.state.press_inside(inside, cx);
                if pressed {
                    cx.request_focus(Some(self.id.get()));
                }
                pressed
            }
            PointerEventKind::Up => {
                if self.state.release_inside(inside, cx).activated {
                    self.toggle(cx);
                    true
                } else {
                    false
                }
            }
        }
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        if event.key == KeyCode::Space && event.modifiers == Modifiers::none() && !event.is_repeat {
            return self
                .dispatch_action(cx, &StandardAction::Activate.into())
                .is_handled();
        }
        false
    }

    fn handle_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        if !cx.is_focused(Some(self.id.get())) || !self.state.can_activate() {
            return ActionOutcome::Ignored;
        }
        match action {
            ActionId::Standard(StandardAction::Activate) => {
                self.toggle(cx);
                ActionOutcome::handled("advanced_ui.switch")
            }
            _ => ActionOutcome::Ignored,
        }
    }
}

pub fn switch(label: impl Into<String>) -> Switch {
    Switch::new(label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::MouseButton;
    use crate::core::geometry::Point;
    use std::cell::RefCell;
    use std::rc::Rc;
    use taffy::TaffyTree;

    fn pointer(kind: PointerEventKind) -> PointerEvent {
        PointerEvent {
            kind,
            position: Point::new(8.0, 12.0),
            button: Some(MouseButton::Left),
        }
    }

    #[test]
    fn advanced_ui_switch_toggles_on_click_and_space() {
        let changes = Rc::new(RefCell::new(Vec::new()));
        let sink = Rc::clone(&changes);
        let id = ElementId::new();
        let mut switch = Switch::new("Wi-Fi")
            .id(id)
            .on_change(move |on| sink.borrow_mut().push(on));
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 120.0, 32.0),
            &taffy,
            &mut focused,
        );

        assert!(switch.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down)));
        assert!(switch.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Up)));
        assert!(switch.is_on());
        assert_eq!(cx.focused_id(), Some(id));
        let space = KeyEvent::new(KeyCode::Space, Modifiers::none());
        assert!(switch.handle_key_event(&mut cx, &space));
        assert!(!switch.is_on());
        assert_eq!(&*changes.borrow(), &[true, false]);
    }

    #[test]
    fn advanced_ui_switch_disabled_does_not_toggle() {
        let id = ElementId::new();
        let mut switch = Switch::new("Wi-Fi").id(id).disabled(true);
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = Some(id);
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 120.0, 32.0),
            &taffy,
            &mut focused,
        );

        assert!(!switch.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down)));
        let space = KeyEvent::new(KeyCode::Space, Modifiers::none());
        assert!(!switch.handle_key_event(&mut cx, &space));
        assert!(!switch.is_on());
    }

    #[test]
    fn advanced_ui_switch_accessibility_is_a_checked_switch() {
        let node = Switch::new("Wi-Fi")
            .on(true)
            .accessibility(&AccessibilityContext::default())
            .expect("switch node")
            .expect("switch exposes a node");

        assert_eq!(node.a11y_role(), AccessibilityRole::Switch);
        assert_eq!(node.a11y_checked(), Some(true));
        assert_eq!(node.a11y_value(), Some("on"));
        assert_eq!(node.a11y_actions(), &[AccessibilityAction::Activate]);
    }
}
