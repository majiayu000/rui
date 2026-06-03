use crate::advanced_ui::state::{IndexedInteractionState, InteractionState, require_non_empty};
use crate::advanced_ui::tokens::{ControlSize, ControlState, Theme};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityRole,
};
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

const MENU_PADDING: f32 = 6.0;
const MENU_MIN_WIDTH: f32 = 160.0;

type MenuSelectHandler = Box<dyn Fn(&str)>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    value: String,
    label: String,
    disabled: bool,
}

impl MenuItem {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        let value = value.into();
        let label = label.into();
        require_non_empty(&value, "menu item value must not be empty");
        require_non_empty(&label, "menu item label must not be empty");

        Self {
            value,
            label,
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn is_disabled(&self) -> bool {
        self.disabled
    }
}

impl From<(&str, &str)> for MenuItem {
    fn from((value, label): (&str, &str)) -> Self {
        Self::new(value, label)
    }
}

pub struct Menu {
    id: ElementId,
    label: String,
    items: Vec<MenuItem>,
    item_ids: Vec<ElementId>,
    selected: Option<String>,
    size: ControlSize,
    theme: Theme,
    state: InteractionState,
    indexed_state: IndexedInteractionState,
    style: Style,
    on_select: Option<MenuSelectHandler>,
}

impl Menu {
    pub fn new<I, M>(label: impl Into<String>, items: I) -> Self
    where
        I: IntoIterator<Item = M>,
        M: Into<MenuItem>,
    {
        let label = label.into();
        require_non_empty(&label, "menu accessibility label must not be empty");
        let items: Vec<MenuItem> = items.into_iter().map(Into::into).collect();
        validate_items(&items);
        let item_ids = items.iter().map(|_| ElementId::new()).collect();

        let theme = Theme::default();
        let mut style = Style::new();
        style.border.radius = Corners::all(theme.control_radius());

        Self {
            id: ElementId::new(),
            label,
            items,
            item_ids,
            selected: None,
            size: ControlSize::default(),
            theme,
            state: InteractionState::default(),
            indexed_state: IndexedInteractionState::default(),
            style,
            on_select: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self
    }

    pub fn selected(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        validate_selected(&self.items, &value);
        self.selected = Some(value);
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

    pub fn on_select(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_select = Some(Box::new(handler));
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn selected_value(&self) -> Option<&str> {
        self.selected.as_deref()
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

    fn preferred_width(&self) -> f32 {
        self.items
            .iter()
            .map(|item| {
                item.label.chars().count() as f32 * self.theme.text_size(self.size) * 0.56
                    + self.theme.horizontal_padding(self.size) * 2.0
            })
            .fold(MENU_MIN_WIDTH, f32::max)
    }

    fn item_height(&self) -> f32 {
        self.theme.control_height(self.size)
    }

    fn index_at(&self, bounds: Bounds, position: Point) -> Option<usize> {
        if !bounds.contains(position) {
            return None;
        }
        let top = bounds.y() + MENU_PADDING;
        let bottom = bounds.y() + bounds.height() - MENU_PADDING;
        if position.y < top || position.y >= bottom {
            return None;
        }
        let index = ((position.y - top) / self.item_height()).floor() as usize;
        (index < self.items.len()).then_some(index)
    }

    fn can_activate_index(&self, index: usize) -> bool {
        self.state.can_activate() && !self.items[index].disabled
    }

    fn interactive_index_at(&self, bounds: Bounds, position: Point) -> Option<usize> {
        self.index_at(bounds, position)
            .filter(|index| self.can_activate_index(*index))
    }

    fn activate_index(&mut self, index: usize, cx: &EventContext) -> bool {
        if !self.can_activate_index(index) {
            return false;
        }

        let value = self.items[index].value.clone();
        let label = self.items[index].label.clone();
        self.selected = Some(value.clone());
        if let Some(handler) = &self.on_select {
            handler(&value);
        }
        cx.announce_accessibility_action(self.id, format!("{label} selected"));
        cx.request_redraw();
        true
    }

    fn next_enabled_index(&self, from: Option<usize>, direction: MenuDirection) -> Option<usize> {
        if self.items.iter().all(|item| item.disabled) {
            return None;
        }

        let len = self.items.len();
        let start = match (from, direction) {
            (Some(index), MenuDirection::Forward) => (index + 1) % len,
            (Some(index), MenuDirection::Backward) => (index + len - 1) % len,
            (None, MenuDirection::Forward) => 0,
            (None, MenuDirection::Backward) => len - 1,
        };

        for offset in 0..len {
            let index = match direction {
                MenuDirection::Forward => (start + offset) % len,
                MenuDirection::Backward => (start + len - offset) % len,
            };
            if !self.items[index].disabled {
                return Some(index);
            }
        }
        None
    }
}

impl Element for Menu {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let mut style = style_to_taffy(&self.style);
        style.size.width = Dimension::Length(self.preferred_width());
        style.size.height =
            Dimension::Length(self.item_height() * self.items.len() as f32 + MENU_PADDING * 2.0);

        cx.taffy
            .new_leaf(style)
            .unwrap_or_else(|err| panic!("failed to create advanced menu layout node: {err}"))
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        cx.register_hit_region(self.id, bounds);

        cx.paint(Primitive::Quad {
            bounds,
            background: self.theme.colors.surface.to_rgba(),
            border_color: self
                .theme
                .state_border_color(self.state.into(), self.theme.colors.border)
                .to_rgba(),
            border_widths: Edges::all(1.0),
            corner_radii: self.style.border.radius,
        });

        for index in 0..self.items.len() {
            let item_bounds = Bounds::from_xywh(
                bounds.x() + MENU_PADDING,
                bounds.y() + MENU_PADDING + index as f32 * self.item_height(),
                bounds.width() - MENU_PADDING * 2.0,
                self.item_height(),
            );
            let hovered = self.indexed_state.hovered_index() == Some(index);
            let selected = self.selected.as_deref() == Some(self.items[index].value());
            let disabled = self.state.disabled() || self.items[index].disabled;

            if hovered || selected {
                let item_background = if selected {
                    self.theme.colors.primary.rest.background.with_alpha(0.14)
                } else {
                    self.theme.surface_color_for_state(ControlState {
                        hovered: true,
                        ..self.state.into()
                    })
                };
                cx.paint(Primitive::Quad {
                    bounds: item_bounds,
                    background: item_background.to_rgba(),
                    border_color: Color::TRANSPARENT.to_rgba(),
                    border_widths: Edges::ZERO,
                    corner_radii: Corners::all(self.theme.control_radius()),
                });
            }

            cx.paint(Primitive::Text {
                bounds: item_bounds,
                content: self.items[index].label.clone(),
                color: self
                    .theme
                    .colors
                    .text
                    .with_alpha(if disabled { 0.5 } else { 1.0 })
                    .to_rgba(),
                font_size: self.theme.text_size(self.size),
                font_weight: if selected {
                    self.theme.typography.selected_weight
                } else {
                    self.theme.typography.label_weight
                },
                font_family: None,
                line_height: self.theme.typography.line_height,
                align: crate::elements::text::TextAlign::Left,
            });
        }
    }

    fn accessibility(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let mut node =
            AccessibilityNode::label_required(self.id, AccessibilityRole::Menu, &self.label)?
                .with_enabled(!self.state.disabled())
                .with_read_only(self.state.read_only())
                .with_invalid(self.state.invalid())
                .with_focused(cx.a11y_has_focus(self.id));
        if let Some(selected) = &self.selected {
            node = node.with_value(selected.clone());
        }

        for (item, id) in self.items.iter().zip(self.item_ids.iter().copied()) {
            let selected = self.selected.as_deref() == Some(item.value());
            let enabled = !self.state.disabled() && !item.disabled;
            let mut child =
                AccessibilityNode::label_required(id, AccessibilityRole::MenuItem, &item.label)?
                    .value_required(&item.value)?
                    .with_selected(selected)
                    .with_enabled(enabled)
                    .with_read_only(self.state.read_only())
                    .with_invalid(self.state.invalid());
            if enabled && !self.state.read_only() {
                child = child.with_action(AccessibilityAction::Activate);
            }
            node = node.with_child(child);
        }

        Ok(Some(node))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let index = self.interactive_index_at(cx.bounds(), event.position);
        match event.kind {
            PointerEventKind::Move => {
                self.indexed_state.update_hover(index, cx, self.state);
                false
            }
            PointerEventKind::Down => self.indexed_state.press(index, cx, self.state),
            PointerEventKind::Up => {
                let release = self.indexed_state.release(index, cx, self.state);
                if release.activated {
                    if let Some(index) = release.released_index {
                        return self.activate_index(index, cx);
                    }
                }
                false
            }
        }
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        if !self.state.can_activate() {
            self.indexed_state.clear();
            return false;
        }

        match event.key {
            KeyCode::ArrowDown => {
                let next = self
                    .next_enabled_index(self.indexed_state.hovered_index(), MenuDirection::Forward);
                self.indexed_state.update_hover(next, cx, self.state);
                next.is_some()
            }
            KeyCode::ArrowUp => {
                let next = self.next_enabled_index(
                    self.indexed_state.hovered_index(),
                    MenuDirection::Backward,
                );
                self.indexed_state.update_hover(next, cx, self.state);
                next.is_some()
            }
            KeyCode::Enter | KeyCode::Space => self
                .indexed_state
                .hovered_index()
                .map(|index| self.activate_index(index, cx))
                .unwrap_or(false),
            KeyCode::Escape => {
                if self.indexed_state.hovered_index().is_some() {
                    self.indexed_state.clear();
                    cx.request_redraw();
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum MenuDirection {
    Forward,
    Backward,
}

fn validate_items(items: &[MenuItem]) {
    if items.is_empty() {
        panic!("menu requires at least one item");
    }
}

fn validate_selected(items: &[MenuItem], selected: &str) {
    if !items.iter().any(|item| item.value == selected) {
        panic!("menu selected value must match an item");
    }
}

pub fn menu<I, M>(label: impl Into<String>, items: I) -> Menu
where
    I: IntoIterator<Item = M>,
    M: Into<MenuItem>,
{
    Menu::new(label, items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advanced_ui::tokens::ThemeDensity;
    use crate::core::event::{Modifiers, MouseButton};
    use crate::core::geometry::Size;
    use std::cell::RefCell;
    use std::rc::Rc;
    use taffy::TaffyTree;

    fn pointer(kind: PointerEventKind, x: f32, y: f32) -> PointerEvent {
        PointerEvent {
            kind,
            position: Point::new(x, y),
            button: Some(MouseButton::Left),
        }
    }

    fn event_context<'a>(
        taffy: &'a TaffyTree<ElementId>,
        focused: &'a mut Option<ElementId>,
    ) -> EventContext<'a> {
        EventContext::new(Bounds::from_xywh(0.0, 0.0, 180.0, 120.0), taffy, focused)
    }

    #[test]
    fn advanced_ui_menu_selects_item_from_pointer_release() {
        let selected = Rc::new(RefCell::new(Vec::<String>::new()));
        let selected_ref = Rc::clone(&selected);
        let mut menu = Menu::new("File", [("new", "New"), ("open", "Open")])
            .on_select(move |value| selected_ref.borrow_mut().push(value.to_string()));
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = event_context(&taffy, &mut focused);

        assert!(menu.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 12.0, 48.0)));
        assert!(menu.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Up, 12.0, 48.0)));

        assert_eq!(menu.selected_value(), Some("open"));
        assert_eq!(&*selected.borrow(), &["open".to_string()]);
        assert_eq!(
            cx.take_accessibility_announcements()[0].message(),
            "Open selected"
        );
    }

    #[test]
    fn advanced_ui_menu_skips_disabled_items_for_pointer_and_keyboard() {
        let mut menu = Menu::new(
            "File",
            [
                MenuItem::new("new", "New").disabled(true),
                MenuItem::new("open", "Open"),
            ],
        );
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = event_context(&taffy, &mut focused);

        assert!(!menu.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 12.0, 12.0)));
        assert!(menu.handle_key_event(
            &mut cx,
            &KeyEvent::new(KeyCode::ArrowDown, Modifiers::none())
        ));
        assert_eq!(menu.hovered_index(), Some(1));
        assert!(menu.handle_key_event(&mut cx, &KeyEvent::new(KeyCode::Enter, Modifiers::none())));
        assert_eq!(menu.selected_value(), Some("open"));
    }

    #[test]
    fn advanced_ui_menu_disabled_or_read_only_blocks_activation() {
        for mut menu in [
            Menu::new("File", [("new", "New")]).disabled(true),
            Menu::new("File", [("new", "New")]).read_only(true),
        ] {
            let taffy = TaffyTree::<ElementId>::new();
            let mut focused = None;
            let mut cx = event_context(&taffy, &mut focused);

            assert!(
                !menu.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 12.0, 12.0),)
            );
            assert!(!menu.handle_key_event(
                &mut cx,
                &KeyEvent::new(KeyCode::ArrowDown, Modifiers::none()),
            ));
            assert!(menu.selected_value().is_none());
        }
    }

