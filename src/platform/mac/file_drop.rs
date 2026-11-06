//! Files dragged onto a window from Finder or another app.
//!
//! The content view is the window's dragging destination. AppKit calls it
//! from inside the event poll; it records [`FileDragEvent`]s and posts a wake
//! so the poll returns, and the window hands them to the tree with its other
//! events.

use crate::core::drag::FileDragEvent;
use crate::core::geometry::Point;
use crate::platform::mac::events::{MAC_REDRAW_EVENT_DATA, post_application_event};
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeFileURL, NSView};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSString, NSUInteger};
use std::path::PathBuf;

/// `NSDragOperationNone`: the window does not take what is dragged.
pub(crate) const DRAG_OPERATION_NONE: NSUInteger = 0;
/// `NSDragOperationCopy`: dropped files are copied, as Finder shows with a plus.
pub(crate) const DRAG_OPERATION_COPY: NSUInteger = 1;

/// The file drag over one window, and what it has to tell the tree.
#[derive(Debug, Default)]
pub(crate) struct FileDropTarget {
    /// The drag carries files, so the window accepted it.
    accepted: bool,
    events: Vec<FileDragEvent>,
}

impl FileDropTarget {
    /// Files entered at `position`; returns whether the window takes them.
    pub(crate) fn enter(&mut self, position: Point, paths: Vec<PathBuf>) -> bool {
        self.accepted = !paths.is_empty();
        if self.accepted {
            self.events.push(FileDragEvent::Entered { position, paths });
        }
        self.accepted
    }

    pub(crate) fn update(&mut self, position: Point) -> bool {
        if self.accepted {
            self.events.push(FileDragEvent::Moved { position });
        }
        self.accepted
    }

    pub(crate) fn exit(&mut self) {
        if std::mem::take(&mut self.accepted) {
            self.events.push(FileDragEvent::Exited);
        }
    }

    pub(crate) fn drop_at(&mut self, position: Point) -> bool {
        let accepted = std::mem::take(&mut self.accepted);
        if accepted {
            self.events.push(FileDragEvent::Dropped { position });
        }
        accepted
    }

    pub(crate) fn drain(&mut self) -> Vec<FileDragEvent> {
        std::mem::take(&mut self.events)
    }
}

/// Make `view` a destination for file drags.
pub(crate) fn register_for_file_drags(view: &NSView) {
    let types = NSArray::from_slice(&[unsafe { NSPasteboardTypeFileURL }]);
    let _: () = unsafe { msg_send![view, registerForDraggedTypes: &*types] };
}

/// Where the drag described by `info` is, in `view`'s top-left coordinates.
pub(crate) fn drag_position(view: &NSView, info: &AnyObject) -> Point {
    let in_window: NSPoint = unsafe { msg_send![info, draggingLocation] };
    let in_view = view.convertPoint_fromView(in_window, None);
    let bounds: NSRect = view.bounds();
    Point::new(in_view.x as f32, (bounds.size.height - in_view.y) as f32)
}

/// The file paths on the drag's pasteboard, resolving file reference URLs.
pub(crate) fn dragged_paths(info: &AnyObject) -> Vec<PathBuf> {
    let pasteboard: Option<Retained<NSPasteboard>> = unsafe { msg_send![info, draggingPasteboard] };
    let Some(pasteboard) = pasteboard else {
        log::error!("macOS file drag has no pasteboard");
        return Vec::new();
    };
    let Some(url_class) = AnyClass::get(c"NSURL") else {
        log::error!("NSURL class is unavailable");
        return Vec::new();
    };
    let items: Option<Retained<NSArray<AnyObject>>> =
        unsafe { msg_send![&*pasteboard, pasteboardItems] };
    let Some(items) = items else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    for item in items.iter() {
        let url_string: Option<Retained<NSString>> =
            unsafe { msg_send![&*item, stringForType: NSPasteboardTypeFileURL] };
        let Some(url_string) = url_string else {
            continue;
        };
        let url: Option<Retained<AnyObject>> =
            unsafe { msg_send![url_class, URLWithString: &*url_string] };
        let path: Option<Retained<NSString>> = match url {
            Some(url) => unsafe { msg_send![&*url, path] },
            None => None,
        };
        match path {
            Some(path) => paths.push(PathBuf::from(path.to_string())),
            None => log::error!("macOS file drag carried an unreadable file URL: {url_string}"),
        }
    }
    paths
}

/// Wake the event poll so the window sees the file drag events of `view`.
pub(crate) fn wake_window_of(view: &NSView) {
    let Some(window) = view.window() else {
        log::error!("macOS file drag reached a view without a window");
        return;
    };
    if let Err(err) = post_application_event(window.windowNumber(), MAC_REDRAW_EVENT_DATA) {
        log::error!("failed to wake the event loop for a file drag: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_without_files_is_refused_and_tells_the_tree_nothing() {
        let mut target = FileDropTarget::default();
        assert!(!target.enter(Point::new(1.0, 2.0), Vec::new()));
        assert!(!target.update(Point::new(3.0, 4.0)));
        target.exit();
        assert!(!target.drop_at(Point::new(3.0, 4.0)));
        assert!(target.drain().is_empty());
    }

    #[test]
    fn an_accepted_drag_reports_enter_moves_and_the_drop_once() {
        let mut target = FileDropTarget::default();
        let paths = vec![PathBuf::from("/tmp/a")];
        assert!(target.enter(Point::new(1.0, 2.0), paths.clone()));
        assert!(target.update(Point::new(3.0, 4.0)));
        assert!(target.drop_at(Point::new(3.0, 4.0)));
        target.exit();
        assert_eq!(
            target.drain(),
            [
                FileDragEvent::Entered {
                    position: Point::new(1.0, 2.0),
                    paths,
                },
                FileDragEvent::Moved {
                    position: Point::new(3.0, 4.0),
                },
                FileDragEvent::Dropped {
                    position: Point::new(3.0, 4.0),
                },
            ]
        );
    }
}
