use crate::advanced_ui::state::{IndexedInteractionState, InteractionState, require_non_empty};
use crate::advanced_ui::tokens::{ControlSize, ControlVariant, Theme};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityRole,
};
use crate::core::action::{ActionId, ActionOutcome, StandardAction};
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Corners, Style};
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    style_to_taffy,
};
use crate::renderer::Primitive;
use taffy::prelude::*;

type SegmentChangeHandler = Box<dyn Fn(&str)>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentedOption {
    value: String,
    label: String,
}

impl SegmentedOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        let value = value.into();
        let label = label.into();
        require_non_empty(&value, "segmented option value must not be empty");
        require_non_empty(&label, "segmented option label must not be empty");

        Self { value, label }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

impl From<(&str, &str)> for SegmentedOption {
    fn from((value, label): (&str, &str)) -> Self {
        Self::new(value, label)
    }
}

pub struct SegmentedControl {
    id: ElementId,
    options: Vec<SegmentedOption>,
    option_ids: Vec<ElementId>,
    selected: String,
    accessibility_label: Option<String>,
    size: ControlSize,
    theme: Theme,
    state: InteractionState,
    indexed_state: IndexedInteractionState,
    style: Style,
    on_change: Option<SegmentChangeHandler>,
}

impl SegmentedControl {
    pub fn new<I, O>(options: I, selected: impl Into<String>) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<SegmentedOption>,
    {
        let options: Vec<SegmentedOption> = options.into_iter().map(Into::into).collect();
        let selected = selected.into();
        validate_options(&options, &selected);
        let option_ids = options.iter().map(|_| ElementId::new()).collect();

        let theme = Theme::default();
        let mut style = Style::new();
        style.border.radius = Corners::all(theme.control_radius());

        Self {
            id: ElementId::new(),
            options,
            option_ids,
            selected,
            accessibility_label: None,
            size: ControlSize::default(),
            theme,
            state: InteractionState::default(),
            indexed_state: IndexedInteractionState::default(),
            style,
            on_change: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self
    }

    pub fn selected(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        validate_selected(&self.options, &value);
        self.selected = value;
        self
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

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.state.set_invalid(invalid);
        self
    }

    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        let label = label.into();
        require_non_empty(
            &label,
            "segmented control accessibility label must not be empty",
        );
        self.accessibility_label = Some(label);
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

    pub fn pressed_index(&self) -> Option<usize> {
        self.indexed_state.pressed_index()
    }

    pub fn interaction_state(&self) -> InteractionState {
        self.state
    }

    fn option_width(&self) -> f32 {
        self.options
            .iter()
            .map(|option| {
                option.label.chars().count() as f32 * self.theme.text_size(self.size) * 0.56
                    + self.theme.horizontal_padding(self.size) * 2.0
            })
            .fold(72.0, f32::max)
    }

    fn index_at(&self, bounds: Bounds, position: crate::core::geometry::Point) -> Option<usize> {
        if !bounds.contains(position) {
            return None;
        }
        let width = bounds.width() / self.options.len() as f32;
        let index = ((position.x - bounds.x()) / width).floor() as usize;
        (index < self.options.len()).then_some(index)
    }

    fn selected_index(&self) -> Option<usize> {
        self.options
            .iter()
            .position(|option| option.value == self.selected)
    }

    fn select_index(&mut self, index: usize, cx: &EventContext) -> ActionOutcome {
        let Some(option) = self.options.get(index) else {
            return ActionOutcome::Ignored;
        };
        let value = option.value.clone();
        let label = option.label.clone();
        if value == self.selected {
            return ActionOutcome::handled("advanced_ui.segmented_control");
        }

        self.selected = value;
        if let Some(handler) = &self.on_change {
            handler(&self.selected);
        }
        cx.announce_accessibility_action(self.id, format!("{label} selected"));
        cx.request_redraw();
        ActionOutcome::handled("advanced_ui.segmented_control")
    }
}

impl Element for SegmentedControl {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let mut style = style_to_taffy(&self.style);
        style.size.width = Dimension::Length(self.option_width() * self.options.len() as f32);
        style.size.height = Dimension::Length(self.theme.control_height(self.size));

        match cx.taffy.new_leaf(style) {
            Ok(node) => node,
            Err(err) => panic!(
                "failed to create advanced segmented control layout node: {}",
                err
            ),
        }
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        cx.register_hit_region(self.id, bounds);
        let container_state = self.state.into();

        cx.paint(Primitive::Quad {
            bounds,
            background: self
                .theme
                .surface_color_for_state(container_state)
                .to_rgba(),
            border_color: self
                .theme
                .state_border_color(container_state, self.theme.colors.border)
                .to_rgba(),
            border_widths: Edges::all(1.0),
            corner_radii: self.style.border.radius,
        });

