use crate::advanced_ui::menu::{Menu, MenuDirection, MenuItem};
use crate::advanced_ui::state::{InteractionState, require_non_empty};
use crate::advanced_ui::tokens::{ControlSize, ControlState, Theme};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityRole,
};
use crate::core::action::{
    ActionId, ActionOutcome, SYNC_ACCESSIBILITY_FOCUS_ACTION, StandardAction,
};
use crate::core::event::{KeyCode, KeyEvent, Modifiers};
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Corners, Style};
use crate::elements::element::StableId;
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    style_to_taffy,
};
use crate::elements::overlay::{Align, OverlaySlot, Placement, Side, place_anchored};
use crate::renderer::Overlay;
use crate::renderer::Primitive;
use crate::renderer::text::BaseDirection;
use std::any::TypeId;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use taffy::prelude::*;

const CHEVRON_WIDTH: f32 = 16.0;
const TYPE_AHEAD_TIMEOUT: Duration = Duration::from_millis(500);

type SelectChangeHandler = Box<dyn Fn(&str)>;

/// A pop-up button showing the selected item; activating it opens a
/// [`Menu`] of the items in an overlay below it (above it when there is no
/// room below), over the content after it.
pub struct Select {
    id: StableId,
    label: String,
    items: Vec<MenuItem>,
    selected: String,
    size: ControlSize,
    theme: Theme,
    state: InteractionState,
    style: Style,
    on_change: Option<SelectChangeHandler>,
    /// The open list, and the value it reported picked.
    menu: Option<Menu>,
    picked: Rc<RefCell<Option<String>>>,
    slot: OverlaySlot,
    type_ahead: String,
    typed_at: Option<Instant>,
}

