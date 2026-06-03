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
use std::collections::HashSet;
use taffy::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataTreeItem {
    id: ElementId,
    value: String,
    label: String,
    detail: Option<String>,
    children: Vec<DataTreeItem>,
    expanded: bool,
    disabled: bool,
    read_only: bool,
    invalid: bool,
}

impl DataTreeItem {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        let value = value.into();
        let label = label.into();
        require_non_empty(&value, "data tree item value must not be empty");
        require_non_empty(&label, "data tree item label must not be empty");

        Self {
            id: ElementId::new(),
            value,
            label,
            detail: None,
            children: Vec::new(),
            expanded: true,
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
        require_non_empty(&detail, "data tree item detail must not be empty");
        self.detail = Some(detail);
        self
    }

    pub fn child(mut self, child: impl Into<DataTreeItem>) -> Self {
        self.children.push(child.into());
        self
    }

    pub fn children<I, O>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<DataTreeItem>,
    {
        self.children.extend(children.into_iter().map(Into::into));
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
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

    pub fn child_items(&self) -> &[DataTreeItem] {
        &self.children
    }

    fn can_activate(&self) -> bool {
        !self.disabled && !self.read_only
    }
}

impl From<(&str, &str)> for DataTreeItem {
    fn from((value, label): (&str, &str)) -> Self {
        Self::new(value, label)
    }
}

pub struct DataTree {
    id: ElementId,
    roots: Vec<DataTreeItem>,
    selected_value: Option<String>,
    accessibility_label: Option<String>,
    size: ControlSize,
    theme: Theme,
    row_height: f32,
    indent_width: f32,
    state: InteractionState,
    indexed_state: IndexedInteractionState,
    style: Style,
    on_select: Option<DataValueHandler>,
}

impl DataTree {
    pub fn new<I, O>(roots: I) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<DataTreeItem>,
    {
        let theme = Theme::default();
        let size = ControlSize::default();
        let roots: Vec<DataTreeItem> = roots.into_iter().map(Into::into).collect();
        validate_unique_tree_values(&roots);
        Self {
            id: ElementId::new(),
            roots,
            selected_value: None,
            accessibility_label: None,
            size,
            theme,
            row_height: theme.control_height(size),
            indent_width: 18.0,
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
        require_non_empty(&label, "data tree accessibility label must not be empty");
        self.accessibility_label = Some(label);
        self
    }

    pub fn selected(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        if !self
            .visible_items()
            .iter()
            .any(|visible| visible.item.value == value)
        {
            panic!("data tree selected value must match a visible item");
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
            "data tree row height must be finite and non-negative",
        );
        self.row_height = height;
        self
    }

    pub fn indent_width(mut self, width: f32) -> Self {
        validate_dimension(
            width,
            "data tree indent width must be finite and non-negative",
        );
        self.indent_width = width;
        self
    }

    pub fn w(mut self, width: f32) -> Self {
        validate_dimension(width, "data tree width must be finite and non-negative");
        self.style.width = Some(width);
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        validate_dimension(height, "data tree height must be finite and non-negative");
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

    pub fn visible_item_count(&self) -> usize {
        visible_tree_item_count(&self.roots)
    }

    pub fn hovered_index(&self) -> Option<usize> {
        self.indexed_state.hovered_index()
    }

    fn visible_items(&self) -> Vec<VisibleTreeItem<'_>> {
        let mut visible = Vec::new();
        collect_visible_tree_items(&self.roots, 0, &mut visible);
        visible
    }

    fn index_at(&self, position: Point, bounds: crate::core::geometry::Bounds) -> Option<usize> {
        index_at_row(bounds, position, self.row_height, self.visible_item_count())
    }

    fn interactive_index(&self, index: Option<usize>) -> Option<usize> {
        let visible = self.visible_items();
        index.filter(|&index| {
            self.state.can_activate()
                && visible
                    .get(index)
                    .map(|visible| visible.item.can_activate())
                    .unwrap_or(false)
        })
    }
}

impl Element for DataTree {
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
                .unwrap_or(self.row_height * self.visible_item_count() as f32),
        );
        cx.taffy
            .new_leaf(style)
            .expect("failed to create advanced data tree layout node")
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

