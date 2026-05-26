use crate::core::ElementId;

use super::error::AccessibilityError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibilityRole {
    Button,
    Checkbox,
    SegmentedControl,
    SegmentedOption,
    Text,
    ScrollArea,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibilityNode {
    id: ElementId,
    role: AccessibilityRole,
    label: Option<String>,
    value: Option<String>,
    enabled: bool,
    focused: bool,
    selected: Option<bool>,
    checked: Option<bool>,
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
            focused: false,
            selected: None,
            checked: None,
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

    pub fn a11y_focused(&self) -> bool {
        self.focused
    }

    pub fn a11y_selected(&self) -> Option<bool> {
        self.selected
    }

    pub fn a11y_checked(&self) -> Option<bool> {
        self.checked
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

    pub fn with_child(mut self, child: AccessibilityNode) -> Self {
        self.children.push(child);
        self
    }

    pub fn with_children(mut self, children: Vec<AccessibilityNode>) -> Self {
        self.children.extend(children);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
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
