//! Routing the actions chosen from native menus through the presented tree.

use super::Presenter;
use crate::core::action::route_action;
use crate::core::app::AppContext;
use crate::elements::Element;

impl<E> Presenter<E>
where
    E: Element,
{
    /// Route the actions chosen from menus since the last frame to the focused
    /// element, then the app's action handlers. Returns whether the tree asked
    /// for a redraw or handled one.
    pub(crate) fn dispatch_menu_actions(&mut self, context: &mut AppContext) -> bool {
        let mut redraw = false;
        for action in context.take_menu_actions() {
            let (handled, redraw_requested) = self.with_event_context(|root, event_cx| {
                route_action(root, context, event_cx, &action)
            });
            if !handled {
                log::error!("menu action {action:?} had no handler");
            }
            redraw |= handled || redraw_requested;
        }
        redraw
    }
}
