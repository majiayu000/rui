//! Actions a [`Div`] handles: key bindings and menu items that reach it.

use super::Div;
use crate::core::action::{ActionId, ActionOutcome};

pub(super) type ActionHandlers = Vec<(ActionId, Box<dyn Fn()>)>;

impl Div {
    /// Run `handler` when `action` reaches this div and nothing inside it
    /// handled it first. An action goes to the focused element and the divs
    /// around it first, then to the rest of the tree, then to the app's
    /// handlers. Bind a key to the action with [`crate::AppContext::keymap_mut`].
    ///
    /// With [`crate::ViewContext::listener`] the handler updates its view:
    ///
    /// ```ignore
    /// div().on_action(ActionId::custom("todo.add"), cx.listener(|this, _| this.add()))
    /// ```
    pub fn on_action(mut self, action: impl Into<ActionId>, handler: impl Fn() + 'static) -> Self {
        self.actions.push((action.into(), Box::new(handler)));
        self
    }

    pub(super) fn run_action(&self, action: &ActionId) -> ActionOutcome {
        match self.actions.iter().find(|(handled, _)| handled == action) {
            Some((_, handler)) => {
                handler();
                ActionOutcome::handled("div")
            }
            None => ActionOutcome::Ignored,
        }
    }

    /// Whether it has handlers, which cannot be compared between trees.
    pub(super) fn has_handlers(&self) -> bool {
        self.on_click.is_some()
            || self.on_hover.is_some()
            || self.on_context_menu.is_some()
            || self.drag.is_some()
            || !self.actions.is_empty()
    }
}
