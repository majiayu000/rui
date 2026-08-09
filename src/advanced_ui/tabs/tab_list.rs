use super::model::{Tab, validate_selected_tab, validate_tabs};
use crate::advanced_ui::state::{IndexedInteractionState, InteractionState, require_non_empty};
use crate::advanced_ui::tokens::{ControlSize, Theme};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityRole,
};
use crate::core::action::{ActionId, ActionOutcome, StandardAction};
use crate::core::color::Color;
use crate::core::event::{KeyCode, KeyEvent};
use crate::core::geometry::{Bounds, Edges, Point};
use crate::core::style::{Corners, Style};
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    style_to_taffy,
};
use crate::renderer::Primitive;
use taffy::prelude::*;

type TabChangeHandler = Box<dyn Fn(&str)>;

pub struct TabList {
    id: ElementId,
    tabs: Vec<Tab>,
    selected: String,
    accessibility_label: Option<String>,
    size: ControlSize,
    theme: Theme,
    state: InteractionState,
    indexed_state: IndexedInteractionState,
    style: Style,
    on_change: Option<TabChangeHandler>,
}

impl TabList {
    pub fn new<I, T>(tabs: I, selected: impl Into<String>) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Tab>,
    {
        let tabs: Vec<Tab> = tabs.into_iter().map(Into::into).collect();
        let selected = selected.into();
        validate_tabs(&tabs, &selected);

        let theme = Theme::default();
        let mut style = Style::new();
        style.border.radius = Corners::top(theme.control_radius());

        Self {
            id: ElementId::new(),
            tabs,
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
        validate_selected_tab(&self.tabs, &value);
        self.selected = value;
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self.style.border.radius = Corners::top(self.theme.control_radius());
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
        require_non_empty(&label, "tab list accessibility label must not be empty");
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

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
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

    pub(crate) fn selected_index(&self) -> usize {
        self.tabs
            .iter()
            .position(|tab| tab.value == self.selected)
            .expect("tab list selected value must match a tab")
    }

    pub(crate) fn selected_tab_label(&self) -> &str {
        self.tabs[self.selected_index()].label()
    }

    fn tab_width(&self) -> f32 {
        self.tabs
            .iter()
            .map(|tab| {
                tab.label.chars().count() as f32 * self.theme.text_size(self.size) * 0.58
                    + self.theme.horizontal_padding(self.size) * 2.0
            })
            .fold(72.0, f32::max)
    }

    fn tab_bounds(&self, bounds: Bounds, index: usize) -> Bounds {
        let width = bounds.width() / self.tabs.len() as f32;
        Bounds::from_xywh(
            bounds.x() + index as f32 * width,
            bounds.y(),
            width,
            bounds.height(),
        )
    }

    fn enabled_index_at(&self, bounds: Bounds, position: Point) -> Option<usize> {
        if !bounds.contains(position) {
            return None;
        }
        let width = bounds.width() / self.tabs.len() as f32;
        let index = ((position.x - bounds.x()) / width).floor() as usize;
        (index < self.tabs.len() && !self.tabs[index].disabled).then_some(index)
    }

    fn select_index(&mut self, index: usize, cx: &mut EventContext) -> bool {
        if !self.state.can_activate() || self.tabs[index].disabled {
            return false;
        }

        let focus_was_on_tab = cx
            .focused_id()
            .is_some_and(|focused| self.tabs.iter().any(|tab| tab.id == focused));
        if self.tabs[index].value != self.selected {
            self.selected = self.tabs[index].value.clone();
            if let Some(handler) = &self.on_change {
                handler(&self.selected);
            }
            cx.announce_accessibility_action(
                self.tabs[index].id,
                format!("{} tab selected", self.tabs[index].label),
            );
            cx.request_redraw();
        }
        if focus_was_on_tab {
            cx.request_focus(Some(self.tabs[index].id));
        }
        true
    }

    fn next_enabled_index(&self, from: usize, direction: isize) -> Option<usize> {
        let len = self.tabs.len();
        for offset in 1..=len {
            let candidate = if direction >= 0 {
                (from + offset) % len
            } else {
                (from + len - (offset % len)) % len
            };
            if !self.tabs[candidate].disabled {
                return Some(candidate);
            }
        }
        None
    }

    fn first_enabled_index(&self) -> Option<usize> {
        self.tabs.iter().position(|tab| !tab.disabled)
    }

    fn last_enabled_index(&self) -> Option<usize> {
        self.tabs.iter().rposition(|tab| !tab.disabled)
    }

    fn handles_keyboard(&self, cx: &EventContext) -> bool {
        cx.focused_id()
            .map(|id| id == self.id || self.tabs.iter().any(|tab| tab.id == id))
            .unwrap_or(false)
    }
}

impl Element for TabList {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let mut style = style_to_taffy(&self.style);
        style.size.width = Dimension::Length(self.tab_width() * self.tabs.len() as f32);
        style.size.height = Dimension::Length(self.theme.control_height(self.size));

        match cx.taffy.new_leaf(style) {
            Ok(node) => node,
            Err(err) => panic!("failed to create advanced tab list layout node: {}", err),
        }
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        cx.register_hit_region(self.id, bounds);
        paint_tab_list_rule(cx, bounds, self.state.invalid(), self.theme);

        for (index, tab) in self.tabs.iter().enumerate() {
            let tab_bounds = self.tab_bounds(bounds, index);
            cx.register_hit_region(tab.id, tab_bounds);
            self.paint_tab(cx, tab, index, tab_bounds);
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
                    role: AccessibilityRole::TabList,
                })?;
        let mut node =
            AccessibilityNode::label_required(self.id, AccessibilityRole::TabList, label)?
                .value_required(&self.selected)?
                .with_enabled(!self.state.disabled())
                .with_read_only(self.state.read_only())
                .with_invalid(self.state.invalid())
                .with_focused(cx.a11y_has_focus(self.id));

        for tab in &self.tabs {
            let tab_enabled = !self.state.disabled() && !tab.disabled;
            let mut child =
                AccessibilityNode::label_required(tab.id, AccessibilityRole::Tab, &tab.label)?
                    .value_required(&tab.value)?
                    .with_selected(tab.value == self.selected)
                    .with_enabled(tab_enabled)
                    .with_read_only(self.state.read_only())
                    .with_invalid(self.state.invalid())
                    .with_focused(cx.a11y_has_focus(tab.id));
            if self.state.can_activate() && !tab.disabled {
                child = child.with_action(AccessibilityAction::Activate);
            }
            node = node.with_child(child);
        }

        Ok(Some(node))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let index = self.enabled_index_at(cx.bounds(), event.position);
        match event.kind {
            PointerEventKind::Move => {
                self.indexed_state.update_hover(index, cx, self.state);
                false
            }
            PointerEventKind::Down => {
                if index.is_some() && self.state.can_activate() {
                    cx.request_focus(Some(self.id));
                }
                self.indexed_state.press(index, cx, self.state)
            }
            PointerEventKind::Up => {
                let release = self.indexed_state.release(index, cx, self.state);
                if release.activated {
                    return release
                        .released_index
                        .map(|released| self.select_index(released, cx))
                        .unwrap_or(false);
                }
                false
            }
        }
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        if !self.state.can_activate() || !self.handles_keyboard(cx) {
            return false;
        }
        if !event.modifiers.is_empty() {
            return false;
        }

        let selected = self.selected_index();
        let target = match event.key {
            KeyCode::ArrowRight | KeyCode::ArrowDown => self.next_enabled_index(selected, 1),
            KeyCode::ArrowLeft | KeyCode::ArrowUp => self.next_enabled_index(selected, -1),
            KeyCode::Home => self.first_enabled_index(),
            KeyCode::End => self.last_enabled_index(),
            KeyCode::Enter | KeyCode::Space => Some(selected),
            _ => None,
        };

        match target {
            Some(index) => self.select_index(index, cx),
            None => false,
        }
    }

