//! View system - declarative UI components

use crate::core::app::AppContext;
use crate::core::entity::Entity;
use std::cell::Cell;
use std::rc::Rc;

/// Cloneable rebuild signal for callbacks created while rendering a view.
#[derive(Clone, Debug)]
pub struct ViewNotifier {
    notified: Rc<Cell<bool>>,
}

impl ViewNotifier {
    pub(crate) fn new() -> Self {
        Self {
            notified: Rc::new(Cell::new(false)),
        }
    }

    /// Schedule the owning view to render again on the next runtime pass.
    pub fn notify(&self) {
        self.notified.set(true);
    }

    pub(crate) fn take_notified(&self) -> bool {
        let notified = self.notified.get();
        self.notified.set(false);
        notified
    }
}

/// Context provided to views during rendering
pub struct ViewContext<'a, T> {
    pub(crate) app: &'a mut AppContext,
    pub(crate) view: Entity<T>,
    pub(crate) notifier: ViewNotifier,
}

impl<'a, T: 'static> ViewContext<'a, T> {
    pub fn new(app: &'a mut AppContext, view: Entity<T>) -> Self {
        Self::with_notifier(app, view, ViewNotifier::new())
    }

    pub(crate) fn with_notifier(
        app: &'a mut AppContext,
        view: Entity<T>,
        notifier: ViewNotifier,
    ) -> Self {
        Self {
            app,
            view,
            notifier,
        }
    }

    /// Get the view's entity handle
    pub fn entity(&self) -> Entity<T> {
        self.view
    }

    /// Access the application context
    pub fn app(&self) -> &AppContext {
        self.app
    }

    /// Access the application context mutably
    pub fn app_mut(&mut self) -> &mut AppContext {
        self.app
    }

    /// Create a notifier that can be moved into event callbacks.
    pub fn notifier(&self) -> ViewNotifier {
        self.notifier.clone()
    }

    /// Schedule a re-render of this view
    pub fn notify(&mut self) {
        self.app.notify(self.view.id());
    }
}

/// Trait for types that can be rendered as views
pub trait View: Sized + 'static {
    /// Render this view into an element tree
    type Element: crate::elements::Element;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element;
}