    #[test]
    fn advanced_ui_menu_accessibility_exposes_item_tree() {
        let id = ElementId::from(700);
        let menu = Menu::new("File", [("new", "New"), ("open", "Open")])
            .id(id)
            .selected("open");

        let nodes = menu
            .accessibility_nodes(&AccessibilityContext::new(Some(id)))
            .expect("menu accessibility should build");

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].a11y_role(), AccessibilityRole::Menu);
        assert_eq!(nodes[0].a11y_label(), Some("File"));
        assert_eq!(nodes[0].a11y_value(), Some("open"));
        assert!(nodes[0].a11y_focused());
        assert_eq!(nodes[0].a11y_children().len(), 2);
        assert_eq!(
            nodes[0].a11y_children()[1].a11y_role(),
            AccessibilityRole::MenuItem
        );
        assert_eq!(nodes[0].a11y_children()[1].a11y_selected(), Some(true));
    }

    #[test]
    fn advanced_ui_menu_lays_out_from_item_count() {
        let mut menu = Menu::new("File", [("new", "New"), ("open", "Open")]);
        let mut taffy = TaffyTree::<ElementId>::new();
        let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(240.0, 160.0));
        let node = menu.layout(&mut layout_cx);
        taffy
            .compute_layout(
                node,
                taffy::Size {
                    width: AvailableSpace::Definite(240.0),
                    height: AvailableSpace::Definite(160.0),
                },
            )
            .expect("menu layout should compute");
        let layout = taffy.layout(node).expect("menu layout should be available");

        assert!(layout.size.width >= MENU_MIN_WIDTH);
        assert_eq!(layout.size.height, 84.0);
    }

    #[test]
    fn advanced_ui_menu_theme_density_changes_layout_tokens() {
        let theme = Theme::light().with_density(ThemeDensity { scale: 1.5 });
        let mut menu = Menu::new("File", [("new", "New"), ("open", "Open")]).theme(theme);
        let mut taffy = TaffyTree::<ElementId>::new();
        let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(240.0, 180.0));
        let node = menu.layout(&mut layout_cx);
        taffy
            .compute_layout(
                node,
                taffy::Size {
                    width: AvailableSpace::Definite(240.0),
                    height: AvailableSpace::Definite(180.0),
                },
            )
            .expect("menu layout should compute");
        let layout = taffy.layout(node).expect("menu layout should be available");

        assert_eq!(layout.size.height, 120.0);
    }

    #[test]
    #[should_panic(expected = "menu requires at least one item")]
    fn advanced_ui_menu_rejects_empty_items() {
        drop(Menu::new("File", Vec::<MenuItem>::new()));
    }
}
