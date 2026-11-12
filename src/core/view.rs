//! View system - declarative UI components

use crate::core::ElementId;
use crate::core::app::AppContext;
use crate::core::entity::{Entity, EntityId};
use crate::core::executor::{AsyncApp, Task};
use crate::core::geometry::Point;
use crate::core::menu::Menu;
use crate::core::prompt::{PathPromptOptions, PromptDismissed};
use crate::core::text_editing::{ClipboardError, ClipboardItem};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

/// Views notified from event handlers, which have no context to notify with;
/// the next frame notifies them.
pub(crate) type NotifiedViews = Rc<RefCell<Vec<EntityId>>>;

/// Shared by every element that shows a view. When the last one is dropped,
/// the view is queued to be released unless a tree places it again first.
pub(crate) struct ViewPlacement {
    view: EntityId,
    unplaced: NotifiedViews,
}

impl ViewPlacement {
    pub(crate) fn new(view: EntityId, unplaced: NotifiedViews) -> Self {
        Self { view, unplaced }
    }
}

impl Drop for ViewPlacement {
    fn drop(&mut self) {
        self.unplaced.borrow_mut().push(self.view);
    }
}

/// Cloneable rebuild signal for callbacks created while rendering a view.
#[derive(Clone, Debug)]
pub struct ViewNotifier {
    notified: Rc<Cell<bool>>,
    /// Where a view inside another view's tree reports itself.
    queue: Option<(EntityId, NotifiedViews)>,
}

impl ViewNotifier {
    pub(crate) fn new() -> Self {
        Self {
            notified: Rc::new(Cell::new(false)),
            queue: None,
        }
    }

    /// A notifier for the view `entity`, which reports to `queue`.
    pub(crate) fn queued(entity: EntityId, queue: NotifiedViews) -> Self {
        Self {
            queue: Some((entity, queue)),
            ..Self::new()
        }
    }