    fn handle_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        if !matches!(action, ActionId::Standard(StandardAction::Activate)) {
            return ActionOutcome::Ignored;
        }
        let Some(index) = cx
            .focused_id()
            .and_then(|focused| self.tabs.iter().position(|tab| tab.id == focused))
        else {
            return ActionOutcome::Ignored;
        };
        if self.select_index(index, cx) {
            ActionOutcome::handled("advanced_ui.tab_list")
        } else {
            ActionOutcome::Ignored
        }
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id == id || self.tabs.iter().any(|tab| tab.id == id)
    }
}

impl TabList {
    fn paint_tab(&self, cx: &mut PaintContext, tab: &Tab, index: usize, bounds: Bounds) {
        let selected = tab.value == self.selected;
        let hovered = self.indexed_state.hovered_index() == Some(index);
        let pressed = self.indexed_state.pressed_index() == Some(index);
        let mut tab_state = self.state;
        tab_state.set_selected(selected);
        tab_state.set_hovered(hovered && !tab.disabled);
        tab_state.set_pressed(pressed && !tab.disabled);
        if tab.disabled {
            tab_state.set_disabled(true);
        }

        if selected || hovered {
            let tab_background = if selected {
                self.theme.colors.surface
            } else {
                self.theme.surface_color_for_state(tab_state.into())
            };
            let tab_border = if selected {
                self.theme
                    .state_border_color(self.state.into(), self.theme.colors.border)
            } else {
                Color::TRANSPARENT
            };
            cx.paint(Primitive::Quad {
                bounds,
                background: tab_background.to_rgba(),
                border_color: tab_border.to_rgba(),
                border_widths: if selected {
                    Edges::all(1.0)
                } else {
                    Edges::ZERO
                },
                corner_radii: Corners::top(self.theme.control_radius()),
            });
        }

        let text = if selected {
            self.theme.colors.primary.rest.background
        } else {
            self.theme.colors.text
        };
        cx.paint(Primitive::Text {
            bounds,
            content: tab.label.clone(),
            color: if tab.disabled {
                text.with_alpha(0.5)
            } else {
                text
            }
            .to_rgba(),
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

fn paint_tab_list_rule(cx: &mut PaintContext, bounds: Bounds, invalid: bool, theme: Theme) {
    cx.paint(Primitive::Quad {
        bounds,
        background: Color::TRANSPARENT.to_rgba(),
        border_color: theme
            .validation_border_color(invalid, theme.colors.border)
            .to_rgba(),
        border_widths: Edges {
            top: 0.0,
            right: 0.0,
            bottom: 1.0,
            left: 0.0,
        },
        corner_radii: Corners::ZERO,
    });
}

trait ModifiersExt {
    fn is_empty(self) -> bool;
}

impl ModifiersExt for crate::core::event::Modifiers {
    fn is_empty(self) -> bool {
        !self.shift && !self.ctrl && !self.alt && !self.meta
    }
}
