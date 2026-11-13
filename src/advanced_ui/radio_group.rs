use crate::advanced_ui::state::{IndexedInteractionState, InteractionState, require_non_empty};
use crate::advanced_ui::tokens::{ControlSize, ControlState, ControlVariant, Theme};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityRole,
};
use crate::core::action::{ActionId, ActionOutcome, StandardAction};
use crate::core::event::{KeyCode, KeyEvent, Modifiers};
use crate::core::geometry::{Bounds, Edges, Point};
use crate::core::style::{Corners, Style};
use crate::elements::element::StableId;
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    style_to_taffy,
};
use crate::renderer::Primitive;
use crate::renderer::text::BaseDirection;
use std::any::TypeId;
use taffy::prelude::*;

type RadioChangeHandler = Box<dyn Fn(&str)>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioOption {
    value: String,
    label: String,
}

impl RadioOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        let value = value.into();
        let label = label.into();
        require_non_empty(&value, "radio option value must not be empty");
        require_non_empty(&label, "radio option label must not be empty");
        Self { value, label }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

impl From<(&str, &str)> for RadioOption {
    fn from((value, label): (&str, &str)) -> Self {
        Self::new(value, label)
    }
}

/// A column of mutually exclusive options. The group takes focus as a whole
/// and the arrow keys move the selection, wrapping at the ends.
pub struct RadioGroup {
    id: StableId,
    label: String,
    options: Vec<RadioOption>,
    option_ids: Vec<ElementId>,
    selected: String,
    size: ControlSize,
    theme: Theme,
    state: InteractionState,
    indexed_state: IndexedInteractionState,
    style: Style,
    on_change: Option<RadioChangeHandler>,
}

