//! View system - declarative UI components

use crate::core::app::AppContext;
use crate::core::entity::Entity;
use std::cell::{Cell, RefCell};
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

type ViewUpdate<T> = Box<dyn FnOnce(&mut T, &mut ViewContext<'_, T>)>;

/// Updates queued by listeners, applied to the view before it next renders.
pub(crate) struct ViewUpdates<T>(Rc<RefCell<Vec<ViewUpdate<T>>>>);

impl<T> ViewUpdates<T> {
    pub(crate) fn new() -> Self {
        Self(Rc::new(RefCell::new(Vec::new())))
    }

    fn push(&self, update: ViewUpdate<T>) {
        self.0.borrow_mut().push(update);
    }

    pub(crate) fn take(&self) -> Vec<ViewUpdate<T>> {
        std::mem::take(&mut *self.0.borrow_mut())
    }
}

impl<T> Clone for ViewUpdates<T> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

/// Context provided to views during rendering
pub struct ViewContext<'a, T> {
    pub(crate) app: &'a mut AppContext,
    pub(crate) view: Entity<T>,
    pub(crate) notifier: ViewNotifier,
    updates: ViewUpdates<T>,
}

impl<'a, T: 'static> ViewContext<'a, T> {
    pub fn new(app: &'a mut AppContext, view: Entity<T>) -> Self {
        Self::with_parts(app, view, ViewNotifier::new(), ViewUpdates::new())
    }

    pub(crate) fn with_parts(
        app: &'a mut AppContext,
        view: Entity<T>,
        notifier: ViewNotifier,
        updates: ViewUpdates<T>,
    ) -> Self {
        Self {
            app,
            view,
            notifier,
            updates,
        }
    }

    /// An event handler that updates this view.
    ///
    /// When the event fires, `handler` is queued and the view is scheduled to render;
    /// before that render it runs with the view and a context that reaches every
    /// entity. The view renders again afterwards, so `handler` need not notify it.
    ///
    /// ```ignore
    /// button("+").on_click(cx.listener(|this, _| this.count += 1))
    /// ```
    pub fn listener(
        &self,
        handler: impl Fn(&mut T, &mut ViewContext<'_, T>) + 'static,
    ) -> impl Fn() + 'static {
        let listener = self.listener_with(move |view, (), cx| handler(view, cx));
        move || listener(())
    }

    /// [`Self::listener`] for handlers that receive a value, such as `on_hover(bool)`.
    pub fn listener_with<A: 'static>(
        &self,
        handler: impl Fn(&mut T, A, &mut ViewContext<'_, T>) + 'static,
    ) -> impl Fn(A) + 'static {
        let handler = Rc::new(handler);
        let updates = self.updates.clone();
        let notifier = self.notifier.clone();
        move |value| {
            let handler = Rc::clone(&handler);
            updates.push(Box::new(move |view, cx| handler(view, value, cx)));
            notifier.notify();
        }
    }

    /// [`Self::listener`] for handlers that receive text, such as `on_change(&str)`.
    pub fn text_listener(
        &self,
        handler: impl Fn(&mut T, String, &mut ViewContext<'_, T>) + 'static,
    ) -> impl Fn(&str) + 'static {
        let listener = self.listener_with(handler);
        move |text| listener(text.to_owned())
    }

    /// Get the view's entity handle
    pub fn entity(&self) -> Entity<T> {
        self.view
    }

    /// A `Send` handle that other threads use to update the application.
    pub fn app_handle(&self) -> crate::core::app_handle::AppHandle {
        self.app.app_handle()
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

    /// Render this view again on the next display refresh.
    ///
    /// Call it from every render that animates; the display stops driving
    /// frames once no render asks.
    pub fn request_animation_frame(&mut self) {
        self.app.request_animation_frame_for(self.view.id());
    }
}

/// Trait for types that can be rendered as views
pub trait View: Sized + 'static {
    /// Render this view into an element tree
    type Element: crate::elements::Element;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element;
}
