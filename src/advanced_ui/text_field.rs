use crate::advanced_ui::state::{InteractionState, require_non_empty};
use crate::advanced_ui::tokens::{ControlSize, Theme};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityRole,
};
use crate::core::event::{Cursor, KeyEvent};
use crate::core::geometry::Edges;
use crate::core::style::Style;
use crate::core::text_editing::{TextEditError, TextEditOutcome, TextInputEvent};
use crate::elements::element::{Element, EventContext, LayoutContext, PaintContext, PointerEvent};
use crate::elements::{Input, InputType};
use crate::renderer::Primitive;
use taffy::prelude::NodeId;

pub struct TextField {
    id: ElementId,
    label: String,
    input: Input,
    size: ControlSize,
    theme: Theme,
    state: InteractionState,
}

impl TextField {
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        require_non_empty(&label, "advanced text field label must not be empty");

        let id = ElementId::new();
        let size = ControlSize::default();
        let theme = Theme::default();
        Self {
            id,
            input: Input::new()
                .id(id)
                .accessibility_label(label.clone())
                .h(theme.control_height(size))
                .rounded(theme.control_radius()),
            label,
            size,
            theme,
            state: InteractionState::default(),
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self.input = self.input.id(id);
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.input = self.input.placeholder(placeholder);
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.input = self.input.value(value);
        self
    }

    pub fn input_type(mut self, input_type: InputType) -> Self {
        self.input = self.input.input_type(input_type);
        self
    }

    pub fn password(self) -> Self {
        self.input_type(InputType::Password)
    }

    pub fn email(self) -> Self {
        self.input_type(InputType::Email)
    }

    pub fn number(self) -> Self {
        self.input_type(InputType::Number)
    }

    pub fn search(self) -> Self {
        self.input_type(InputType::Search)
    }