impl Select {
    pub fn new<I, M>(label: impl Into<String>, items: I, selected: impl Into<String>) -> Self
    where
        I: IntoIterator<Item = M>,
        M: Into<MenuItem>,
    {
        let label = label.into();
        require_non_empty(&label, "select accessibility label must not be empty");
        let items: Vec<MenuItem> = items.into_iter().map(Into::into).collect();
        assert!(!items.is_empty(), "select requires at least one item");
        let selected = selected.into();
        validate_selected(&items, &selected);

        Self {
            id: StableId::new(),
            label,
            items,
            selected,
            size: ControlSize::default(),
            theme: Theme::default(),
            state: InteractionState::default(),
            style: Style::new(),
            on_change: None,
            menu: None,
            picked: Rc::default(),
            slot: OverlaySlot::default(),
            type_ahead: String::new(),
            typed_at: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = StableId::explicit(id);
        self
    }

    pub fn selected(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        validate_selected(&self.items, &value);
        self.selected = value;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.set_disabled(disabled);
        if disabled {
            self.menu = None;
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

    pub fn is_open(&self) -> bool {
        self.menu.is_some()
    }

    /// The item the open list highlights.
    pub fn highlighted_index(&self) -> Option<usize> {
        self.menu.as_ref().and_then(Menu::hovered_index)
    }

    pub fn interaction_state(&self) -> InteractionState {
        self.state
    }

    /// Keep the open list of the select this one replaces, so it stays open
    /// across renders that do not change the selection.
    pub(crate) fn reconcile_transient_state(&mut self, previous: &mut Self) {
        if self.state.can_activate() && previous.selected == self.selected {
            self.menu = previous.menu.take();
            self.picked = Rc::clone(&previous.picked);
            self.type_ahead = std::mem::take(&mut previous.type_ahead);
            self.typed_at = previous.typed_at;
        }
    }

    fn selected_index(&self) -> usize {
        self.items
            .iter()
            .position(|item| item.value() == self.selected)
            .unwrap_or(0)
    }

    fn open(&mut self, highlight: Option<usize>, cx: &EventContext) {
        if !self.state.can_activate() {
            return;
        }
        let picked = Rc::clone(&self.picked);
        let menu_id = ElementId::derived(self.id.get(), 0, TypeId::of::<Menu>());
        let mut menu = Menu::new(self.label.clone(), self.items.clone())
            .id(menu_id)
            .selected(self.selected.clone())
            .size(self.size)
            .theme(self.theme)
            .on_select(move |value| *picked.borrow_mut() = Some(value.to_string()));
        menu.highlight(highlight, cx);
        self.menu = Some(menu);
        cx.request_redraw();
    }

    fn close(&mut self, cx: &EventContext) {
        if self.menu.take().is_some() {
            cx.request_redraw();
        }
    }

    /// Choose what the list reported picked, closing it. The list announced
    /// the pick.
    fn commit_picked(&mut self, cx: &mut EventContext) -> bool {
        let Some(value) = self.picked.borrow_mut().take() else {
            return false;
        };
        self.close(cx);
        if value != self.selected {
            self.selected = value;
            if let Some(handler) = &self.on_change {
                handler(&self.selected);
            }
        }
        cx.request_focus(Some(self.id.get()));
        true
    }

    fn move_highlight(&mut self, direction: MenuDirection, cx: &EventContext) {
        let Some(menu) = self.menu.as_mut() else {
            return self.open(Some(self.selected_index()), cx);
        };
        let next = menu.next_enabled_index(menu.hovered_index(), direction);
        menu.highlight(next, cx);
    }

    fn highlight_end(&mut self, last: bool, cx: &EventContext) {
        let direction = if last {
            MenuDirection::Backward
        } else {
            MenuDirection::Forward
        };
        if self.menu.is_none() {
            self.open(None, cx);
        }
        if let Some(menu) = self.menu.as_mut() {
            let index = menu.next_enabled_index(None, direction);
            menu.highlight(index, cx);
        }
    }

    /// Enter and Space: open the list, or choose its highlighted item.
    fn activate(&mut self, cx: &mut EventContext) -> bool {
        let Some(menu) = self.menu.as_mut() else {
            self.open(Some(self.selected_index()), cx);
            return self.menu.is_some();
        };
        match menu.hovered_index() {
            Some(index) => {
                menu.activate_index(index, cx);
                self.commit_picked(cx);
            }
            None => self.close(cx),
        }
        true
    }

    /// Whether a key now continues the type-ahead search of the keys before.
    fn typing(&self) -> bool {
        self.typed_at
            .is_some_and(|at| at.elapsed() <= TYPE_AHEAD_TIMEOUT)
    }

    fn type_ahead(&mut self, ch: char, cx: &EventContext) -> bool {
        if !self.typing() {
            self.type_ahead.clear();
        }
        self.typed_at = Some(Instant::now());
        self.type_ahead.extend(ch.to_lowercase());
        if self.menu.is_none() {
            self.open(Some(self.selected_index()), cx);
        }
        let Some(menu) = self.menu.as_mut() else {
            return false;
        };
        // Typing one letter again cycles through the items it starts.
        let len = self.items.len();
        let start = menu.hovered_index().map_or(0, |index| {
            if self.type_ahead.chars().count() == 1 {
                index + 1
            } else {
                index
            }
        });
        let found = (0..len)
            .map(|offset| (start + offset) % len)
            .find(|&index| {
                let item = &self.items[index];
                !item.is_disabled() && item.label().to_lowercase().starts_with(&self.type_ahead)
            });
        if found.is_some() {
            menu.highlight(found, cx);
        }
        true
    }

    fn trigger_width(&self, cx: &mut LayoutContext) -> f32 {
        let weight = self.theme.typography.label_weight;
        self.items
            .iter()
            .map(|item| {
                self.theme
                    .measure_label(cx, item.label(), self.size, weight)
            })
            .fold(0.0, f32::max)
            + self.theme.horizontal_padding(self.size) * 2.0
            + CHEVRON_WIDTH
    }

    /// Where the open list goes below the `trigger`.
    fn menu_bounds(&self, taffy: &TaffyTree<ElementId>, trigger: Bounds, window: Bounds) -> Bounds {
        let gap = self.theme.control_gap() * 0.5;
        place_anchored(
            trigger,
            self.slot.size(taffy).unwrap_or_default(),
            window,
            Placement::new(Side::Bottom, Align::Start, gap),
        )
    }

    fn has_focus(&self, cx: &EventContext) -> bool {
        cx.focused_id()
            .is_some_and(|focused| self.contains_id(focused))
    }
}

impl Element for Select {
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
        let mut trigger = taffy::Style::default();
        trigger.size.width = Dimension::Length(self.trigger_width(cx));
        trigger.size.height = Dimension::Length(self.theme.control_height(self.size));
        match self.menu.as_mut() {
            Some(menu) => self.slot.layout_with(cx, |cx| menu.layout(cx)),
            None => self.slot.clear(),
        }
        let mut style = style_to_taffy(&self.style);
        style.size = trigger.size;
        cx.new_leaf(style)
            .unwrap_or_else(|err| panic!("failed to create select layout node: {err}"))
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let trigger = cx.bounds();
        cx.register_hit_region(self.id.get(), trigger);

        let mut state: ControlState = self.state.into();
        state.focused = cx.is_focused(self.id.get()) && !state.disabled;
        cx.paint(Primitive::Quad {
            bounds: trigger,
            background: self.theme.surface_color_for_state(state).to_rgba(),
            border_color: self
                .theme
                .state_border_color(state, self.theme.colors.border)
                .to_rgba(),
            border_widths: Edges::all(if state.focused { 2.0 } else { 1.0 }),
            corner_radii: Corners::all(self.theme.control_radius()),
        });
        let mut text_color = self.theme.colors.text;
        if state.disabled {
            text_color = text_color.with_alpha(self.theme.state.disabled_opacity);
        }
        let padding = self.theme.horizontal_padding(self.size);
        let label = self.items[self.selected_index()].label().to_string();
        let text_bounds = Bounds::from_xywh(
            trigger.x() + padding,
            trigger.y(),
            (trigger.width() - padding * 2.0 - CHEVRON_WIDTH).max(0.0),
            trigger.height(),
        );
        let chevron_bounds = Bounds::from_xywh(
            trigger.x() + trigger.width() - padding - CHEVRON_WIDTH,
            trigger.y(),
            CHEVRON_WIDTH,
            trigger.height(),
        );
        for (bounds, content, align) in [
            (text_bounds, label, crate::elements::text::TextAlign::Left),
            (
                chevron_bounds,
                "▾".to_string(),
                crate::elements::text::TextAlign::Right,
            ),
        ] {
            cx.paint(Primitive::Text {
                bounds,
                content: content.into(),
                color: text_color.to_rgba(),
                font_size: self.theme.text_size(self.size),
                font_weight: self.theme.typography.label_weight,
                font_family: None,
                line_height: self.theme.typography.line_height,
                align,
                direction: BaseDirection::Auto,
            });
        }

        let menu_bounds = self.menu_bounds(cx.taffy, trigger, cx.window());
        if let Some(menu) = self.menu.as_mut() {
            cx.paint_overlay(
                Overlay::popup(self.id.get(), menu_bounds),
                menu_bounds,
                |cx| menu.paint(cx),
            );
        }
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let mut node = AccessibilityNode::label_required(
            self.id.get(),
            AccessibilityRole::ComboBox,
            &self.label,
        )?
        .value_required(self.items[self.selected_index()].label())?
        .with_enabled(!self.state.disabled())
        .with_focused(cx.a11y_has_focus(self.id.get()));
        if self.state.can_activate() {
            node = node.with_action(AccessibilityAction::Activate);
        }
        if let Some(menu) = &self.menu
            && let Some(list) = menu.accessibility(cx)?
        {
            node = node.with_child(list);
        }
        Ok(Some(node))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let trigger = cx.layout_bounds();
        let menu_bounds = self.menu_bounds(cx.taffy, trigger, cx.window());
        if let Some(menu) = self.menu.as_mut() {
            let mut menu_cx = cx.for_overlay(menu_bounds);
            let in_menu = menu_cx.contains_pointer(event.position);
            let handled = menu.handle_pointer_event(&mut menu_cx, event);
            if self.commit_picked(cx) {
                return true;
            }
            if in_menu && !matches!(event.kind, PointerEventKind::Move) {
                // The menu took focus for its press; keep it on the select.
                cx.request_focus(Some(self.id.get()));
                return handled || in_menu;
            }
        }

        let inside = cx.contains_pointer(event.position);
        match event.kind {
            PointerEventKind::Move => {
                self.state.update_hover_inside(inside, cx);
                false
            }
            PointerEventKind::Down => {
                if self.state.press_inside(inside, cx) {
                    cx.request_focus(Some(self.id.get()));
                    return true;
                }
                // A press anywhere else dismisses the list.
                self.close(cx);
                false
            }
            PointerEventKind::Up => {
                if !self.state.release_inside(inside, cx).activated {
                    return false;
                }
                if self.menu.is_some() {
                    self.close(cx);
                } else {
                    self.open(Some(self.selected_index()), cx);
                }
                true
            }
        }
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        if !self.state.can_activate() || !self.has_focus(cx) {
            return false;
        }
        match event.key {
            KeyCode::Space if event.modifiers == Modifiers::none() && !self.typing() => {
                self.activate(cx)
            }
            KeyCode::Home => {
                self.highlight_end(false, cx);
                true
            }
            KeyCode::End => {
                self.highlight_end(true, cx);
                true
            }
            _ => match event.char {
                Some(ch) if !ch.is_control() && !event.modifiers.meta && !event.modifiers.ctrl => {
                    self.type_ahead(ch, cx)
                }
                _ => false,
            },
        }
    }

    fn handle_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        if let ActionId::Custom(name) = action
            && name == SYNC_ACCESSIBILITY_FOCUS_ACTION
        {
            if !self.has_focus(cx) {
                self.close(cx);
            }
            return ActionOutcome::Ignored;
        }
        if !self.state.can_activate() || !self.has_focus(cx) {
            return ActionOutcome::Ignored;
        }
        // Assistive technology activates a list item directly.
        if !cx.is_focused(Some(self.id.get())) {
            let handled = self
                .menu
                .as_mut()
                .is_some_and(|menu| menu.handle_action(cx, action).is_handled());
            return if handled && self.commit_picked(cx) {
                ActionOutcome::handled("advanced_ui.select")
            } else {
                ActionOutcome::Ignored
            };
        }
        match action {
            ActionId::Standard(StandardAction::MoveDown) => {
                self.move_highlight(MenuDirection::Forward, cx)
            }
            ActionId::Standard(StandardAction::MoveUp) => {
                self.move_highlight(MenuDirection::Backward, cx)
            }
            ActionId::Standard(StandardAction::Activate) => {
                self.activate(cx);
            }
            ActionId::Standard(StandardAction::Cancel) if self.menu.is_some() => self.close(cx),
            _ => return ActionOutcome::Ignored,
        }
        ActionOutcome::handled("advanced_ui.select")
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id.get() == id || self.menu.as_ref().is_some_and(|menu| menu.contains_id(id))
    }
}

fn validate_selected(items: &[MenuItem], selected: &str) {
    if !items.iter().any(|item| item.value() == selected) {
        panic!("select selected value must match an item");
    }
}

pub fn select<I, M>(label: impl Into<String>, items: I, selected: impl Into<String>) -> Select
where
    I: IntoIterator<Item = M>,
    M: Into<MenuItem>,
{
    Select::new(label, items, selected)
}

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;
