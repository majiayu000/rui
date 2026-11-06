//! Lists written as one view per entity: [`AppContext::rows`].

use super::AppContext;
use crate::core::entity::{Entity, EntityId};
use crate::core::view::{View, ViewContext};
use crate::elements::{Element, ViewElement, view};
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

type RowFn<T, E> = dyn Fn(&T, &mut ViewContext<'_, Row<T, E>>) -> E;

/// The newest closure passed at one `rows` call site, shared by its rows.
type RowRender<T, E> = Rc<RefCell<Rc<RowFn<T, E>>>>;

/// The row views made by [`AppContext::rows`], keyed by the row closure's type
/// and the entity each one shows.
#[derive(Default)]
pub(crate) struct RowViews {
    views: HashMap<(TypeId, EntityId), EntityId>,
    keys: HashMap<EntityId, (TypeId, EntityId)>,
    /// A [`RowRender`] per closure type.
    renders: HashMap<TypeId, Box<dyn Any>>,
}

/// The view [`AppContext::rows`] makes for one entity: it renders the entity
/// with the row closure, and renders again when the entity is notified.
pub struct Row<T, E> {
    item: Entity<T>,
    render: RowRender<T, E>,
}

impl<T: 'static, E: Element + 'static> View for Row<T, E> {
    type Element = E;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> E {
        let render = Rc::clone(&self.render.borrow());
        let id = self.item.id();
        // The entity's own cell, so the closure gets the item and the context
        // together. Borrowing it again inside the closure panics as any double
        // borrow of an entity does.
        let Some(cell) = cx.app.entities.cell(id) else {
            panic!(
                "row {:?} shows entity {id:?}, which was released",
                cx.entity().id()
            );
        };
        cx.app.record_read(id);
        let item = cell.borrow();
        let Some(item) = item.downcast_ref::<T>() else {
            unreachable!("entity {id:?} holds the type its handle names");
        };
        render(item, cx)
    }
}

impl AppContext {
    /// Place a view per entity of `items`, each showing its entity through
    /// `row`.
    ///
    /// A row's view is made the first time this call site shows that entity:
    /// rows are keyed by the type of `row` and the entity. Notifying an entity
    /// renders its row again and nothing else; the tree that places the rows
    /// is not rebuilt. A row renders with the newest `row` given at its call
    /// site, but a new closure alone does not render any row again. A row is
    /// released once no tree places it.
    pub fn rows<T, E, F>(&mut self, items: &[Entity<T>], row: F) -> Vec<ViewElement>
    where
        T: 'static,
        E: Element + 'static,
        F: Fn(&T, &mut ViewContext<'_, Row<T, E>>) -> E + 'static,
    {
        let key = TypeId::of::<F>();
        let newest: Rc<RowFn<T, E>> = Rc::new(row);
        let render = match self
            .row_views
            .renders
            .get(&key)
            .and_then(|render| render.downcast_ref::<RowRender<T, E>>())
        {
            Some(render) => {
                *render.borrow_mut() = newest;
                Rc::clone(render)
            }
            None => {
                let render = Rc::new(RefCell::new(newest));
                self.row_views
                    .renders
                    .insert(key, Box::new(Rc::clone(&render)));
                render
            }
        };
        items
            .iter()
            .map(|item| {
                let row = match self.row_views.views.get(&(key, item.id())) {
                    Some(row) => Entity::<Row<T, E>>::new(*row),
                    None => {
                        let row = self.new_view(Row {
                            item: *item,
                            render: Rc::clone(&render),
                        });
                        self.row_views.views.insert((key, item.id()), row.id());
                        self.row_views.keys.insert(row.id(), (key, item.id()));
                        row
                    }
                };
                view(row)
            })
            .collect()
    }

    /// Forget the row key of the view `id`, released now.
    pub(super) fn forget_row(&mut self, id: EntityId) {
        if let Some(key) = self.row_views.keys.remove(&id) {
            self.row_views.views.remove(&key);
        }
    }
}

impl<T: 'static> ViewContext<'_, T> {
    /// [`AppContext::rows`] from inside a view's render.
    pub fn rows<I, E, F>(&mut self, items: &[Entity<I>], row: F) -> Vec<ViewElement>
    where
        I: 'static,
        E: Element + 'static,
        F: Fn(&I, &mut ViewContext<'_, Row<I, E>>) -> E + 'static,
    {
        self.app.rows(items, row)
    }
}
