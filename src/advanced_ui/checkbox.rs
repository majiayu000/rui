use crate::advanced_ui::state::{InteractionState, require_non_empty};
use crate::advanced_ui::tokens::{ControlSize, ControlState, ControlVariant, Theme};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityRole,
};
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Corners, Style};
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    style_to_taffy,
};
use crate::renderer::Primitive;
use taffy::prelude::*;

pub struct Checkbox {
    id: ElementId,
    label: String,
    checked: bool,
    size: ControlSize,
    theme: Theme,
    state: InteractionState,
    style: Style,
    on_change: Option<Box<dyn Fn(bool)>>,
}

impl Checkbox {
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        require_non_empty(&label, "advanced checkbox label must not be empty");

        Self {
            id: ElementId::new(),
            label,
            checked: false,
            size: ControlSize::default(),
            theme: Theme::default(),
            state: InteractionState::default(),
            style: Style::new(),
            on_change: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
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

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.state.set_invalid(invalid);
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

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn state(&self) -> ControlState {
        let mut state = self.state;
        state.set_selected(self.checked);
        state.into()
    }

    pub fn interaction_state(&self) -> InteractionState {
        self.state
    }

    fn preferred_width(&self) -> f32 {
        self.theme.indicator_extent(self.size)
            + self.theme.control_gap()
            + self.label.chars().count() as f32 * self.theme.text_size(self.size) * 0.55
    }
}

impl Element for Checkbox {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let mut style = style_to_taffy(&self.style);
        style.size.width = Dimension::Length(self.preferred_width());
        style.size.height = Dimension::Length(self.theme.control_height(self.size));

        match cx.taffy.new_leaf(style) {
            Ok(node) => node,
            Err(err) => panic!("failed to create advanced checkbox layout node: {}", err),
        }
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        cx.register_hit_region(self.id, bounds);

        let control_state = self.state();
        let indicator = self.theme.indicator_extent(self.size);
        let indicator_y = bounds.y() + (bounds.height() - indicator) / 2.0;
        let box_bounds = Bounds::from_xywh(bounds.x(), indicator_y, indicator, indicator);
        let colors = self
            .theme
            .control_colors(ControlVariant::Primary, control_state);

        let box_background = if self.checked {
            colors.background
        } else {
            self.theme.surface_color_for_state(control_state)
        };
        let default_border = if self.checked {
            colors.background
        } else {
            self.theme.colors.border
        };

        cx.paint(Primitive::Quad {
            bounds: box_bounds,
            background: box_background.to_rgba(),
            border_color: self
                .theme
                .state_border_color(control_state, default_border)
                .to_rgba(),
            border_widths: Edges::all(1.0),
            corner_radii: Corners::all(self.theme.indicator_radius()),
        });

        if self.checked {
            cx.paint(Primitive::Text {
                bounds: box_bounds,
                content: "x".to_string(),
                color: colors.foreground.to_rgba(),
                font_size: self.theme.text_size(self.size),
                font_weight: self.theme.typography.selected_weight,
                font_family: None,
                line_height: 1.0,
                align: crate::elements::text::TextAlign::Center,
            });
        }

