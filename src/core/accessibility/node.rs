use crate::core::ElementId;

use super::error::AccessibilityError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibilityRole {
    Button,
    Checkbox,
    ProgressIndicator,
    SegmentedControl,
    SegmentedOption,
    Text,
    TextInput,
    ScrollArea,
    Toolbar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibilityAction {
    Activate,
    SetValue,
    ScrollForward,
    ScrollBackward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AccessibilityContext {
    focused: Option<ElementId>,
}

impl AccessibilityContext {
    pub fn new(focused: Option<ElementId>) -> Self {
        Self { focused }
    }

    pub fn a11y_focused_id(&self) -> Option<ElementId> {
        self.focused
    }

    pub fn a11y_has_focus(&self, id: ElementId) -> bool {
        self.focused == Some(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessibilityTextRange {
    start: usize,
    end: usize,
}

impl AccessibilityTextRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AccessibilityScrollPosition {
    offset_x: f32,
    offset_y: f32,
    max_x: f32,
    max_y: f32,
}

impl AccessibilityScrollPosition {
    pub fn new(offset_x: f32, offset_y: f32, max_x: f32, max_y: f32) -> Self {
        Self {
            offset_x,
            offset_y,
            max_x,
            max_y,
        }
    }

    pub fn offset_x(&self) -> f32 {
        self.offset_x
    }

    pub fn offset_y(&self) -> f32 {
        self.offset_y
    }

    pub fn max_x(&self) -> f32 {
        self.max_x
    }

    pub fn max_y(&self) -> f32 {
        self.max_y
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccessibilityNode {
    id: ElementId,
    role: AccessibilityRole,
    label: Option<String>,
    value: Option<String>,
    enabled: bool,
    read_only: bool,
    invalid: bool,
    focused: bool,
    selected: Option<bool>,
    checked: Option<bool>,
    text_caret: Option<usize>,
    text_selection: Option<AccessibilityTextRange>,
    text_composition: Option<AccessibilityTextRange>,
    scroll_position: Option<AccessibilityScrollPosition>,
    actions: Vec<AccessibilityAction>,
    children: Vec<AccessibilityNode>,
}

impl AccessibilityNode {
    pub fn new(id: ElementId, role: AccessibilityRole) -> Self {
        Self {
            id,
            role,
            label: None,
            value: None,
            enabled: true,
            read_only: false,
            invalid: false,
            focused: false,
            selected: None,
            checked: None,
            text_caret: None,
            text_selection: None,
            text_composition: None,
            scroll_position: None,
            actions: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn a11y_id(&self) -> ElementId {
        self.id
    }

    pub fn a11y_role(&self) -> AccessibilityRole {
        self.role
    }

    pub fn a11y_label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn a11y_value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    pub fn a11y_enabled(&self) -> bool {
        self.enabled
    }

    pub fn a11y_read_only(&self) -> bool {
        self.read_only
    }

    pub fn a11y_invalid(&self) -> bool {
        self.invalid
    }

    pub fn a11y_focused(&self) -> bool {
        self.focused
    }

    pub fn a11y_selected(&self) -> Option<bool> {
        self.selected
    }

    pub fn a11y_checked(&self) -> Option<bool> {
        self.checked
    }

    pub fn a11y_text_caret(&self) -> Option<usize> {
        self.text_caret
    }

    pub fn a11y_text_selection(&self) -> Option<AccessibilityTextRange> {
        self.text_selection
    }

    pub fn a11y_text_composition(&self) -> Option<AccessibilityTextRange> {
        self.text_composition
    }

    pub fn a11y_scroll_position(&self) -> Option<AccessibilityScrollPosition> {
        self.scroll_position
    }

    pub fn a11y_actions(&self) -> &[AccessibilityAction] {
        &self.actions
    }

    pub fn a11y_children(&self) -> &[AccessibilityNode] {
        &self.children
    }

    pub fn label_required(
        id: ElementId,
        role: AccessibilityRole,
        label: impl Into<String>,
    ) -> Result<Self, AccessibilityError> {
        let label = label.into();
        if label.trim().is_empty() {
            return Err(AccessibilityError::MissingLabel { role });
        }
        Ok(Self::new(id, role).with_label(label))
    }

    pub fn value_required(mut self, value: impl Into<String>) -> Result<Self, AccessibilityError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(AccessibilityError::MissingValue { role: self.role });
        }
        self.value = Some(value);
        Ok(self)
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn with_invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    pub fn with_focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn with_selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    pub fn with_checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    pub fn with_text_caret(mut self, caret: usize) -> Self {
        self.text_caret = Some(caret);
        self
    }

    pub fn with_text_selection(mut self, range: AccessibilityTextRange) -> Self {
        self.text_selection = Some(range);
        self
    }

    pub fn with_text_composition(mut self, range: AccessibilityTextRange) -> Self {
        self.text_composition = Some(range);
        self
    }

    pub fn with_scroll_position(mut self, position: AccessibilityScrollPosition) -> Self {
        self.scroll_position = Some(position);
        self
    }

    pub fn with_action(mut self, action: AccessibilityAction) -> Self {
        if !self.actions.contains(&action) {
            self.actions.push(action);
        }
        self
    }

    pub fn with_actions(mut self, actions: impl IntoIterator<Item = AccessibilityAction>) -> Self {
        for action in actions {
            if !self.actions.contains(&action) {
                self.actions.push(action);
            }
        }
        self
    }

    pub fn with_child(mut self, child: AccessibilityNode) -> Self {
        self.children.push(child);
        self
    }

    pub fn with_children(mut self, children: Vec<AccessibilityNode>) -> Self {
        self.children.extend(children);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct AccessibilityTree {
    roots: Vec<AccessibilityNode>,
}

impl AccessibilityTree {
    pub fn new(roots: Vec<AccessibilityNode>) -> Self {
        Self { roots }
    }

    pub fn roots(&self) -> &[AccessibilityNode] {
        &self.roots
    }

    pub fn find(&self, id: ElementId) -> Option<&AccessibilityNode> {
        self.roots.iter().find_map(|node| find_node(node, id))
    }
}

fn find_node(node: &AccessibilityNode, id: ElementId) -> Option<&AccessibilityNode> {
    if node.id == id {
        return Some(node);
    }
    node.children.iter().find_map(|child| find_node(child, id))
}
