//! Drag sources and drop targets on [`Div`].

use super::Div;
use crate::core::drag::{ActiveDrag, ExternalPaths};
use crate::core::geometry::Point;
use crate::elements::AnyElement;
use crate::elements::element::{EventContext, PointerEvent, PointerEventKind, StableId};
use std::any::{Any, TypeId};
use std::path::PathBuf;

type MakeDrag = Box<dyn Fn() -> (Box<dyn Any>, AnyElement)>;
type DropHandler = Box<dyn Fn(&dyn Any)>;
type OverHandler = Box<dyn Fn(bool)>;

#[derive(Default)]
pub(super) struct DragHandlers {
    /// The div's stable identity, which the drag remembers it is over by.
    pub(super) identity: StableId,
    on_drag: Option<MakeDrag>,
    on_drop: Vec<(TypeId, DropHandler)>,
    on_drag_over: Vec<(TypeId, OverHandler)>,
}

impl Div {
    fn drag_handlers(&mut self) -> &mut DragHandlers {
        self.drag.get_or_insert_with(Box::default)
    }

    /// Make this div a drag source. A press inside it makes the payload and
    /// the preview; once the pointer moves a few points the preview follows
    /// it, and releasing over a div with [`Div::on_drop`] for `T` drops the
    /// payload there. A press and release without moving is still a click.
    pub fn on_drag<T, P>(
        mut self,
        payload: impl Fn() -> T + 'static,
        preview: impl Fn(&T) -> P + 'static,
    ) -> Self
    where
        T: 'static,
        P: Into<AnyElement>,
    {
        self.drag_handlers().on_drag = Some(Box::new(move || {
            let payload = payload();
            let preview = preview(&payload).into();
            (Box::new(payload) as Box<dyn Any>, preview)
        }));
        self
    }

    /// Receive payloads of type `T` released over this div.
    pub fn on_drop<T: 'static>(mut self, handler: impl Fn(&T) + 'static) -> Self {
        self.drag_handlers().on_drop.push((
            TypeId::of::<T>(),
            Box::new(move |payload| {
                if let Some(payload) = payload.downcast_ref::<T>() {
                    handler(payload);
                }
            }),
        ));
        self
    }

    /// Called with `true` when a drag of `T` moves over this div and `false`
    /// when it leaves, is dropped or is cancelled: the drop target's hover state.
    pub fn on_drag_over<T: 'static>(mut self, handler: impl Fn(bool) + 'static) -> Self {
        self.drag_handlers()
            .on_drag_over
            .push((TypeId::of::<T>(), Box::new(handler)));
        self
    }

    /// Receive files dropped on this div from another application.
    /// `on_drag_over::<ExternalPaths>` reports them hovering.
    pub fn on_drop_files(self, handler: impl Fn(&[PathBuf]) + 'static) -> Self {
        self.on_drop::<ExternalPaths>(move |files| handler(files.paths()))
    }
}

impl DragHandlers {
    /// Handle a pointer event for this div after its children. Returns whether
    /// it started a drag or took a drop.
    pub(super) fn handle_pointer_event(
        &mut self,
        cx: &mut EventContext,
        event: &PointerEvent,
        inside: bool,
        handled_by_child: bool,
    ) -> bool {
        match event.kind {
            PointerEventKind::Down => {
                let Some(make) = self
                    .on_drag
                    .as_ref()
                    .filter(|_| inside && !handled_by_child)
                else {
                    return false;
                };
                let Some(drag) = cx.drag() else {
                    log::error!("a drag started from an event no presenter dispatched");
                    return false;
                };
                let (payload, preview) = make();
                let origin = cx.layout_bounds().origin;
                let grab_offset =
                    Point::new(event.position.x - origin.x, event.position.y - origin.y);
                *drag.borrow_mut() = Some(ActiveDrag::new(
                    payload,
                    Some(preview),
                    event.position,
                    grab_offset,
                ));
                true
            }
            PointerEventKind::Move => {
                self.update_over(cx, inside);
                false
            }
            PointerEventKind::Up => {
                let dropped = inside && !handled_by_child && self.drop(cx);
                self.update_over(cx, false);
                dropped
            }
        }
    }

    fn drop(&self, cx: &EventContext) -> bool {
        let Some(drag) = cx.drag() else {
            return false;
        };
        let drag = drag.borrow();
        let Some(payload) = drag.as_ref().and_then(ActiveDrag::dragged) else {
            return false;
        };
        let kind = (*payload).type_id();
        let mut dropped = false;
        for (_, handler) in self.on_drop.iter().filter(|(accepts, _)| *accepts == kind) {
            handler(payload);
            dropped = true;
        }
        dropped
    }

    fn update_over(&self, cx: &EventContext, inside: bool) {
        let changed = {
            let Some(drag) = cx.drag() else {
                return;
            };
            let mut drag = drag.borrow_mut();
            let Some(drag) = drag.as_mut() else {
                return;
            };
            let Some(kind) = drag.dragged_type() else {
                return;
            };
            if !self
                .on_drag_over
                .iter()
                .any(|(accepts, _)| *accepts == kind)
            {
                return;
            }
            drag.set_over(self.identity.get(), inside)
                .map(|over| (kind, over))
        };
        let Some((kind, over)) = changed else {
            return;
        };
        for (_, handler) in self
            .on_drag_over
            .iter()
            .filter(|(accepts, _)| *accepts == kind)
        {
            handler(over);
        }
    }
}
