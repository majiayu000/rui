//! Keyboard focus from app code: focus or blur an element from a listener or
//! task, ask what is focused, and move focus with Tab.

use crate::core::ElementId;
use crate::core::app::AppContext;

/// The keyboard focus of one element. Give the element `.id(handle.id())`,
/// keep the handle in the view, and focus it or ask about it from listeners.
///
/// ```ignore
/// struct Search { field: FocusHandle }
///
/// input().id(self.field.id()).placeholder("Search");
/// button("Find").on_click(cx.listener(|this, cx| this.field.focus(cx.app_mut())));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FocusHandle {
    id: ElementId,
}

impl FocusHandle {
    pub fn new() -> Self {
        Self {
            id: ElementId::new(),
        }
    }

    pub fn id(&self) -> ElementId {
        self.id
    }

    /// Focus the element in the window whose frame runs now.
    pub fn focus(&self, cx: &mut AppContext) {
        cx.focus(self.id);
    }

    pub fn is_focused(&self, cx: &AppContext) -> bool {
        cx.focused() == Some(self.id)
    }
}

impl Default for FocusHandle {
    fn default() -> Self {
        Self::new()
    }
}
