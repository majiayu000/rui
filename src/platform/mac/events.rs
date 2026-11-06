use super::window::MacWindow;
use crate::core::drag::FileDragEvent;
use crate::core::text_editing::{TextInputCommand, TextInputEvent};
use crate::platform::mac::accessibility::MacAccessibilityActionRequest;
use crate::platform::window::{
    PlatformImeEvent, PlatformInputEvent, PlatformWindowError, PlatformWindowEvent,
};
use objc2::rc::Retained;
use objc2::{ClassType, MainThreadMarker, msg_send};
use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType};
use objc2_foundation::{NSDate, NSPoint};

pub(crate) const MAC_REDRAW_EVENT_DATA: isize = 0x5255_4952;
pub(crate) const MAC_ACCESSIBILITY_EVENT_DATA: isize = 0x5255_4941;
pub(crate) const MAC_DISPLAY_TICK_EVENT_DATA: isize = 0x5255_4954;
const MAC_APPLICATION_EVENT_SUBTYPE: i16 = 0;

pub(crate) enum MacWindowEvent {
    Platform(PlatformWindowEvent),
    Text(TextInputCommand),
    Accessibility(MacAccessibilityActionRequest),
    /// Files dragged over the window from another app.
    FileDrag(FileDragEvent),
    /// The display refreshed while an animation asked for frames.
    DisplayTick,
}

/// Ordered macOS events with lossless native text-input and accessibility payloads.
///
/// Use [`MacWindow::poll_events_with_text_commands`] when replacement ranges,
/// marked-text selections, or accessibility actions must be preserved.
#[derive(Debug, Clone)]
pub enum MacPlatformEvent {
    Platform(PlatformWindowEvent),
    Text(TextInputCommand),
    Accessibility(MacAccessibilityActionRequest),
    FileDrag(FileDragEvent),
}

impl MacWindowEvent {
    pub(crate) fn into_public_event(self) -> MacPlatformEvent {
        match self {
            Self::Platform(event) => MacPlatformEvent::Platform(event),
            Self::Text(command) => MacPlatformEvent::Text(command),
            Self::Accessibility(request) => MacPlatformEvent::Accessibility(request),
            Self::FileDrag(event) => MacPlatformEvent::FileDrag(event),
            Self::DisplayTick => MacPlatformEvent::Platform(PlatformWindowEvent::RedrawRequested),
        }
    }

    pub(crate) fn try_into_platform_event(
        self,
    ) -> Result<PlatformWindowEvent, PlatformWindowError> {
        match self {
            Self::Platform(event) => Ok(event),
            Self::DisplayTick => Ok(PlatformWindowEvent::RedrawRequested),
            Self::Text(command) if command_has_lossless_legacy_representation(&command) => {
                let Some(event) = command.into_legacy_event() else {
                    return Err(PlatformWindowError::backend(
                        "macos",
                        "legacy text input command did not produce a platform IME event",
                    ));
                };
                let ime = match event {
                    TextInputEvent::InsertText(text) => PlatformImeEvent::InsertText(text),
                    TextInputEvent::BeginComposition(text) => {
                        PlatformImeEvent::BeginComposition(text)
                    }
                    TextInputEvent::UpdateComposition(text) => {
                        PlatformImeEvent::UpdateComposition(text)
                    }
                    TextInputEvent::CommitComposition(text) => PlatformImeEvent::Commit(text),
                    TextInputEvent::CancelComposition => PlatformImeEvent::CancelComposition,
                };
                Ok(PlatformWindowEvent::Input(PlatformInputEvent::Ime(ime)))
            }
            Self::Text(_) => Err(PlatformWindowError::backend(
                "macos",
                "native text input command cannot be represented by PlatformImeEvent; use MacWindow::poll_events_with_text_commands",
            )),
            Self::Accessibility(_) => Err(PlatformWindowError::backend(
                "macos",
                "native accessibility action cannot be represented by PlatformWindowEvent; use MacWindow::poll_events_with_text_commands",
            )),
            Self::FileDrag(_) => Err(PlatformWindowError::backend(
                "macos",
                "file drag cannot be represented by PlatformWindowEvent; use MacWindow::poll_events_with_text_commands",
            )),
        }
    }
}