    pub fn w(mut self, width: f32) -> Self {
        self.input = self.input.w(width);
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self.input = self.input.h(self.theme.control_height(size));
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self.input = self
            .input
            .h(self.theme.control_height(self.size))
            .rounded(self.theme.control_radius());
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

    pub fn on_change(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.input = self.input.on_change(handler);
        self
    }

    pub fn on_submit(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.input = self.input.on_submit(handler);
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn get_value(&self) -> &str {
        self.input.get_value()
    }

    pub fn interaction_state(&self) -> InteractionState {
        self.state
    }

    pub fn cursor(&self) -> Cursor {
        if self.state.disabled() {
            Cursor::NotAllowed
        } else if self.state.read_only() {
            Cursor::Default
        } else {
            Cursor::Text
        }
    }

    pub fn apply_text_input_event(
        &mut self,
        event: TextInputEvent,
    ) -> Result<TextEditOutcome, TextEditError> {
        if !self.can_edit() {
            return Ok(TextEditOutcome::default());
        }
        self.input.apply_text_input_event(event)
    }

    pub fn apply_key_event(&mut self, event: &KeyEvent) -> Result<TextEditOutcome, TextEditError> {
        if !self.can_edit() {
            return Ok(TextEditOutcome::default());
        }
        self.input.apply_key_event(event)
    }

    fn can_edit(&self) -> bool {
        !self.state.disabled() && !self.state.read_only()
    }
}

impl Element for TextField {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        self.input.style()
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.input.layout(cx)
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        cx.register_hit_region(self.id, bounds);
        self.input.paint(cx);

        if self.state.invalid() {
            cx.paint(Primitive::Quad {
                bounds,
                background: crate::core::color::Rgba::TRANSPARENT,
                border_color: self
                    .theme
                    .state_border_color(self.state.into(), self.theme.colors.border)
                    .to_rgba(),
                border_widths: Edges::all(1.0),
                corner_radii: self.input.style().border.radius,
            });
        }
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let Some(input_node) = self.input.accessibility(cx)? else {
            return Ok(None);
        };

        let mut node =
            AccessibilityNode::label_required(self.id, AccessibilityRole::TextInput, &self.label)?
                .with_enabled(!self.state.disabled())
                .with_read_only(self.state.read_only())
                .with_invalid(self.state.invalid())
                .with_focused(input_node.a11y_focused());

        if let Some(value) = input_node.a11y_value() {
            node = node.with_value(value.to_string());
        }
        if let Some(caret) = input_node.a11y_text_caret() {
            node = node.with_text_caret(caret);
        }
        if let Some(selection) = input_node.a11y_text_selection() {
            node = node.with_text_selection(selection);
        }
        if let Some(composition) = input_node.a11y_text_composition() {
            node = node.with_text_composition(composition);
        }
        if self.can_edit() {
            node = node.with_action(AccessibilityAction::SetValue);
        }

        Ok(Some(node))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        if self.state.disabled() {
            if self.state.hovered() {
                self.state.set_hovered(false);
                cx.request_redraw();
            }
            return false;
        }

        let inside = cx.bounds().contains(event.position);
        if matches!(event.kind, crate::elements::element::PointerEventKind::Move) {
            if self.state.hovered() != inside {
                self.state.set_hovered(inside);
                cx.request_redraw();
            }
            if inside {
                cx.set_cursor(self.cursor());
            }
        }

        let handled = self.input.handle_pointer_event(cx, event);
        self.state.set_focused(cx.is_focused(Some(self.id)));
        handled
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        if !self.can_edit() {
            return false;
        }
        self.input.handle_key_event(cx, event)
    }
}

pub fn text_field(label: impl Into<String>) -> TextField {
    TextField::new(label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advanced_ui::tokens::ThemeDensity;
    use crate::core::accessibility::AccessibilityTextRange;
    use crate::core::color::Color;
    use crate::core::event::{KeyCode, Modifiers, MouseButton};
    use crate::core::geometry::{Bounds, Point, Size};
    use crate::renderer::Scene;
    use std::cell::RefCell;
    use std::rc::Rc;
    use taffy::prelude::{AvailableSpace, TaffyTree};

    fn pointer(kind: crate::elements::element::PointerEventKind) -> PointerEvent {
        PointerEvent {
            kind,
            position: Point::new(4.0, 4.0),
            button: Some(MouseButton::Left),
        }
    }

    #[test]
    fn advanced_ui_text_field_applies_input_events_and_reports_change() {
        let changes = Rc::new(RefCell::new(Vec::<String>::new()));
        let changes_ref = Rc::clone(&changes);
        let mut field = TextField::new("Name")
            .on_change(move |value| changes_ref.borrow_mut().push(value.to_string()));

        let outcome = field
            .apply_text_input_event(TextInputEvent::InsertText("RUI".to_string()))
            .expect("text input should apply");

        assert!(outcome.changed);
        assert_eq!(field.get_value(), "RUI");
        assert_eq!(&*changes.borrow(), &["RUI".to_string()]);
    }

    #[test]
    fn advanced_ui_text_field_blocks_disabled_and_read_only_editing() {
        let changes = Rc::new(RefCell::new(Vec::<String>::new()));
        let disabled_changes = Rc::clone(&changes);
        let mut disabled = TextField::new("Disabled")
            .value("fixed")
            .disabled(true)
            .on_change(move |value| disabled_changes.borrow_mut().push(value.to_string()));

        let outcome = disabled
            .apply_text_input_event(TextInputEvent::InsertText("x".to_string()))
            .expect("disabled edit should be ignored");
        assert!(!outcome.changed);
        assert_eq!(disabled.get_value(), "fixed");
        assert_eq!(disabled.cursor(), Cursor::NotAllowed);

        let mut read_only = TextField::new("Read only").value("fixed").read_only(true);
        let outcome = read_only
            .apply_key_event(&KeyEvent::new(KeyCode::Backspace, Modifiers::none()))
            .expect("read-only edit should be ignored");
        assert!(!outcome.changed);
        assert_eq!(read_only.get_value(), "fixed");
        assert_eq!(read_only.cursor(), Cursor::Default);
        assert!(changes.borrow().is_empty());
    }

    #[test]
    fn advanced_ui_text_field_disabled_pointer_does_not_focus() {
        let mut field = TextField::new("Name").disabled(true);
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 160.0, 36.0),
            &taffy,
            &mut focused,
        );

        assert!(!field.handle_pointer_event(
            &mut cx,
            &pointer(crate::elements::element::PointerEventKind::Down)
        ));
        assert_eq!(cx.focused_id(), None);
    }