        for (index, visible) in self.visible_items().iter().enumerate() {
            let Some(row_bounds) = row_bounds(bounds, self.row_height, index) else {
                continue;
            };
            cx.register_hit_region(visible.item.id, row_bounds);
            let marker = tree_marker(visible.item);
            paint_data_row(
                cx,
                row_bounds,
                DataRowPaint {
                    label: visible.item.label(),
                    detail: visible.item.detail.as_deref(),
                    depth: visible.depth,
                    indent_width: self.indent_width,
                    marker,
                    selected: self.selected_value.as_deref() == Some(visible.item.value()),
                    hovered: self.indexed_state.hovered_index() == Some(index),
                    pressed: self.indexed_state.pressed_index() == Some(index),
                    disabled: self.state.disabled() || visible.item.disabled,
                    invalid: self.state.invalid() || visible.item.invalid,
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
                    role: AccessibilityRole::DataTree,
                })?;
        let mut node =
            AccessibilityNode::label_required(self.id, AccessibilityRole::DataTree, label)?
                .with_enabled(!self.state.disabled())
                .with_read_only(self.state.read_only())
                .with_invalid(self.state.invalid())
                .with_focused(cx.a11y_has_focus(self.id));
        if let Some(selected) = &self.selected_value {
            node = node.with_value(selected.clone());
        }
        for item in &self.roots {
            node = node.with_child(tree_item_accessibility(
                item,
                &self.state,
                self.selected_value.as_deref(),
            )?);
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
                let visible = self.visible_items();
                let item = visible[release.released_index.expect("selected tree index")].item;
                let value = item.value.clone();
                let label = item.label.clone();
                if self.selected_value.as_deref() != Some(value.as_str()) {
                    self.selected_value = Some(value.clone());
                    if let Some(handler) = &self.on_select {
                        handler(&value);
                    }
                }
                cx.announce_accessibility_action(self.id, format!("{label} selected"));
                cx.request_redraw();
                true
            }
        }
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id == id || tree_contains_id(&self.roots, id)
    }
}

struct VisibleTreeItem<'a> {
    item: &'a DataTreeItem,
    depth: usize,
}

fn tree_marker(item: &DataTreeItem) -> Option<&'static str> {
    if item.children.is_empty() {
        None
    } else if item.expanded {
        Some("v")
    } else {
        Some(">")
    }
}

fn visible_tree_item_count(items: &[DataTreeItem]) -> usize {
    items
        .iter()
        .map(|item| {
            1 + if item.expanded {
                visible_tree_item_count(&item.children)
            } else {
                0
            }
        })
        .sum()
}

fn collect_visible_tree_items<'a>(
    items: &'a [DataTreeItem],
    depth: usize,
    visible: &mut Vec<VisibleTreeItem<'a>>,
) {
    for item in items {
        visible.push(VisibleTreeItem { item, depth });
        if item.expanded {
            collect_visible_tree_items(&item.children, depth + 1, visible);
        }
    }
}

fn tree_contains_id(items: &[DataTreeItem], id: ElementId) -> bool {
    items
        .iter()
        .any(|item| item.id == id || tree_contains_id(&item.children, id))
}

fn tree_item_accessibility(
    item: &DataTreeItem,
    state: &InteractionState,
    selected_value: Option<&str>,
) -> Result<AccessibilityNode, AccessibilityError> {
    let mut node =
        AccessibilityNode::label_required(item.id, AccessibilityRole::DataTreeItem, &item.label)?
            .value_required(&item.value)?
            .with_selected(selected_value == Some(item.value()))
            .with_enabled(!state.disabled() && !item.disabled)
            .with_read_only(state.read_only() || item.read_only)
            .with_invalid(state.invalid() || item.invalid);
    if state.can_activate() && item.can_activate() {
        node = node.with_action(AccessibilityAction::Activate);
    }
    if item.expanded {
        for child in &item.children {
            node = node.with_child(tree_item_accessibility(child, state, selected_value)?);
        }
    }
    Ok(node)
}

fn validate_unique_tree_values(items: &[DataTreeItem]) {
    fn visit<'a>(items: &'a [DataTreeItem], seen: &mut HashSet<&'a str>) {
        for item in items {
            if !seen.insert(item.value()) {
                panic!("data tree item values must be unique");
            }
            visit(item.child_items(), seen);
        }
    }

    let mut seen = HashSet::new();
    visit(items, &mut seen);
}

pub fn data_tree<I, O>(roots: I) -> DataTree
where
    I: IntoIterator<Item = O>,
    O: Into<DataTreeItem>,
{
    DataTree::new(roots)
}
