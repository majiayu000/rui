use crate::advanced_ui::state::require_non_empty;
use crate::core::ElementId;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    pub(crate) id: ElementId,
    pub(crate) value: String,
    pub(crate) label: String,
    pub(crate) disabled: bool,
}

impl Tab {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        let value = value.into();
        let label = label.into();
        require_non_empty(&value, "tab value must not be empty");
        require_non_empty(&label, "tab label must not be empty");

        Self {
            id: ElementId::new(),
            value,
            label,
            disabled: false,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn tab_id(&self) -> ElementId {
        self.id
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

impl From<(&str, &str)> for Tab {
    fn from((value, label): (&str, &str)) -> Self {
        Self::new(value, label)
    }
}

pub(crate) fn validate_tabs(tabs: &[Tab], selected: &str) {
    if tabs.is_empty() {
        panic!("tab list requires at least one tab");
    }
    let mut seen = HashSet::new();
    for tab in tabs {
        if !seen.insert(tab.value()) {
            panic!("tab values must be unique");
        }
    }
    validate_selected_tab(tabs, selected);
}

pub(crate) fn validate_selected_tab(tabs: &[Tab], selected: &str) {
    let tab = tabs
        .iter()
        .find(|tab| tab.value == selected)
        .unwrap_or_else(|| panic!("tab list selected value must match a tab"));
    if tab.disabled {
        panic!("tab list selected tab must be enabled");
    }
}

pub(crate) fn validate_panel_value(tabs: &[Tab], value: &str) {
    if !tabs.iter().any(|tab| tab.value == value) {
        panic!("tab panel value must match a tab");
    }
}