        let segment_width = bounds.width() / self.options.len() as f32;
        for (index, option) in self.options.iter().enumerate() {
            let segment_bounds = Bounds::from_xywh(
                bounds.x() + index as f32 * segment_width,
                bounds.y(),
                segment_width,
                bounds.height(),
            );
            let selected = option.value == self.selected;
            let hovered = self.indexed_state.hovered_index() == Some(index);
            let mut segment_state = self.state;
            segment_state.set_selected(selected);
            segment_state.set_hovered(hovered);
            segment_state.set_pressed(self.indexed_state.pressed_index() == Some(index));
            let segment_token_state = segment_state.into();
            let colors = self
                .theme
                .control_colors(ControlVariant::Primary, segment_token_state);

            if selected || hovered {
                cx.paint(Primitive::Quad {
                    bounds: segment_bounds,
                    background: if selected {
                        colors.background
                    } else {
                        self.theme.surface_color_for_state(segment_token_state)
                    }
                    .to_rgba(),
                    border_color: Color::TRANSPARENT.to_rgba(),
                    border_widths: Edges::ZERO,
                    corner_radii: segment_corners(
                        index,
                        self.options.len(),
                        self.theme.control_radius(),
                    ),
                });
            }

            let mut label_color = if selected {
                colors.foreground
            } else {
                self.theme.colors.text
            };
            if segment_token_state.disabled {
                label_color = label_color.with_alpha(self.theme.state.disabled_opacity);
            } else if segment_token_state.read_only {
                label_color = label_color.with_alpha(self.theme.state.read_only_opacity);
            }
            cx.paint(Primitive::Text {
                bounds: segment_bounds,
                content: option.label.clone(),
                color: label_color.to_rgba(),
                font_size: self.theme.text_size(self.size),
                font_weight: if selected {
                    self.theme.typography.selected_weight
                } else {
                    self.theme.typography.label_weight
                },
                font_family: None,
                line_height: self.theme.typography.line_height,
                align: crate::elements::text::TextAlign::Center,
            });
        }
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let label =
            self.accessibility_label
                .as_deref()
                .ok_or(AccessibilityError::MissingLabel {
                    role: AccessibilityRole::SegmentedControl,
                })?;
        let mut node =
            AccessibilityNode::label_required(self.id, AccessibilityRole::SegmentedControl, label)?
                .value_required(&self.selected)?
                .with_enabled(!self.state.disabled())
                .with_read_only(self.state.read_only())
                .with_invalid(self.state.invalid())
                .with_focused(cx.a11y_has_focus(self.id));

        for (option, id) in self.options.iter().zip(self.option_ids.iter().copied()) {
            let mut child = AccessibilityNode::label_required(
                id,
                AccessibilityRole::SegmentedOption,
                &option.label,
            )?
            .value_required(&option.value)?
            .with_selected(option.value == self.selected)
            .with_enabled(!self.state.disabled())
            .with_read_only(self.state.read_only())
            .with_invalid(self.state.invalid());
            if self.state.can_activate() {
                child = child.with_action(AccessibilityAction::Activate);
            }
            node = node.with_child(child);
        }

        Ok(Some(node))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let index = self.index_at(cx.bounds(), event.position);
        match event.kind {
            PointerEventKind::Move => {
                self.indexed_state.update_hover(index, cx, self.state);
                false
            }
            PointerEventKind::Down => self.indexed_state.press(index, cx, self.state),
            PointerEventKind::Up => {
                let release = self.indexed_state.release(index, cx, self.state);
                if release.activated {
                    let released = match release.released_index {
                        Some(released) => released,
                        None => return false,
                    };
                    let value = self.options[released].value.clone();
                    if value != self.selected {
                        self.selected = value;
                        if let Some(handler) = &self.on_change {
                            handler(&self.selected);
                        }
                        cx.announce_accessibility_action(
                            self.id,
                            format!("{} selected", self.options[released].label),
                        );
                    }
                    cx.request_redraw();
                    return true;
                }
                false
            }
        }
    }

    fn handle_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        if !cx.is_focused(Some(self.id)) || !self.state.can_activate() {
            return ActionOutcome::Ignored;
        }

        let Some(selected) = self.selected_index() else {
            return ActionOutcome::Ignored;
        };

        match action {
            ActionId::Standard(StandardAction::MoveLeft | StandardAction::MoveUp) => {
                let index = selected.saturating_sub(1);
                self.select_index(index, cx)
            }
            ActionId::Standard(StandardAction::MoveRight | StandardAction::MoveDown) => {
                let index = (selected + 1).min(self.options.len() - 1);
                self.select_index(index, cx)
            }
            ActionId::Standard(StandardAction::Activate) => self.select_index(selected, cx),
            _ => ActionOutcome::Ignored,
        }
    }
}

