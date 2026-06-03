use crate::advanced_ui::state::{InteractionState, require_non_empty};
use crate::advanced_ui::tokens::{ControlSize, ControlState, ControlVariant, Theme};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityRole,
};
use crate::core::event::Cursor;
use crate::core::geometry::Edges;
use crate::core::style::{Corners, Style};
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    style_to_taffy,
};
use crate::renderer::Primitive;
use taffy::prelude::*;

pub struct Button {
    id: ElementId,
    label: String,
    variant: ControlVariant,
    size: ControlSize,
    theme: Theme,
    state: InteractionState,
    style: Style,
    on_click: Option<Box<dyn Fn()>>,
}

impl Button {
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        require_non_empty(&label, "advanced button label must not be empty");

        let theme = Theme::default();
        let mut style = Style::new();
        style.border.radius = Corners::all(theme.control_radius());

        Self {
            id: ElementId::new(),
            label,
            variant: ControlVariant::default(),
            size: ControlSize::default(),
            theme,
            state: InteractionState::default(),
            style,
            on_click: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self
    }

    pub fn variant(mut self, variant: ControlVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn primary(self) -> Self {
        self.variant(ControlVariant::Primary)
    }

    pub fn secondary(self) -> Self {
        self.variant(ControlVariant::Secondary)
    }

    pub fn outline(self) -> Self {
        self.variant(ControlVariant::Outline)
    }

    pub fn ghost(self) -> Self {
        self.variant(ControlVariant::Ghost)
    }

    pub fn danger(self) -> Self {
        self.variant(ControlVariant::Danger)
    }

    pub fn success(self) -> Self {
        self.variant(ControlVariant::Success)
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self.style.border.radius = Corners::all(self.theme.control_radius());
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

    pub fn on_click(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn state(&self) -> ControlState {
        self.state.into()
    }

    pub fn interaction_state(&self) -> InteractionState {
        self.state
    }

    pub fn cursor(&self) -> Cursor {
        self.state.cursor()
    }

    fn preferred_width(&self) -> f32 {
        let text_width = self.label.chars().count() as f32 * self.theme.text_size(self.size) * 0.56;
        text_width + self.theme.horizontal_padding(self.size) * 2.0
    }
}

impl Element for Button {
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
            Err(err) => panic!("failed to create advanced button layout node: {}", err),
        }
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        cx.register_hit_region(self.id, bounds);

        let colors = self.theme.control_colors(self.variant, self.state.into());
        let border_widths = if matches!(self.variant, ControlVariant::Outline) {
            Edges::all(1.0)
        } else {
            Edges::ZERO
        };

        cx.paint(Primitive::Quad {
            bounds,
            background: colors.background.to_rgba(),
            border_color: colors.border.to_rgba(),
            border_widths,
            corner_radii: self.style.border.radius,
        });

        cx.paint(Primitive::Text {
            bounds,
            content: self.label.clone(),
            color: colors.foreground.to_rgba(),
            font_size: self.theme.text_size(self.size),
            font_weight: self.theme.typography.control_weight,
            font_family: None,
            line_height: self.theme.typography.line_height,
            align: crate::elements::text::TextAlign::Center,
        });
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let mut node =
            AccessibilityNode::label_required(self.id, AccessibilityRole::Button, &self.label)?
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
                    if let Some(handler) = &self.on_click {
                        handler();
                    }
                    cx.announce_accessibility_action(self.id, format!("{} activated", self.label));
                    true
                } else {
                    false
                }
            }
        }
    }
}

pub fn button(label: impl Into<String>) -> Button {
    Button::new(label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::MouseButton;
    use crate::core::geometry::{Bounds, Point, Size};
    use std::cell::Cell;
    use std::rc::Rc;
    use taffy::TaffyTree;

    fn event(kind: PointerEventKind, x: f32, y: f32) -> PointerEvent {
        PointerEvent {
            kind,
            position: Point::new(x, y),
            button: Some(MouseButton::Left),
        }
    }

    #[test]
    fn advanced_ui_button_invokes_click_after_press_release() {
        let clicked = Rc::new(Cell::new(false));
        let clicked_ref = Rc::clone(&clicked);
        let mut button = Button::new("Save").on_click(move || clicked_ref.set(true));
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 80.0, 36.0),
            &taffy,
            &mut focused,
        );

        assert!(button.handle_pointer_event(&mut cx, &event(PointerEventKind::Down, 4.0, 4.0)));
        assert!(button.state().pressed);
        assert!(button.handle_pointer_event(&mut cx, &event(PointerEventKind::Up, 4.0, 4.0)));
        assert!(clicked.get());
        assert!(!button.state().pressed);
    }

    #[test]
    fn advanced_ui_button_disabled_does_not_click() {
        let clicked = Rc::new(Cell::new(false));
        let clicked_ref = Rc::clone(&clicked);
        let mut button = Button::new("Save")
            .disabled(true)
            .on_click(move || clicked_ref.set(true));
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 80.0, 36.0),
            &taffy,
            &mut focused,
        );

        assert!(!button.handle_pointer_event(&mut cx, &event(PointerEventKind::Down, 4.0, 4.0)));
        assert!(!button.handle_pointer_event(&mut cx, &event(PointerEventKind::Up, 4.0, 4.0)));
        assert!(!clicked.get());
        assert_eq!(button.cursor(), Cursor::NotAllowed);
    }

    #[test]
    fn advanced_ui_button_read_only_does_not_click() {
        let clicked = Rc::new(Cell::new(false));
        let clicked_ref = Rc::clone(&clicked);
        let mut button = Button::new("Save")
            .read_only(true)
            .on_click(move || clicked_ref.set(true));
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 80.0, 36.0),
            &taffy,
            &mut focused,
        );

        assert!(!button.handle_pointer_event(&mut cx, &event(PointerEventKind::Down, 4.0, 4.0)));
        assert!(!button.handle_pointer_event(&mut cx, &event(PointerEventKind::Up, 4.0, 4.0)));
        assert!(!clicked.get());
        assert_eq!(button.cursor(), Cursor::Default);
    }

    #[test]
    fn advanced_ui_button_disabled_clears_hover_and_active() {
        let mut button = Button::new("Save");
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 80.0, 36.0),
            &taffy,
            &mut focused,
        );

        button.handle_pointer_event(&mut cx, &event(PointerEventKind::Move, 4.0, 4.0));
        button.handle_pointer_event(&mut cx, &event(PointerEventKind::Down, 4.0, 4.0));
        assert!(button.interaction_state().hovered());
        assert!(button.interaction_state().pressed());

        button = button.disabled(true);

        assert!(!button.interaction_state().hovered());
        assert!(!button.interaction_state().pressed());
    }

    #[test]
    #[should_panic(expected = "advanced button label must not be empty")]
    fn advanced_ui_button_rejects_empty_label() {
        drop(Button::new(" "));
    }

    #[test]
    fn advanced_ui_button_layout_uses_control_size() {
        let mut button = Button::new("Save").size(ControlSize::Large);
        let mut taffy = TaffyTree::<ElementId>::new();
        let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(200.0, 60.0));
        let node = button.layout(&mut layout_cx);

        if let Err(err) = taffy.compute_layout(
            node,
            taffy::Size {
                width: taffy::prelude::AvailableSpace::Definite(200.0),
                height: taffy::prelude::AvailableSpace::Definite(60.0),
            },
        ) {
            panic!("layout should compute: {}", err);
        }
        assert_eq!(
            taffy.layout(node).map(|layout| layout.size.height),
            Ok(44.0)
        );
    }
}
