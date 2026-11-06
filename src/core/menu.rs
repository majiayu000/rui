//! Native menus: the application menu bar and context menus, declared as a
//! tree of [`Menu`]s whose items run actions.
//!
//! Choosing an item routes its action as a key binding would: to the focused
//! element first, then to the app's action handlers. An item shows the keymap
//! shortcut bound to its action, and the keymap, not the menu, handles that
//! shortcut when it is pressed.

use crate::core::action::ActionId;
use crate::core::geometry::Point;
use crate::core::window::WindowId;

/// A menu: a top-level menu of the menu bar, a submenu or a context menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu {
    title: String,
    items: Vec<MenuItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItem {
    Action(MenuAction),
    Separator,
    Submenu(Menu),
}

/// An item that runs `action` when chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuAction {
    label: String,
    action: ActionId,
    enabled: bool,
    checked: bool,
}

impl Menu {
    /// The first menu of the menu bar is the application menu; macOS shows it
    /// under the application's name whatever `title` says.
    pub fn new(title: impl Into<String>) -> Self {
        let title = title.into();
        assert!(!title.trim().is_empty(), "menu title must not be empty");
        Self {
            title,
            items: Vec::new(),
        }
    }

    pub fn item(mut self, item: impl Into<MenuItem>) -> Self {
        self.items.push(item.into());
        self
    }

    pub fn separator(self) -> Self {
        self.item(MenuItem::Separator)
    }

    pub fn submenu(self, menu: Menu) -> Self {
        self.item(MenuItem::Submenu(menu))
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn items(&self) -> &[MenuItem] {
        &self.items
    }

    /// The item labelled `label` in this menu or its submenus, depth first.
    pub fn find(&self, label: &str) -> Option<&MenuAction> {
        self.items.iter().find_map(|item| match item {
            MenuItem::Action(action) if action.label == label => Some(action),
            MenuItem::Submenu(menu) => menu.find(label),
            _ => None,
        })
    }
}

impl MenuAction {
    pub fn new(label: impl Into<String>, action: impl Into<ActionId>) -> Self {
        let label = label.into();
        assert!(
            !label.trim().is_empty(),
            "menu item label must not be empty"
        );
        Self {
            label,
            action: action.into(),
            enabled: true,
            checked: false,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Show a checkmark beside the label.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn action(&self) -> &ActionId {
        &self.action
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }
}

impl From<MenuAction> for MenuItem {
    fn from(value: MenuAction) -> Self {
        Self::Action(value)
    }
}

impl From<Menu> for MenuItem {
    fn from(value: Menu) -> Self {
        Self::Submenu(value)
    }
}

/// A context menu asked for at `position` in `window`'s coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextMenuRequest {
    pub window: Option<WindowId>,
    pub menu: Menu,
    pub position: Point,
}
