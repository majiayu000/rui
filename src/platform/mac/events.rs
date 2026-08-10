use crate::core::text_editing::{TextInputCommand, TextInputEvent};
use crate::platform::mac::accessibility::MacAccessibilityActionRequest;
use crate::platform::window::{
    PlatformImeEvent, PlatformInputEvent, PlatformWindowError, PlatformWindowEvent,
};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags, NSEventType};
use objc2_foundation::{NSDate, NSPoint};

pub(crate) const MAC_REDRAW_EVENT_DATA: isize = 0x5255_4952;
pub(crate) const MAC_ACCESSIBILITY_EVENT_DATA: isize = 0x5255_4941;
const MAC_APPLICATION_EVENT_SUBTYPE: i16 = 0;

pub(crate) enum MacWindowEvent {
    Platform(PlatformWindowEvent),
    Text(TextInputCommand),
    Accessibility(MacAccessibilityActionRequest),
}

impl MacWindowEvent {
    pub(crate) fn into_platform_event(self) -> Option<PlatformWindowEvent> {
        match self {
            Self::Platform(event) => Some(event),
            Self::Text(command) => command.into_legacy_event().map(|event| {
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
                PlatformWindowEvent::Input(PlatformInputEvent::Ime(ime))
            }),
            Self::Accessibility(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MacApplicationEvent {
    Redraw,
    Accessibility,
}

pub(crate) fn application_event(data: isize) -> Option<MacApplicationEvent> {
    match data {
        MAC_REDRAW_EVENT_DATA => Some(MacApplicationEvent::Redraw),
        MAC_ACCESSIBILITY_EVENT_DATA => Some(MacApplicationEvent::Accessibility),
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
    let app = NSApplication::sharedApplication(mtm);
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
        assert_eq!(application_event(0), None);
    }
}
