//! The menu bar and context menus the app asked for, and the actions chosen
//! from them, waiting for the platform and the next frame. The platform takes
//! menu changes after every pass over its windows, so asking for them does
//! not need a frame.

use super::AppContext;
use crate::core::action::ActionId;
use crate::core::geometry::Point;
use crate::core::menu::{ContextMenuRequest, Menu};
use crate::core::window::WindowId;

#[derive(Debug, Default)]
pub(crate) struct MenuState {
    bar: Vec<Menu>,
    /// The platform has not shown `bar` yet.
    bar_changed: bool,
    context_menu: Option<ContextMenuRequest>,
    chosen: Vec<ActionId>,
    /// The window whose next frame routes `chosen`.
    chosen_for: Option<WindowId>,
}

impl AppContext {
    /// Replace the application menu bar. Call it again to change items'
    /// labels, enabled or checked state; a render may call it every time, and
    /// the platform rebuilds the bar only when it changed.
    pub fn set_menus(&mut self, menus: Vec<Menu>) {
        if self.menus.bar != menus {
            self.menus.bar = menus;
            self.menus.bar_changed = true;
        }
    }

    /// The menu bar last set with [`Self::set_menus`].
    pub fn menus(&self) -> &[Menu] {
        &self.menus.bar
    }

    /// Show `menu` at `position` in the window whose frame runs now, as a
    /// right click does. A chosen item's action is routed like a menu bar one.
    pub fn show_context_menu(&mut self, menu: Menu, position: Point) {
        self.menus.context_menu = Some(ContextMenuRequest {
            window: self.active_window(),
            menu,
            position,
        });
    }

    /// Whether the menu bar changed since the platform last showed it.
    pub(crate) fn take_menu_bar_update(&mut self) -> bool {
        std::mem::take(&mut self.menus.bar_changed)
    }

    pub(crate) fn take_context_menu(&mut self) -> Option<ContextMenuRequest> {
        self.menus.context_menu.take()
    }

    /// An item was chosen from a native menu; its action runs in the next
    /// frame of the window whose frame runs now.
    pub(crate) fn queue_menu_action(&mut self, action: ActionId) {
        self.menus.chosen.push(action);
        self.menus.chosen_for = self.active_window();
        self.request_redraw();
    }

    /// The chosen actions, in the frame of the window they were queued for.
    pub(crate) fn take_menu_actions(&mut self) -> Vec<ActionId> {
        if self.menus.chosen_for != self.active_window() {
            return Vec::new();
        }
        std::mem::take(&mut self.menus.chosen)
    }
}
