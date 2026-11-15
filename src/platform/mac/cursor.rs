//! The cursor elements under the pointer ask for, shown through `NSCursor`.

use crate::core::event::Cursor;
use objc2_app_kit::NSCursor;

/// Show `cursor`; called after every pointer move, so it also restores the
/// arrow over elements that ask for nothing.
// The resize cursors macOS 15 deprecates still draw on every version.
#[allow(deprecated)]
pub(super) fn show_cursor(cursor: Cursor) {
    let cursor = match cursor {
        Cursor::Pointer => NSCursor::pointingHandCursor(),
        Cursor::Text => NSCursor::IBeamCursor(),
        Cursor::Crosshair => NSCursor::crosshairCursor(),
        Cursor::NotAllowed => NSCursor::operationNotAllowedCursor(),
        Cursor::Grab | Cursor::Move => NSCursor::openHandCursor(),
        Cursor::Grabbing => NSCursor::closedHandCursor(),
        Cursor::ResizeNS => NSCursor::resizeUpDownCursor(),
        Cursor::ResizeEW => NSCursor::resizeLeftRightCursor(),
        Cursor::Default
        | Cursor::ResizeNESW
        | Cursor::ResizeNWSE
        | Cursor::Wait
        | Cursor::Progress => NSCursor::arrowCursor(),
    };
    cursor.set();
}
