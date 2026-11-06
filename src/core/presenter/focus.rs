//! Applying the focus changes app code asked for to the presented tree.

use super::Presenter;
use crate::core::action::sync_focus;
use crate::core::app::AppContext;
use crate::core::app::FocusRequest;
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
}