fn validate_options(options: &[SegmentedOption], selected: &str) {
    if options.is_empty() {
        panic!("segmented control requires at least one option");
    }
    validate_selected(options, selected);
}

fn validate_selected(options: &[SegmentedOption], selected: &str) {
    if !options.iter().any(|option| option.value == selected) {
        panic!("segmented control selected value must match an option");
    }
}

fn segment_corners(index: usize, len: usize, radius: f32) -> Corners {
    match (index == 0, index + 1 == len) {
        (true, true) => Corners::all(radius),
        (true, false) => Corners::left(radius),
        (false, true) => Corners::right(radius),
        (false, false) => Corners::ZERO,
    }
}

pub fn segmented_control<I, O>(options: I, selected: impl Into<String>) -> SegmentedControl
where
    I: IntoIterator<Item = O>,
    O: Into<SegmentedOption>,
{
    SegmentedControl::new(options, selected)
}

use crate::core::color::Color;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::MouseButton;
    use crate::core::geometry::Point;
    use std::cell::RefCell;
    use std::rc::Rc;
    use taffy::TaffyTree;

    fn pointer(kind: PointerEventKind, x: f32) -> PointerEvent {
        PointerEvent {
            kind,
            position: Point::new(x, 4.0),
            button: Some(MouseButton::Left),
        }
    }

    fn pointer_at(kind: PointerEventKind, x: f32, y: f32) -> PointerEvent {
        PointerEvent {
            kind,
            position: Point::new(x, y),
            button: Some(MouseButton::Left),
        }
    }

    #[test]
    fn advanced_ui_segmented_control_changes_selected_value() {
        let changes = Rc::new(RefCell::new(Vec::<String>::new()));
        let changes_ref = Rc::clone(&changes);
        let mut control = SegmentedControl::new([("list", "List"), ("grid", "Grid")], "list")
            .on_change(move |value| changes_ref.borrow_mut().push(value.to_string()));
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 160.0, 36.0),
            &taffy,
            &mut focused,
        );

        assert!(control.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 120.0)));
        assert!(control.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Up, 120.0)));
        assert_eq!(control.selected_value(), "grid");
        assert_eq!(&*changes.borrow(), &["grid".to_string()]);
    }

    #[test]
    fn advanced_ui_segmented_control_ignores_vertical_miss() {
        let mut control = SegmentedControl::new([("list", "List"), ("grid", "Grid")], "list");
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 160.0, 36.0),
            &taffy,
            &mut focused,
        );

        assert!(
            !control
                .handle_pointer_event(&mut cx, &pointer_at(PointerEventKind::Down, 120.0, 80.0),)
        );
        assert_eq!(control.selected_value(), "list");
    }

    #[test]
    fn advanced_ui_segmented_control_read_only_does_not_change_selected_value() {
        let changes = Rc::new(RefCell::new(Vec::<String>::new()));
        let changes_ref = Rc::clone(&changes);
        let mut control = SegmentedControl::new([("list", "List"), ("grid", "Grid")], "list")
            .read_only(true)
            .on_change(move |value| changes_ref.borrow_mut().push(value.to_string()));
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 160.0, 36.0),
            &taffy,
            &mut focused,
        );

        assert!(!control.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 120.0)));
        assert!(!control.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Up, 120.0)));
        assert_eq!(control.selected_value(), "list");
        assert!(changes.borrow().is_empty());
    }

    #[test]
    fn advanced_ui_segmented_control_disabled_clears_hovered_and_pressed_index() {
        let mut control = SegmentedControl::new([("list", "List"), ("grid", "Grid")], "list");
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 160.0, 36.0),
            &taffy,
            &mut focused,
        );

        control.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Move, 120.0));
        control.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 120.0));
        assert_eq!(control.hovered_index(), Some(1));
        assert_eq!(control.pressed_index(), Some(1));

        control = control.disabled(true);

        assert_eq!(control.hovered_index(), None);
        assert_eq!(control.pressed_index(), None);
    }

    #[test]
    #[should_panic(expected = "segmented control selected value must match an option")]
    fn advanced_ui_segmented_control_rejects_missing_selection() {
        drop(SegmentedControl::new([("list", "List")], "grid"));
    }

    #[test]
    #[should_panic(expected = "segmented option label must not be empty")]
    fn advanced_ui_segmented_control_rejects_empty_option_label() {
        drop(SegmentedControl::new([("list", "")], "list"));
    }
}