    #[test]
    fn advanced_ui_text_field_layout_uses_shared_control_size() {
        let mut field = TextField::new("Search").size(ControlSize::Large).w(260.0);
        let mut taffy = TaffyTree::<ElementId>::new();
        let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(300.0, 80.0));
        let node = field.layout(&mut layout_cx);

        taffy
            .compute_layout(
                node,
                taffy::Size {
                    width: AvailableSpace::Definite(300.0),
                    height: AvailableSpace::Definite(80.0),
                },
            )
            .expect("text field layout should compute");

        let layout = taffy.layout(node).expect("text field layout should exist");
        assert_eq!(layout.size.width, 260.0);
        assert_eq!(layout.size.height, 44.0);
    }

    #[test]
    fn advanced_ui_text_field_theme_density_changes_layout_tokens() {
        let theme = Theme::light().with_density(ThemeDensity { scale: 1.5 });
        let mut field = TextField::new("Search").theme(theme).w(260.0);
        let mut taffy = TaffyTree::<ElementId>::new();
        let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(300.0, 80.0));
        let node = field.layout(&mut layout_cx);

        taffy
            .compute_layout(
                node,
                taffy::Size {
                    width: AvailableSpace::Definite(300.0),
                    height: AvailableSpace::Definite(80.0),
                },
            )
            .expect("text field layout should compute");

        let layout = taffy.layout(node).expect("text field layout should exist");
        assert_eq!(layout.size.width, 260.0);
        assert_eq!(layout.size.height, 54.0);
    }

    #[test]
    fn advanced_ui_text_field_accessibility_reflects_shared_contract() {
        let id = ElementId::new();
        let mut field = TextField::new("Search").id(id).value("alpha");
        field
            .apply_key_event(&KeyEvent::new(KeyCode::ArrowLeft, Modifiers::shift()))
            .expect("shift-left should create a selection");
        field = field.invalid(true).read_only(true);

        let nodes = field
            .accessibility_nodes(&AccessibilityContext::new(Some(id)))
            .expect("text field accessibility should build");
        let node = nodes.first().expect("text field should expose one node");

        assert_eq!(node.a11y_id(), id);
        assert_eq!(node.a11y_role(), AccessibilityRole::TextInput);
        assert_eq!(node.a11y_label(), Some("Search"));
        assert_eq!(node.a11y_value(), Some("alpha"));
        assert_eq!(node.a11y_text_caret(), Some(4));
        assert_eq!(
            node.a11y_text_selection(),
            Some(AccessibilityTextRange::new(4, 5))
        );
        assert!(node.a11y_focused());
        assert!(node.a11y_read_only());
        assert!(node.a11y_invalid());
        assert!(node.a11y_actions().is_empty());
    }

    #[test]
    fn advanced_ui_text_field_paints_invalid_border() {
        let mut field = TextField::new("Email").invalid(true);
        let mut taffy = TaffyTree::<ElementId>::new();
        let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(200.0, 40.0));
        let node = field.layout(&mut layout_cx);
        taffy
            .compute_layout(
                node,
                taffy::Size {
                    width: AvailableSpace::Definite(200.0),
                    height: AvailableSpace::Definite(40.0),
                },
            )
            .expect("text field layout should compute");

        let mut scene = Scene::new();
        let mut paint_cx =
            PaintContext::new(&mut scene, Bounds::from_xywh(0.0, 0.0, 200.0, 36.0), &taffy);
        field.paint(&mut paint_cx);

        assert!(scene.primitives().iter().any(|primitive| {
            matches!(
                primitive,
                Primitive::Quad { border_color, .. }
                    if *border_color == Color::hex(0xdc2626).to_rgba()
            )
        }));
    }

    #[test]
    #[should_panic(expected = "advanced text field label must not be empty")]
    fn advanced_ui_text_field_rejects_empty_label() {
        drop(TextField::new(" "));
    }
}