        let gap = self.theme.control_gap();
        let label_bounds = Bounds::from_xywh(
            bounds.x() + indicator + gap,
            bounds.y(),
            (bounds.width() - indicator - gap).max(0.0),
            bounds.height(),
        );
        let mut label_color = self.theme.colors.text;
        if control_state.disabled {
            label_color = label_color.with_alpha(self.theme.state.disabled_opacity);
        } else if control_state.read_only {
            label_color = label_color.with_alpha(self.theme.state.read_only_opacity);
        }
        cx.paint(Primitive::Text {
            bounds: label_bounds,
            content: self.label.clone(),
            color: label_color.to_rgba(),
            font_size: self.theme.text_size(self.size),
            font_weight: self.theme.typography.label_weight,
            font_family: None,
            line_height: self.theme.typography.line_height,
            align: crate::elements::text::TextAlign::Left,
        });
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let value = if self.checked { "checked" } else { "unchecked" };
        let mut node =
            AccessibilityNode::label_required(self.id, AccessibilityRole::Checkbox, &self.label)?
                .value_required(value)?
                .with_checked(self.checked)
                .with_enabled(!self.state.disabled())
                .with_read_only(self.state.read_only())
                .with_invalid(self.state.invalid())
                .with_focused(cx.a11y_has_focus(self.id));
        if self.state.can_activate() {
            node = node.with_action(AccessibilityAction::Activate);
        }
        Ok(Some(node))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let inside = cx.bounds().contains(event.position);
        match event.kind {
            PointerEventKind::Move => {
                self.state.update_hover(cx.bounds(), event.position, cx);
                false
            }
            PointerEventKind::Down => self.state.press_inside(inside, cx),
            PointerEventKind::Up => {
                let release = self.state.release_inside(inside, cx);
                if release.activated {
                    self.checked = !self.checked;
                    if let Some(handler) = &self.on_change {
                        handler(self.checked);
                    }
                    let message = if self.checked {
                        format!("{} checked", self.label)
                    } else {
                        format!("{} unchecked", self.label)
                    };
                    cx.announce_accessibility_action(self.id, message);
                    cx.request_redraw();
                    true
                } else {
                    false
                }
            }
        }
    }
}

pub fn checkbox(label: impl Into<String>) -> Checkbox {
    Checkbox::new(label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::MouseButton;
    use crate::core::geometry::Point;
    use std::cell::Cell;
    use std::rc::Rc;
    use taffy::TaffyTree;

    fn pointer(kind: PointerEventKind) -> PointerEvent {
        PointerEvent {
            kind,
            position: Point::new(4.0, 4.0),
            button: Some(MouseButton::Left),
        }
    }

    #[test]
    fn advanced_ui_checkbox_toggles_and_reports_change() {
        let latest = Rc::new(Cell::new(false));
        let latest_ref = Rc::clone(&latest);
        let mut checkbox =
            Checkbox::new("Enable").on_change(move |checked| latest_ref.set(checked));
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 120.0, 36.0),
            &taffy,
            &mut focused,
        );

        assert!(checkbox.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down)));
        assert!(checkbox.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Up)));
        assert!(checkbox.is_checked());
        assert!(latest.get());
    }

    #[test]
    fn advanced_ui_checkbox_disabled_does_not_toggle() {
        let mut checkbox = Checkbox::new("Enable").disabled(true);
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 120.0, 36.0),
            &taffy,
            &mut focused,
        );

        assert!(!checkbox.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down)));
        assert!(!checkbox.is_checked());
    }

    #[test]
    fn advanced_ui_checkbox_read_only_does_not_toggle() {
        let latest = Rc::new(Cell::new(false));
        let latest_ref = Rc::clone(&latest);
        let mut checkbox = Checkbox::new("Enable")
            .read_only(true)
            .on_change(move |checked| latest_ref.set(checked));
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 120.0, 36.0),
            &taffy,
            &mut focused,
        );

        assert!(!checkbox.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down)));
        assert!(!checkbox.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Up)));
        assert!(!checkbox.is_checked());
        assert!(!latest.get());
    }

    #[test]
    fn advanced_ui_checkbox_disabled_clears_hover_and_active() {
        let mut checkbox = Checkbox::new("Enable");
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 120.0, 36.0),
            &taffy,
            &mut focused,
        );

        checkbox.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Move));
        checkbox.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down));
        assert!(checkbox.interaction_state().hovered());
        assert!(checkbox.interaction_state().pressed());

        checkbox = checkbox.disabled(true);

        assert!(!checkbox.interaction_state().hovered());
        assert!(!checkbox.interaction_state().pressed());
    }

    #[test]
    #[should_panic(expected = "advanced checkbox label must not be empty")]
    fn advanced_ui_checkbox_rejects_empty_label() {
        drop(Checkbox::new(""));
    }
}
