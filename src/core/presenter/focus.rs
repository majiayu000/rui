//! Applying the focus changes app code asked for to the presented tree, and
//! moving focus with the window as it loses and gets back key status.

use super::Presenter;
use crate::core::action::{can_refocus, sync_focus};
use crate::core::app::AppContext;
use crate::core::app::FocusRequest;
use crate::core::event::{Event, FocusEvent};
use crate::elements::Element;

impl<E> Presenter<E>
where
    E: Element,
{
    /// Apply the focus change asked for with `AppContext::focus` or `blur`, and
    /// let the elements that gain or lose focus react. Returns whether the
    /// focus changed.
    pub(crate) fn apply_focus_request(&mut self, context: &mut AppContext) -> bool {
        let next = match context.take_focus_request() {
            None => return false,
            Some(FocusRequest::Blur) => None,
            Some(FocusRequest::Focus(id)) if self.root().contains_id(id) => Some(id),
            Some(FocusRequest::Focus(id)) => {
                log::error!("focus asked for element {id:?}, which is not in this window");
                return false;
            }
        };
        // App code chose the focus, so the window's return must not undo it.
        self.focus_before_blur = None;
        if self.focused_element() == next {
            return false;
        }
        // Blur first, so the old element's on_blur runs before the new one's on_focus.
        if self.focused_element().is_some() {
            self.set_focused_element(None);
            self.with_event_context(|root, event_cx| sync_focus(root, event_cx));
        }
        if next.is_some() {
            self.set_focused_element(next);
            self.with_event_context(|root, event_cx| sync_focus(root, event_cx));
        }
        true
    }

    /// Tell the tree that the window lost or got back key status, and move
    /// focus with it, as AppKit's first responder, browsers and GPUI do.
    ///
    /// Losing key status blurs the focused element, so it paints no ring or
    /// caret and takes no keys while another window has them, and remembers
    /// it. Getting key status back focuses that element again, unless
    /// something has focus by then or the element can no longer take it: it
    /// left the tree, was disabled, or is outside a modal that opened.
    /// Returns whether the tree handled the window event, as
    /// [`Self::handle_window_event`] does.
    pub(crate) fn handle_window_focus(&mut self, focused: bool) -> bool {
        if !focused {
            if let Some(id) = self.focused_element() {
                self.focus_before_blur = Some(id);
                self.set_focused_element(None);
                self.with_event_context(|root, event_cx| sync_focus(root, event_cx));
            }
            return self.handle_window_event(&Event::Blur(FocusEvent { focused: false }));
        }
        let handled = self.handle_window_event(&Event::Focus(FocusEvent { focused: true }));
        let modal = self
            .scene
            .overlays()
            .iter()
            .rev()
            .find(|overlay| overlay.modal)
            .map(|overlay| overlay.owner);
        if let Some(id) = self.focus_before_blur.take()
            && self.focused_element().is_none()
            && can_refocus(&self.root, id, modal)
        {
            self.set_focused_element(Some(id));
            self.with_event_context(|root, event_cx| sync_focus(root, event_cx));
        }
        handled
    }
}
