//! View system - declarative UI components

use crate::core::entity::Entity;
use crate::core::app::AppContext;

/// Context provided to views during rendering
pub struct ViewContext<'a, T> {
    pub(crate) app: &'a mut AppContext,
    pub(crate) view: Entity<T>,
}

impl<'a, T: 'static> ViewContext<'a, T> {
    pub fn new(app: &'a mut AppContext, view: Entity<T>) -> Self {
        Self { app, view }
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