impl RadioGroup {
    pub fn new<I, O>(label: impl Into<String>, options: I, selected: impl Into<String>) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<RadioOption>,
    {
        let label = label.into();
        require_non_empty(&label, "radio group accessibility label must not be empty");
        let options: Vec<RadioOption> = options.into_iter().map(Into::into).collect();
        assert!(
            !options.is_empty(),
            "radio group requires at least one option"
        );
        let selected = selected.into();
        validate_selected(&options, &selected);
        let option_ids = options.iter().map(|_| ElementId::new()).collect();

        Self {
            id: StableId::new(),
            label,
            options,
            option_ids,
            selected,
            size: ControlSize::default(),
            theme: Theme::default(),
            state: InteractionState::default(),
            indexed_state: IndexedInteractionState::default(),
            style: Style::new(),
            on_change: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = StableId::explicit(id);
        self
    }

    pub fn selected(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        validate_selected(&self.options, &value);
        self.selected = value;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.set_disabled(disabled);
        if disabled {
            self.indexed_state.clear();
        }
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.set_read_only(read_only);
        if read_only {
            self.indexed_state.clear();
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

    pub fn on_change(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    pub fn selected_value(&self) -> &str {
        &self.selected
    }

    pub fn hovered_index(&self) -> Option<usize> {
        self.indexed_state.hovered_index()
    }

    pub fn interaction_state(&self) -> InteractionState {
        self.state
    }

    fn row_height(&self) -> f32 {
        self.theme.control_height(self.size)
    }

    fn selected_index(&self) -> usize {
        self.options
            .iter()
            .position(|option| option.value == self.selected)
            .unwrap_or(0)
    }

    fn index_at(&self, bounds: Bounds, position: Point) -> Option<usize> {
        if !bounds.contains(position) {
            return None;
        }
        let index = ((position.y - bounds.y()) / self.row_height()).floor() as usize;
        (index < self.options.len()).then_some(index)
    }

    fn select_index(&mut self, index: usize, cx: &EventContext) -> ActionOutcome {
        if !self.state.can_activate() {
            return ActionOutcome::Ignored;
        }
        let option = &self.options[index];
        if option.value != self.selected {
            self.selected = option.value.clone();
            if let Some(handler) = &self.on_change {
                handler(&self.selected);
            }
            cx.announce_accessibility_action(self.id.get(), format!("{} selected", option.label));
            cx.request_redraw();
        }
        ActionOutcome::handled("advanced_ui.radio_group")
    }

    fn step(&mut self, forward: bool, cx: &EventContext) -> ActionOutcome {
        let len = self.options.len();
        let current = self.selected_index();
        let next = if forward {
            (current + 1) % len
        } else {
            (current + len - 1) % len
        };
        self.select_index(next, cx)
    }
}

impl Element for RadioGroup {
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
        let parent = self.id.get();
        for (index, id) in self.option_ids.iter_mut().enumerate() {
            *id = ElementId::derived(parent, index as u64, TypeId::of::<RadioOption>());
        }
        let label_width = self
            .options
            .iter()
            .map(|option| {
                self.theme.measure_label(
                    cx,
                    &option.label,
                    self.size,
                    self.theme.typography.label_weight,
                )
            })
            .fold(0.0, f32::max);
        let mut style = style_to_taffy(&self.style);
        style.size.width = Dimension::Length(
            self.theme.indicator_extent(self.size) + self.theme.control_gap() + label_width,
        );
        style.size.height = Dimension::Length(self.row_height() * self.options.len() as f32);
        cx.new_leaf(style).unwrap_or_else(|err| {
            panic!("failed to create advanced radio group layout node: {err}")
        })
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        cx.register_hit_region(self.id.get(), bounds);
        let focused = cx.is_focused(self.id.get());
        let indicator = self.theme.indicator_extent(self.size);
        let gap = self.theme.control_gap();

        for (index, option) in self.options.iter().enumerate() {
            let row = Bounds::from_xywh(
                bounds.x(),
                bounds.y() + index as f32 * self.row_height(),
                bounds.width(),
                self.row_height(),
            );
            cx.register_accessibility_region(self.option_ids[index], row);
            let selected = option.value == self.selected;
            let mut state: ControlState = self.state.into();
            state.selected = selected;
            state.hovered = self.indexed_state.hovered_index() == Some(index);
            state.pressed = self.indexed_state.pressed_index() == Some(index);
            state.focused = focused && selected && !state.disabled;
            let colors = self.theme.control_colors(ControlVariant::Primary, state);

            let circle = Bounds::from_xywh(
                row.x(),
                row.y() + (row.height() - indicator) / 2.0,
                indicator,
                indicator,
            );
            let border = if selected {
                colors.background
            } else {
                self.theme.colors.border
            };
            cx.paint(Primitive::Quad {
                bounds: circle,
                background: self.theme.surface_color_for_state(state).to_rgba(),
                border_color: self.theme.state_border_color(state, border).to_rgba(),
                border_widths: Edges::all(if state.focused { 2.0 } else { 1.0 }),
                corner_radii: Corners::all(indicator / 2.0),
            });
            if selected {
                let dot = indicator / 2.0;
                cx.paint(Primitive::Quad {
                    bounds: Bounds::from_xywh(
                        circle.x() + dot / 2.0,
                        circle.y() + dot / 2.0,
                        dot,
                        dot,
                    ),
                    background: colors.background.to_rgba(),
                    border_color: colors.background.to_rgba(),
                    border_widths: Edges::ZERO,
                    corner_radii: Corners::all(dot / 2.0),
                });
            }

            let mut label_color = self.theme.colors.text;
            if state.disabled {
                label_color = label_color.with_alpha(self.theme.state.disabled_opacity);
            } else if state.read_only {
                label_color = label_color.with_alpha(self.theme.state.read_only_opacity);
            }
            cx.paint(Primitive::Text {
                bounds: Bounds::from_xywh(
                    row.x() + indicator + gap,
                    row.y(),
                    (row.width() - indicator - gap).max(0.0),
                    row.height(),
                ),
                content: option.label.clone().into(),
                color: label_color.to_rgba(),
                font_size: self.theme.text_size(self.size),
                font_weight: self.theme.typography.label_weight,
                font_family: None,
                line_height: self.theme.typography.line_height,
                align: crate::elements::text::TextAlign::Left,
                direction: BaseDirection::Auto,
            });
        }
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let mut node = AccessibilityNode::label_required(
            self.id.get(),
            AccessibilityRole::RadioGroup,
            &self.label,
        )?
        .value_required(&self.selected)?
        .with_enabled(!self.state.disabled())
        .with_read_only(self.state.read_only())
        .with_focused(cx.a11y_has_focus(self.id.get()));

        for (option, id) in self.options.iter().zip(self.option_ids.iter().copied()) {
            let mut child = AccessibilityNode::label_required(
                id,
                AccessibilityRole::RadioButton,
                &option.label,
            )?
            .value_required(&option.value)?
            .with_selected(option.value == self.selected)
            .with_enabled(!self.state.disabled())
            .with_read_only(self.state.read_only())
            .with_focused(cx.a11y_has_focus(id));
            if self.state.can_activate() {
                child = child.with_action(AccessibilityAction::Activate);
            }
            node = node.with_child(child);
        }
        Ok(Some(node))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let index = if cx.contains_pointer(event.position) {
            self.index_at(cx.bounds(), event.position)
        } else {
            None
        };
        match event.kind {
            PointerEventKind::Move => {
                self.indexed_state.update_hover(index, cx, self.state);
                false
            }
            PointerEventKind::Down => {
                let pressed = self.indexed_state.press(index, cx, self.state);
                if pressed {
                    cx.request_focus(Some(self.id.get()));
                }
                pressed
            }
            PointerEventKind::Up => {
                let release = self.indexed_state.release(index, cx, self.state);
                match release.released_index {
                    Some(index) if release.activated => self.select_index(index, cx).is_handled(),
                    _ => false,
                }
            }
        }
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        if event.key == KeyCode::Space
            && event.modifiers == Modifiers::none()
            && cx.is_focused(Some(self.id.get()))
        {
            return self.select_index(self.selected_index(), cx).is_handled();
        }
        false
    }

    fn handle_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        if matches!(action, ActionId::Standard(StandardAction::Activate))
            && let Some(index) = cx
                .focused_id()
                .and_then(|focused| self.option_ids.iter().position(|id| *id == focused))
        {
            return self.select_index(index, cx);
        }
        if !cx.is_focused(Some(self.id.get())) {
            return ActionOutcome::Ignored;
        }
        match action {
            ActionId::Standard(StandardAction::MoveDown | StandardAction::MoveRight) => {
                self.step(true, cx)
            }
            ActionId::Standard(StandardAction::MoveUp | StandardAction::MoveLeft) => {
                self.step(false, cx)
            }
            _ => ActionOutcome::Ignored,
        }
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id.get() == id || self.option_ids.contains(&id)
    }
}

fn validate_selected(options: &[RadioOption], selected: &str) {
    if !options.iter().any(|option| option.value == selected) {
        panic!("radio group selected value must match an option");
    }
}

pub fn radio_group<I, O>(
    label: impl Into<String>,
    options: I,
    selected: impl Into<String>,
) -> RadioGroup
where
    I: IntoIterator<Item = O>,
    O: Into<RadioOption>,
{
    RadioGroup::new(label, options, selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::MouseButton;
    use std::cell::RefCell;
    use std::rc::Rc;
    use taffy::TaffyTree;

    const OPTIONS: [(&str, &str); 3] = [("s", "Small"), ("m", "Medium"), ("l", "Large")];

    fn pointer(kind: PointerEventKind, y: f32) -> PointerEvent {
        PointerEvent {
            kind,
            position: Point::new(8.0, y),
            button: Some(MouseButton::Left),
        }
    }

    #[test]
    fn advanced_ui_radio_group_click_picks_an_option() {
        let changes = Rc::new(RefCell::new(Vec::<String>::new()));
        let sink = Rc::clone(&changes);
        let mut group = RadioGroup::new("Size", OPTIONS, "s")
            .on_change(move |value| sink.borrow_mut().push(value.to_string()));
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 120.0, 96.0),
            &taffy,
            &mut focused,
        );

        // Rows are 36pt tall: y = 80 is the third option.
        assert!(group.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 80.0)));
        assert!(group.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Up, 80.0)));
        assert_eq!(group.selected_value(), "l");
        assert_eq!(&*changes.borrow(), &["l".to_string()]);
    }

    #[test]
    fn advanced_ui_radio_group_arrows_move_and_wrap() {
        let id = ElementId::new();
        let mut group = RadioGroup::new("Size", OPTIONS, "l").id(id);
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = Some(id);
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 120.0, 96.0),
            &taffy,
            &mut focused,
        );

        let down = ActionId::Standard(StandardAction::MoveDown);
        assert!(group.handle_action(&mut cx, &down).is_handled());
        assert_eq!(group.selected_value(), "s");
        let up = ActionId::Standard(StandardAction::MoveUp);
        assert!(group.handle_action(&mut cx, &up).is_handled());
        assert_eq!(group.selected_value(), "l");
        let left = ActionId::Standard(StandardAction::MoveLeft);
        assert!(group.handle_action(&mut cx, &left).is_handled());
        assert_eq!(group.selected_value(), "m");
    }

    #[test]
    fn advanced_ui_radio_group_disabled_refuses_changes() {
        let id = ElementId::new();
        let mut group = RadioGroup::new("Size", OPTIONS, "s").id(id).disabled(true);
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = Some(id);
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 120.0, 96.0),
            &taffy,
            &mut focused,
        );

        assert!(!group.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 80.0)));
        let down = ActionId::Standard(StandardAction::MoveDown);
        assert!(!group.handle_action(&mut cx, &down).is_handled());
        assert_eq!(group.selected_value(), "s");
    }

    #[test]
    fn advanced_ui_radio_group_accessibility_lists_radio_buttons() {
        let node = RadioGroup::new("Size", OPTIONS, "m")
            .accessibility(&AccessibilityContext::default())
            .expect("radio group node")
            .expect("radio group exposes a node");

        assert_eq!(node.a11y_role(), AccessibilityRole::RadioGroup);
        assert_eq!(node.a11y_value(), Some("m"));
        let selected: Vec<_> = node
            .a11y_children()
            .iter()
            .map(|child| (child.a11y_role(), child.a11y_selected()))
            .collect();
        assert_eq!(
            selected,
            [
                (AccessibilityRole::RadioButton, Some(false)),
                (AccessibilityRole::RadioButton, Some(true)),
                (AccessibilityRole::RadioButton, Some(false)),
            ]
        );
    }

    #[test]
    #[should_panic(expected = "radio group selected value must match an option")]
    fn advanced_ui_radio_group_rejects_missing_selection() {
        drop(RadioGroup::new("Size", OPTIONS, "xl"));
    }
}