    /// Schedule the owning view to render again on the next runtime pass.
    pub fn notify(&self) {
        self.notified.set(true);
        if let Some((entity, queue)) = &self.queue {
            queue.borrow_mut().push(*entity);
        }
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

/// A view as work that outlives a render holds it, such as a task from
/// [`ViewContext::spawn`]. It does not keep the view alive.
pub struct WeakView<T> {
    entity: Entity<T>,
    updates: Weak<RefCell<Vec<ViewUpdate<T>>>>,
}

/// The view was released before an update reached it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewReleased;

impl std::fmt::Display for ViewReleased {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the view was released")
    }
}

impl std::error::Error for ViewReleased {}

impl<T: 'static> WeakView<T> {
    /// Queue `update` for the view and schedule it to render. It runs before
    /// that render, as a [`ViewContext::listener`] handler does, so it need not
    /// notify. Call it from a task's [`AsyncApp`].
    pub fn update(
        &self,
        cx: &AsyncApp,
        update: impl FnOnce(&mut T, &mut ViewContext<'_, T>) + 'static,
    ) -> Result<(), ViewReleased> {
        let updates = self.updates.upgrade().ok_or(ViewReleased)?;
        updates.borrow_mut().push(Box::new(update));
        cx.update(|app| app.notify(self.entity.id()));
        Ok(())
    }

    pub fn entity(&self) -> Entity<T> {
        self.entity
    }
}

impl<T> Clone for WeakView<T> {
    fn clone(&self) -> Self {
        Self {
            entity: self.entity,
            updates: Weak::clone(&self.updates),
        }
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

    /// A handle on this view for work that outlives this render.
    pub fn weak_view(&self) -> WeakView<T> {
        WeakView {
            entity: self.view,
            updates: Rc::downgrade(&self.updates.0),
        }
    }

    /// Run `task` on the UI thread with a handle on this view, as
    /// [`AppContext::spawn`] does. The task updates the view through the handle
    /// and the view renders again; it need not notify.
    ///
    /// ```ignore
    /// cx.spawn(|this, cx| async move {
    ///     let body = cx.background_spawn(fetch(url)).await;
    ///     this.update(&cx, |this, _| this.body = body).ok();
    /// })
    /// .detach();
    /// ```
    pub fn spawn<F, Fut>(&mut self, task: F) -> Task<Fut::Output>
    where
        F: FnOnce(WeakView<T>, AsyncApp) -> Fut,
        Fut: Future + 'static,
        Fut::Output: 'static,
    {
        let this = self.weak_view();
        self.app.spawn(move |cx| task(this, cx))
    }

    /// A `Send` handle that other threads use to update the application.
    pub fn app_handle(&self) -> crate::core::app_handle::AppHandle {
        self.app.app_handle()
    }

    /// An image of `asset` decoded off the UI thread; see [`AppContext::image`].
    pub fn image(&mut self, asset: impl Into<crate::core::ImageAsset>) -> crate::Image {
        self.app.image(asset)
    }

    /// Access the application context
    pub fn app(&self) -> &AppContext {
        self.app
    }

    /// Access the application context mutably
    pub fn app_mut(&mut self) -> &mut AppContext {
        self.app
    }

    /// See [`AppContext::write_to_clipboard`].
    pub fn write_to_clipboard(&self, item: ClipboardItem) -> Result<(), ClipboardError> {
        self.app.write_to_clipboard(item)
    }

    /// See [`AppContext::read_from_clipboard`].
    pub fn read_from_clipboard(&self) -> Result<Option<ClipboardItem>, ClipboardError> {
        self.app.read_from_clipboard()
    }

    /// See [`AppContext::set_menus`].
    pub fn set_menus(&mut self, menus: Vec<Menu>) {
        self.app.set_menus(menus);
    }

    /// See [`AppContext::show_context_menu`].
    pub fn show_context_menu(&mut self, menu: Menu, position: Point) {
        self.app.show_context_menu(menu, position);
    }

    /// See [`AppContext::prompt_for_paths`].
    pub fn prompt_for_paths(
        &mut self,
        options: PathPromptOptions,
    ) -> Task<Result<Option<Vec<PathBuf>>, PromptDismissed>> {
        self.app.prompt_for_paths(options)
    }

    /// See [`AppContext::prompt_for_new_path`].
    pub fn prompt_for_new_path(
        &mut self,
        directory: &Path,
    ) -> Task<Result<Option<PathBuf>, PromptDismissed>> {
        self.app.prompt_for_new_path(directory)
    }

    /// See [`AppContext::focus`].
    pub fn focus(&mut self, id: ElementId) {
        self.app.focus(id);
    }

    /// See [`AppContext::blur`].
    pub fn blur(&mut self) {
        self.app.blur();
    }

    /// See [`AppContext::focused`].
    pub fn focused(&self) -> Option<ElementId> {
        self.app.focused()
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

    /// See [`AppContext::toggle_profiler`].
    pub fn toggle_profiler(&mut self) {
        self.app.toggle_profiler();
    }

    /// See [`AppContext::toggle_damage_visualizer`].
    pub fn toggle_damage_visualizer(&mut self) {
        self.app.toggle_damage_visualizer();
    }

    /// See [`AppContext::toggle_inspector`].
    pub fn toggle_inspector(&mut self) {
        self.app.toggle_inspector();
    }
}

/// A view rendered inside other views' trees, kept between renders.
pub(crate) trait ChildView {
    /// Apply queued listener updates and render.
    fn render_any(&mut self, app: &mut AppContext) -> crate::elements::AnyElement;
}

impl<T> AsMut<AppContext> for ViewContext<'_, T> {
    fn as_mut(&mut self) -> &mut AppContext {
        self.app
    }
}

impl<V: View> ChildView for crate::core::app::RuntimeView<V> {
    fn render_any(&mut self, app: &mut AppContext) -> crate::elements::AnyElement {
        crate::elements::AnyElement::new(self.render(app))
    }
}

/// Trait for types that can be rendered as views
pub trait View: Sized + 'static {
    /// Render this view into an element tree
    type Element: crate::elements::Element;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updating_a_released_view_fails() {
        let mut app = AppContext::new();
        let view = Entity::<u32>::new(app.create(()).id());
        let updates = ViewUpdates::<u32>::new();
        let weak = WeakView {
            entity: view,
            updates: Rc::downgrade(&updates.0),
        };
        let outcomes = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::clone(&outcomes);
        app.spawn(move |cx| async move {
            seen.borrow_mut()
                .push(weak.update(&cx, |count, _| *count += 1));
            drop(updates);
            seen.borrow_mut()
                .push(weak.update(&cx, |count, _| *count += 1));
        })
        .detach();
        app.run_until_parked();
        assert_eq!(*outcomes.borrow(), [Ok(()), Err(ViewReleased)]);
    }
}
