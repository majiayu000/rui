//! Actions a [`Div`] handles: key bindings and menu items that reach it.

use super::Div;
use crate::core::action::{ActionId, ActionOutcome};

pub(super) type ActionHandlers = Vec<(ActionId, Box<dyn Fn()>)>;

impl Div {
    /// Run `handler` when `action` reaches this div and nothing inside it
    /// handled it first. A div offers an action to the child the focused
    /// element is in, then to its other children from the last to the first,
    /// and runs its own handler when none of them handled it. So the focused
    /// element comes first, and a div beside it handles an action before a
    /// div around them both. The app's handlers come after the tree. Bind a
    /// key to the action with [`crate::AppContext::keymap_mut`].
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