fn command_has_lossless_legacy_representation(command: &TextInputCommand) -> bool {
    matches!(
        command,
        TextInputCommand::InsertText(_)
            | TextInputCommand::BeginComposition(_)
            | TextInputCommand::UpdateComposition(_)
            | TextInputCommand::CommitComposition(_)
            | TextInputCommand::CancelComposition
    )
}

impl MacWindow {
    /// Polls one native event without discarding extended text-input data.
    pub fn poll_events_with_text_commands(
        &mut self,
    ) -> Result<Vec<MacPlatformEvent>, PlatformWindowError> {
        let mtm = MainThreadMarker::new().ok_or_else(|| {
            PlatformWindowError::backend("macos", "event polling must run on the main thread")
        })?;
        let app = NSApplication::sharedApplication(mtm);
        self.poll_events_for_app(&app, false).map(|events| {
            events
                .into_iter()
                .map(MacWindowEvent::into_public_event)
                .collect()
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MacApplicationEvent {
    Redraw,
    Accessibility,
    DisplayTick,
}

pub(crate) fn application_event(data: isize) -> Option<MacApplicationEvent> {
    match data {
        MAC_REDRAW_EVENT_DATA => Some(MacApplicationEvent::Redraw),
        MAC_ACCESSIBILITY_EVENT_DATA => Some(MacApplicationEvent::Accessibility),
        MAC_DISPLAY_TICK_EVENT_DATA => Some(MacApplicationEvent::DisplayTick),
        _ => None,
    }
}

pub(crate) fn append_platform_events(
    events: &mut Vec<MacWindowEvent>,
    platform_events: Vec<PlatformWindowEvent>,
) {
    events.extend(platform_events.into_iter().map(MacWindowEvent::Platform));
}

pub(crate) fn post_application_event(
    window_number: isize,
    data: isize,
) -> Result<(), PlatformWindowError> {
    let mtm = MainThreadMarker::new().ok_or_else(|| {
        PlatformWindowError::backend("macos", "application event posted off the main thread")
    })?;
    post_application_event_to(&NSApplication::sharedApplication(mtm), window_number, data)
}

/// [`post_application_event`] from any thread, for waking the event loop.
///
/// AppKit allows `postEvent:atStart:` off the main thread. The shared application
/// already exists by then: the runner creates it before installing the waker.
pub(crate) fn post_application_event_from_any_thread(
    window_number: isize,
    data: isize,
) -> Result<(), PlatformWindowError> {
    objc2::rc::autoreleasepool(|_| {
        // SAFETY: `sharedApplication` returns the existing shared instance; it is
        // only messaged here with `postEvent:atStart:`, which is thread safe.
        let app: Option<Retained<NSApplication>> =
            unsafe { msg_send![NSApplication::class(), sharedApplication] };
        let app = app.ok_or_else(|| {
            PlatformWindowError::backend("macos", "shared application is unavailable")
        })?;
        post_application_event_to(&app, window_number, data)
    })
}

fn post_application_event_to(
    app: &NSApplication,
    window_number: isize,
    data: isize,
) -> Result<(), PlatformWindowError> {
    let event =
        NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
            NSEventType::ApplicationDefined,
            NSPoint::new(0.0, 0.0),
            NSEventModifierFlags::empty(),
            NSDate::timeIntervalSinceReferenceDate_class(),
            window_number,
            None,
            MAC_APPLICATION_EVENT_SUBTYPE,
            data,
            0,
        )
        .ok_or_else(|| PlatformWindowError::backend("macos", "failed to create application event"))?;
    app.postEvent_atStart(&event, false);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_event_data_is_classified_without_aliasing() {
        assert_eq!(
            application_event(MAC_REDRAW_EVENT_DATA),
            Some(MacApplicationEvent::Redraw)
        );
        assert_eq!(
            application_event(MAC_ACCESSIBILITY_EVENT_DATA),
            Some(MacApplicationEvent::Accessibility)
        );
        assert_eq!(
            application_event(MAC_DISPLAY_TICK_EVENT_DATA),
            Some(MacApplicationEvent::DisplayTick)
        );
        assert_eq!(application_event(0), None);
    }
}
