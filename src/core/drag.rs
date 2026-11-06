//! Dragging values between elements, and files dropped on a window.
//!
//! A press on an element with `on_drag` makes the drag's payload and preview;
//! once the pointer has moved [`DRAG_THRESHOLD`] it is dragging, the preview
//! follows the pointer and elements with `on_drop` for the payload's type
//! receive it on release. Files dragged in from another app become a drag of
//! [`ExternalPaths`] driven by [`FileDragEvent`]s.

use crate::core::ElementId;
use crate::core::geometry::Point;
use crate::elements::AnyElement;
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

/// How far the pointer moves after a press before it is a drag, not a click.
pub const DRAG_THRESHOLD: f32 = 3.0;

/// Paths of files dragged onto a window from another application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalPaths(Vec<PathBuf>);

impl ExternalPaths {
    pub fn new(paths: Vec<PathBuf>) -> Self {
        Self(paths)
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.0
    }
}

/// A file drag from another application, in window coordinates.
#[derive(Debug, Clone, PartialEq)]
pub enum FileDragEvent {
    /// Files entered the window.
    Entered {
        position: Point,
        paths: Vec<PathBuf>,
    },
    /// The files moved over the window.
    Moved { position: Point },
    /// The files were released over the window.
    Dropped { position: Point },
    /// The files left the window, or their drag was cancelled.
    Exited,
}

/// The drag in progress in a window.
pub(crate) struct ActiveDrag {
    payload: Box<dyn Any>,
    preview: Option<AnyElement>,
    /// Where the press was, inside the element that started the drag.
    grab_offset: Point,
    start: Point,
    position: Point,
    /// The pointer moved past [`DRAG_THRESHOLD`]; until then a release is a click.
    dragging: bool,
    /// Drop targets told the drag is over them, by stable identity, so a
    /// target rebuilt during the drag still hears that it left.
    over: Vec<ElementId>,
    /// Ending without a drop: drop targets are told the drag left them.
    cancelled: bool,
}

impl ActiveDrag {
    pub(crate) fn new(
        payload: Box<dyn Any>,
        preview: Option<AnyElement>,
        start: Point,
        grab_offset: Point,
    ) -> Self {
        Self {
            payload,
            preview,
            grab_offset,
            start,
            position: start,
            dragging: false,
            over: Vec::new(),
            cancelled: false,
        }
    }

    /// A drag that is already dragging, such as files entering a window.
    pub(crate) fn dragging(payload: Box<dyn Any>, position: Point) -> Self {
        let mut drag = Self::new(payload, None, position, Point::new(0.0, 0.0));
        drag.dragging = true;
        drag
    }

    pub(crate) fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Record whether the drag is over the drop target `id`; returns the new
    /// state when it changed. A cancelled drag is over nothing.
    pub(crate) fn set_over(&mut self, id: ElementId, over: bool) -> Option<bool> {
        let over = over && !self.cancelled;
        let was = self.over.contains(&id);
        if over && !was {
            self.over.push(id);
        } else if !over && was {
            self.over.retain(|target| *target != id);
        }
        (over != was).then_some(over)
    }

    pub(crate) fn cancel(&mut self) {
        self.cancelled = true;
    }

    /// The payload while dragging.
    pub(crate) fn dragged(&self) -> Option<&dyn Any> {
        self.dragging.then_some(&*self.payload)
    }

    pub(crate) fn dragged_type(&self) -> Option<TypeId> {
        self.dragged().map(|payload| (*payload).type_id())
    }

    pub(crate) fn position(&self) -> Point {
        self.position
    }

    /// Follow the pointer; returns whether the preview has to be drawn again.
    pub(crate) fn move_to(&mut self, position: Point) -> bool {
        self.position = position;
        if !self.dragging {
            let dx = position.x - self.start.x;
            let dy = position.y - self.start.y;
            self.dragging = dx * dx + dy * dy >= DRAG_THRESHOLD * DRAG_THRESHOLD;
        }
        self.shows_preview()
    }

    /// Whether a preview is painted over the window.
    pub(crate) fn shows_preview(&self) -> bool {
        self.dragging && self.preview.is_some()
    }

    /// The preview to paint and where its top-left corner goes.
    pub(crate) fn preview_mut(&mut self) -> Option<(&mut AnyElement, Point)> {
        if !self.dragging {
            return None;
        }
        let origin = Point::new(
            self.position.x - self.grab_offset.x,
            self.position.y - self.grab_offset.y,
        );
        self.preview.as_mut().map(|preview| (preview, origin))
    }
}

/// The window's drag, shared by its presenter and the event contexts it builds.
pub(crate) type SharedDrag = Rc<RefCell<Option<ActiveDrag>>>;
