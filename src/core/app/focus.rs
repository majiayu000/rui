//! Focus changes app code asked for, applied by the window's presenter in its
//! next frame, and the focus that frame left, for app code to read.

use super::AppContext;
use crate::core::ElementId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusRequest {
    Focus(ElementId),
    Blur,
}

impl AppContext {
    /// Give keyboard focus to the element `id` in the window whose frame runs
    /// now. Its `on_focus` and the previous element's `on_blur` run in the
    /// next frame.
    pub fn focus(&mut self, id: ElementId) {
        self.frame.focus_request = Some(FocusRequest::Focus(id));
        self.request_redraw();
    }

    /// Take keyboard focus away from whatever has it in that window.
    pub fn blur(&mut self) {
        self.frame.focus_request = Some(FocusRequest::Blur);
        self.request_redraw();
    }

    /// The element with keyboard focus in that window, counting a focus
    /// change asked for but not yet applied.
    pub fn focused(&self) -> Option<ElementId> {
        match self.frame.focus_request {
            Some(FocusRequest::Focus(id)) => Some(id),
            Some(FocusRequest::Blur) => None,
            None => self.frame.focused,
        }
    }

    pub(crate) fn take_focus_request(&mut self) -> Option<FocusRequest> {
        self.frame.focus_request.take()
    }

    /// The focus the window's tree had after its events were dispatched.
    pub(crate) fn record_focus(&mut self, focused: Option<ElementId>) {
        self.frame.focused = focused;
    }
}
