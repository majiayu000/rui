use super::common::{
    DEFAULT_DATA_WIDTH, DataRowPaint, DataValueHandler, base_data_style, data_corners,
    index_at_row, paint_data_row, paint_surface, row_bounds, validate_dimension,
};
use crate::advanced_ui::state::{IndexedInteractionState, InteractionState, require_non_empty};
use crate::advanced_ui::tokens::{ControlSize, Theme};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityContext, AccessibilityError, AccessibilityNode,
    AccessibilityRole,
};
use crate::core::geometry::Point;
use crate::core::style::Style;
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
    style_to_taffy,
};
use crate::elements::text::TextAlign;
use taffy::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataListItem {
    id: ElementId,
    value: String,
    label: String,
    detail: Option<String>,
    disabled: bool,
    read_only: bool,
    invalid: bool,
}

impl DataListItem {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        let value = value.into();
        let label = label.into();
        require_non_empty(&value, "data list item value must not be empty");
        require_non_empty(&label, "data list item label must not be empty");

        Self {
            id: ElementId::new(),
            value,
            label,
            detail: None,
            disabled: false,
            read_only: false,
            invalid: false,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        let detail = detail.into();
        require_non_empty(&detail, "data list item detail must not be empty");
        self.detail = Some(detail);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn detail_text(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    fn can_activate(&self) -> bool {
        !self.disabled && !self.read_only
    }
}

impl From<(&str, &str)> for DataListItem {
    fn from((value, label): (&str, &str)) -> Self {
        Self::new(value, label)
    }
}

impl From<(&str, &str, &str)> for DataListItem {
    fn from((value, label, detail): (&str, &str, &str)) -> Self {
        Self::new(value, label).detail(detail)
    }
}

pub struct DataList {
    id: ElementId,
    items: Vec<DataListItem>,
    selected_value: Option<String>,
    accessibility_label: Option<String>,
    size: ControlSize,
    theme: Theme,
    row_height: f32,
    state: InteractionState,
    indexed_state: IndexedInteractionState,
    style: Style,
    on_select: Option<DataValueHandler>,
}

impl DataList {
    pub fn new<I, O>(items: I) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<DataListItem>,
    {
        let theme = Theme::default();
        let size = ControlSize::default();
        Self {
            id: ElementId::new(),
            items: items.into_iter().map(Into::into).collect(),
            selected_value: None,
            accessibility_label: None,
            size,
            theme,
            row_height: theme.control_height(size),
            state: InteractionState::default(),
            indexed_state: IndexedInteractionState::default(),
            style: base_data_style(data_corners(theme).top_left),
            on_select: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self
    }

    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        let label = label.into();
        require_non_empty(&label, "data list accessibility label must not be empty");
        self.accessibility_label = Some(label);
        self
    }

    pub fn item(mut self, item: impl Into<DataListItem>) -> Self {
        self.items.push(item.into());
        self
    }

    pub fn items<I, O>(mut self, items: I) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<DataListItem>,
    {
        self.items.extend(items.into_iter().map(Into::into));
        self
    }

    pub fn selected(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        if !self.items.iter().any(|item| item.value == value) {
            panic!("data list selected value must match an item");
        }
        self.selected_value = Some(value);
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self.row_height = self.theme.control_height(size);
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self.row_height = self.theme.control_height(self.size);
        self.style.border.radius = data_corners(self.theme);
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        validate_dimension(
            height,
            "data list row height must be finite and non-negative",
        );
        self.row_height = height;
        self
    }

    pub fn w(mut self, width: f32) -> Self {
        validate_dimension(width, "data list width must be finite and non-negative");
        self.style.width = Some(width);
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        validate_dimension(height, "data list height must be finite and non-negative");
        self.style.height = Some(height);
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

    pub fn selected_value(&self) -> Option<&str> {
        self.selected_value.as_deref()
    }

    pub fn item_count(&self) -> usize {
        self.items.len()
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

    fn index_at(&self, position: Point, bounds: crate::core::geometry::Bounds) -> Option<usize> {
        index_at_row(bounds, position, self.row_height, self.items.len())
    }

    fn interactive_index(&self, index: Option<usize>) -> Option<usize> {
        index.filter(|&index| self.state.can_activate() && self.items[index].can_activate())
    }
}

impl Element for DataList {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let mut style = style_to_taffy(&self.style);
        style.size.width = Dimension::Length(self.style.width.unwrap_or(DEFAULT_DATA_WIDTH));
        style.size.height = Dimension::Length(
            self.style
                .height
                .unwrap_or(self.row_height * self.items.len() as f32),
        );
        cx.taffy
            .new_leaf(style)
            .expect("failed to create advanced data list layout node")
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        cx.register_hit_region(self.id, bounds);
        paint_surface(
            cx,
            bounds,
            self.state.invalid(),
            self.state.disabled(),
            data_corners(self.theme),
            self.theme,
        );

        for (index, item) in self.items.iter().enumerate() {
            let Some(row_bounds) = row_bounds(bounds, self.row_height, index) else {
                continue;
            };
            cx.register_hit_region(item.id, row_bounds);
            paint_data_row(
                cx,
                row_bounds,
                DataRowPaint {
                    label: item.label(),
                    detail: item.detail_text(),
                    depth: 0,
                    indent_width: 0.0,
                    marker: None,
                    selected: self.selected_value.as_deref() == Some(item.value()),
                    hovered: self.indexed_state.hovered_index() == Some(index),
                    pressed: self.indexed_state.pressed_index() == Some(index),
                    disabled: self.state.disabled() || item.disabled,
                    invalid: self.state.invalid() || item.invalid,
                    size: self.size,
                    theme: self.theme,
                    align: TextAlign::Left,
                },
            );
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
                    role: AccessibilityRole::DataList,
                })?;
        let mut node =
            AccessibilityNode::label_required(self.id, AccessibilityRole::DataList, label)?
                .with_enabled(!self.state.disabled())
                .with_read_only(self.state.read_only())
                .with_invalid(self.state.invalid())
                .with_focused(cx.a11y_has_focus(self.id));
        if let Some(selected) = &self.selected_value {
            node = node.with_value(selected.clone());
        }
        for item in &self.items {
            let mut child = AccessibilityNode::label_required(
                item.id,
                AccessibilityRole::DataListItem,
                &item.label,
            )?
            .value_required(&item.value)?
            .with_selected(self.selected_value.as_deref() == Some(item.value()))
            .with_enabled(!self.state.disabled() && !item.disabled)
            .with_read_only(self.state.read_only() || item.read_only)
            .with_invalid(self.state.invalid() || item.invalid);
            if self.state.can_activate() && item.can_activate() {
                child = child.with_action(AccessibilityAction::Activate);
            }
            node = node.with_child(child);
        }
        Ok(Some(node))
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let index = self.interactive_index(self.index_at(event.position, cx.bounds()));
        match event.kind {
            PointerEventKind::Move => {
                self.indexed_state.update_hover(index, cx, self.state);
                false
            }
            PointerEventKind::Down => self.indexed_state.press(index, cx, self.state),
            PointerEventKind::Up => {
                let release = self.indexed_state.release(index, cx, self.state);
                if !release.activated {
                    return false;
                }
                let index = release.released_index.expect("selected data list index");
                let value = self.items[index].value.clone();
                if self.selected_value.as_deref() != Some(value.as_str()) {
                    self.selected_value = Some(value.clone());
                    if let Some(handler) = &self.on_select {
                        handler(&value);
                    }
                }
                cx.announce_accessibility_action(
                    self.id,
                    format!("{} selected", self.items[index].label),
                );
                cx.request_redraw();
                true
            }
        }
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id == id || self.items.iter().any(|item| item.id == id)
    }
}

pub fn data_list<I, O>(items: I) -> DataList
where
    I: IntoIterator<Item = O>,
    O: Into<DataListItem>,
{
    DataList::new(items)
}
