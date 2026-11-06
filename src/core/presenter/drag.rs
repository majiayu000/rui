//! The presenter's side of dragging: the drag follows the pointer, ends after
//! the release reaches the tree, and its preview is painted over the window.

use super::{PointerDispatch, Presenter};
use crate::core::drag::{ActiveDrag, ExternalPaths, FileDragEvent};
use crate::core::event::MouseButton;
use crate::core::geometry::{Bounds, Point};
use crate::elements::Element;
use crate::elements::element::{LayoutContext, PaintContext, PointerEvent, PointerEventKind};
use crate::renderer::Scene;
use taffy::prelude::{AvailableSpace, TaffyTree};

impl<E> Presenter<E> {
    /// Before `event` reaches the tree. Returns whether the preview moved.
    pub(super) fn drag_follow_pointer(&self, event: &PointerEvent) -> bool {
        if !matches!(event.kind, PointerEventKind::Move) {
            return false;
        }
        self.drag
            .borrow_mut()
            .as_mut()
            .is_some_and(|drag| drag.move_to(event.position))
    }

    /// After `event` reached the tree. A release ends the drag; returns whether
    /// a preview was showing, so it has to go.
    pub(super) fn drag_end_on_release(&self, event: &PointerEvent) -> bool {
        if !matches!(event.kind, PointerEventKind::Up) {
            return false;
        }
        self.drag
            .borrow_mut()
            .take()
            .is_some_and(|drag| drag.shows_preview())
    }

    /// Whether a drag is past its threshold, for tests and platform layers.
    pub fn is_dragging(&self) -> bool {
        self.drag
            .borrow()
            .as_ref()
            .is_some_and(ActiveDrag::is_dragging)
    }

    /// Paint the preview of the drag in progress over everything else. It is
    /// painted on its own so its hit regions do not shadow the drop targets
    /// under the pointer.
    pub(super) fn paint_drag_preview(&mut self) {
        let mut drag = self.drag.borrow_mut();
        let Some((preview, origin)) = drag.as_mut().and_then(ActiveDrag::preview_mut) else {
            return;
        };
        let mut taffy = TaffyTree::new();
        let mut layout_cx = LayoutContext::with_text_measurer(
            &mut taffy,
            self.laid_out_viewport,
            &mut self.text_measurer,
        );
        let node = preview.layout(&mut layout_cx);
        if let Err(err) = layout_cx.compute_layout(
            node,
            taffy::Size {
                width: AvailableSpace::MaxContent,
                height: AvailableSpace::MaxContent,
            },
        ) {
            log::error!("drag preview layout failed: {err}");
            return;
        }
        let size = match taffy.layout(node) {
            Ok(layout) => layout.size,
            Err(err) => {
                log::error!("drag preview has no layout: {err}");
                return;
            }
        };
        preview.refresh_text_geometry(&mut self.text_measurer);
        let bounds = Bounds::from_xywh(origin.x, origin.y, size.width, size.height);
        let mut preview_scene = Scene::new();
        let mut paint_cx = PaintContext::without_replay(&mut preview_scene, bounds, &taffy);
        preview.paint(&mut paint_cx);
        for primitive in preview_scene.primitives() {
            self.scene.insert(primitive.clone());
        }
    }
}

impl<E> Presenter<E>
where
    E: Element,
{
    /// Drive a file drag from another application through the tree: entering
    /// starts a drag of [`ExternalPaths`], moves and the drop arrive as pointer
    /// moves and a release, and leaving cancels it.
    pub fn dispatch_file_drag(&mut self, event: &FileDragEvent) -> PointerDispatch {
        match event {
            FileDragEvent::Entered { position, paths } => {
                *self.drag.borrow_mut() = Some(ActiveDrag::dragging(
                    Box::new(ExternalPaths::new(paths.clone())),
                    *position,
                ));
                self.dispatch_pointer_event(&pointer(PointerEventKind::Move, *position))
            }
            FileDragEvent::Moved { position } => {
                self.dispatch_pointer_event(&pointer(PointerEventKind::Move, *position))
            }
            FileDragEvent::Dropped { position } => {
                self.dispatch_pointer_event(&pointer(PointerEventKind::Up, *position))
            }
            FileDragEvent::Exited => self.cancel_drag(),
        }
    }

    /// End the drag in progress without a drop. A move at the last position
    /// tells drop targets the drag is no longer over them.
    pub fn cancel_drag(&mut self) -> PointerDispatch {
        let position = match self.drag.borrow_mut().as_mut() {
            Some(drag) => {
                drag.cancel();
                drag.position()
            }
            None => {
                return PointerDispatch {
                    stopped: false,
                    redraw_requested: false,
                };
            }
        };
        let mut dispatch = self.dispatch_pointer_event(&pointer(PointerEventKind::Move, position));
        let ended = self.drag.borrow_mut().take();
        dispatch.redraw_requested |= ended.is_some_and(|drag| drag.shows_preview());
        dispatch
    }
}

fn pointer(kind: PointerEventKind, position: Point) -> PointerEvent {
    PointerEvent {
        kind,
        position,
        button: matches!(kind, PointerEventKind::Up).then_some(MouseButton::Left),
    }
}
