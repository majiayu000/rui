use super::model::{Tab, validate_panel_value};
use super::tab_list::TabList;
use super::tab_panel::TabPanel;
use crate::advanced_ui::tokens::{ControlSize, Theme};
use crate::core::ElementId;
use crate::core::accessibility::{AccessibilityContext, AccessibilityError, AccessibilityNode};
use crate::core::event::ScrollEvent;
use crate::core::geometry::Size;
use crate::core::style::{AlignItems, FlexDirection, Style};
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, style_to_taffy,
};
use taffy::prelude::NodeId;

pub struct Tabs {
    id: ElementId,
    tab_list: TabList,
    panels: Vec<TabPanel>,
    theme: Theme,
    style: Style,
    tab_list_node: Option<NodeId>,
    selected_panel_node: Option<NodeId>,
    selected_panel_index: Option<usize>,
}

impl Tabs {
    pub fn new<I, T>(tabs: I, selected: impl Into<String>) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<Tab>,
    {
        let theme = Theme::default();
        let mut style = Style::new();
        style.flex_direction = FlexDirection::Column;
        style.align_items = AlignItems::Stretch;
        style.gap = theme.control_gap();

        Self {
            id: ElementId::new(),
            tab_list: TabList::new(tabs, selected).theme(theme),
            panels: Vec::new(),
            theme,
            style,
            tab_list_node: None,
            selected_panel_node: None,
            selected_panel_index: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self
    }

    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        self.tab_list = self.tab_list.accessibility_label(label);
        self
    }

    pub fn selected(mut self, value: impl Into<String>) -> Self {
        self.tab_list = self.tab_list.selected(value);
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.tab_list = self.tab_list.size(size);
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self.style.gap = theme.control_gap();
        self.tab_list = self.tab_list.theme(theme);
        self.panels = self
            .panels
            .into_iter()
            .map(|panel| panel.theme(theme))
            .collect();
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.tab_list = self.tab_list.disabled(disabled);
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.tab_list = self.tab_list.read_only(read_only);
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.tab_list = self.tab_list.invalid(invalid);
        self
    }

    pub fn on_change(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.tab_list = self.tab_list.on_change(handler);
        self
    }

    pub fn panel(mut self, panel: TabPanel) -> Self {
        validate_panel_value(self.tab_list.tabs(), panel.value());
        self.panels.push(panel.theme(self.theme));
        self
    }

    pub fn panels<I>(mut self, panels: I) -> Self
    where
        I: IntoIterator<Item = TabPanel>,
    {
        for panel in panels {
            validate_panel_value(self.tab_list.tabs(), panel.value());
            self.panels.push(panel.theme(self.theme));
        }
        self
    }

    pub fn w(mut self, width: f32) -> Self {
        self.style.width = Some(width);
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        self.style.height = Some(height);
        self
    }

    pub fn size_box(mut self, size: impl Into<Size>) -> Self {
        let size = size.into();
        self.style.width = Some(size.width);
        self.style.height = Some(size.height);
        self
    }

    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.style.flex_grow = grow;
        self
    }

    pub fn spacing(mut self, spacing: f32) -> Self {
        self.style.gap = spacing;
        self
    }

    pub fn selected_value(&self) -> &str {
        self.tab_list.selected_value()
    }

    pub fn tab_list(&self) -> &TabList {
        &self.tab_list
    }

    fn selected_panel(&self) -> Option<(usize, &TabPanel)> {
        self.panels
            .iter()
            .enumerate()
            .find(|(_, panel)| panel.value() == self.selected_value())
    }
}

impl Element for Tabs {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let tab_list_node = self.tab_list.layout(cx);
        let selected_panel_index = self
            .panels
            .iter()
            .position(|panel| panel.value() == self.tab_list.selected_value());
        let selected_panel_node = selected_panel_index.map(|index| self.panels[index].layout(cx));

        let mut child_nodes = vec![tab_list_node];
        if let Some(node) = selected_panel_node {
            child_nodes.push(node);
        }

        let mut style = style_to_taffy(&self.style);
        style.flex_direction = taffy::FlexDirection::Column;
        style.align_items = Some(taffy::AlignItems::Stretch);
        let node = match cx.taffy.new_with_children(style, &child_nodes) {
            Ok(node) => node,
            Err(err) => panic!("failed to create advanced tabs layout node: {}", err),
        };

        self.tab_list_node = Some(tab_list_node);
        self.selected_panel_node = selected_panel_node;
        self.selected_panel_index = selected_panel_index;
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        if let Some(tab_list_node) = self.tab_list_node
            && let Some(bounds) = cx.child_bounds(tab_list_node)
        {
            let mut tab_cx = cx.with_bounds(bounds);
            self.tab_list.paint(&mut tab_cx);
        }

        if let (Some(panel_node), Some(panel_index)) =
            (self.selected_panel_node, self.selected_panel_index)
            && let Some(bounds) = cx.child_bounds(panel_node)
        {
            let mut panel_cx = cx.with_bounds(bounds);
            self.panels[panel_index].paint(&mut panel_cx);
        }
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        if let Some(tab_list_node) = self.tab_list_node
            && let Some(bounds) = cx.child_bounds(tab_list_node)
        {
            let mut tab_cx = cx.with_bounds(bounds);
            if self.tab_list.handle_pointer_event(&mut tab_cx, event) {
                return true;
            }
        }

        if let (Some(panel_node), Some(panel_index)) =
            (self.selected_panel_node, self.selected_panel_index)
            && let Some(bounds) = cx.child_bounds(panel_node)
        {
            let mut panel_cx = cx.with_bounds(bounds);
            return self.panels[panel_index].handle_pointer_event(&mut panel_cx, event);
        }

        false
    }

    fn handle_scroll_event(&mut self, cx: &mut EventContext, event: &ScrollEvent) -> bool {
        if let (Some(panel_node), Some(panel_index)) =
            (self.selected_panel_node, self.selected_panel_index)
            && let Some(bounds) = cx.child_bounds(panel_node)
        {
            let mut panel_cx = cx.with_bounds(bounds);
            return self.panels[panel_index].handle_scroll_event(&mut panel_cx, event);
        }

        false
    }

    fn handle_key_event(
        &mut self,
        cx: &mut EventContext,
        event: &crate::core::event::KeyEvent,
    ) -> bool {
        if let Some(focused) = cx.focused_id() {
            if self.tab_list.contains_id(focused) {
                return self.tab_list.handle_key_event(cx, event);
            }

            if let Some(panel_index) = self.selected_panel_index
                && self.panels[panel_index].contains_id(focused)
            {
                return self.panels[panel_index].handle_key_event(cx, event);
            }
        }

        false
    }

    fn accessibility_nodes(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Vec<AccessibilityNode>, AccessibilityError> {
        let mut nodes = self.tab_list.accessibility_nodes(cx)?;
        if let Some((_, panel)) = self.selected_panel() {
            nodes.extend(
                panel.accessibility_nodes_with_label(cx, self.tab_list.selected_tab_label())?,
            );
        }
        Ok(nodes)
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id == id
            || self.tab_list.contains_id(id)
            || self
                .selected_panel()
                .map(|(_, panel)| panel.contains_id(id))
                .unwrap_or(false)
    }
}
