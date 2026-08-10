use crate::platform::window::{PlatformImeEvent, PlatformInputEvent, PlatformWindowEvent};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSEvent, NSTextInputClient, NSView};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSAttributedString, NSAttributedStringKey, NSNotFound,
    NSObjectProtocol, NSPoint, NSRange, NSRangePointer, NSRect, NSString, NSUInteger,
};
use std::cell::RefCell;
use std::collections::VecDeque;

fn not_found_range() -> NSRange {
    NSRange::new(NSNotFound as NSUInteger, 0)
}

#[derive(Debug)]
struct MacImeSession {
    events: VecDeque<PlatformImeEvent>,
    marked_text: Option<String>,
    selected_range: NSRange,
}

impl Default for MacImeSession {
    fn default() -> Self {
        Self {
            events: VecDeque::new(),
            marked_text: None,
            selected_range: not_found_range(),
        }
    }
}

impl MacImeSession {
    fn insert_text(&mut self, text: &str) {
        if self.marked_text.take().is_some() {
            self.events
                .push_back(PlatformImeEvent::Commit(text.to_string()));
        } else {
            self.events
                .push_back(PlatformImeEvent::InsertText(text.to_string()));
        }
        self.selected_range = not_found_range();
    }

    fn set_marked_text(&mut self, text: &str, selected_range: NSRange) {
        let event = if self.marked_text.is_some() {
            PlatformImeEvent::UpdateComposition(text.to_string())
        } else {
            PlatformImeEvent::BeginComposition(text.to_string())
        };
        self.marked_text = Some(text.to_string());
        self.selected_range = selected_range;
        self.events.push_back(event);
    }

    fn cancel_composition(&mut self) {
        if self.marked_text.take().is_some() {
            self.events.push_back(PlatformImeEvent::CancelComposition);
        }
        self.selected_range = not_found_range();
    }

    fn commit_marked_text(&mut self) {
        if let Some(text) = self.marked_text.take() {
            self.events.push_back(PlatformImeEvent::Commit(text));
        }
        self.selected_range = not_found_range();
    }

    fn has_marked_text(&self) -> bool {
        self.marked_text.is_some()
    }

    fn marked_range(&self) -> NSRange {
        self.marked_text
            .as_deref()
            .map_or_else(not_found_range, |text| {
                NSRange::new(0, text.encode_utf16().count())
            })
    }

    fn selected_range(&self) -> NSRange {
        self.selected_range
    }

    fn drain_events(&mut self) -> Vec<PlatformImeEvent> {
        self.events.drain(..).collect()
    }
}

pub(crate) struct RuiContentViewIvars {
    ime: RefCell<MacImeSession>,
}

define_class!(
    // SAFETY: NSView supports subclassing, the ivars are main-thread-only,
    // and this class does not implement Drop.
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[name = "RuiContentView"]
    #[ivars = RuiContentViewIvars]
    pub(crate) struct RuiContentView;

    impl RuiContentView {
        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let events = NSArray::from_slice(&[event]);
            self.interpretKeyEvents(&events);
        }
    }

    unsafe impl NSObjectProtocol for RuiContentView {}

    #[allow(non_snake_case)]
    unsafe impl NSTextInputClient for RuiContentView {
        #[unsafe(method(insertText:replacementRange:))]
        unsafe fn insertText_replacementRange(
            &self,
            string: &AnyObject,
            _replacement_range: NSRange,
        ) {
            match text_from_native_object(string) {
                Some(text) => self.ivars().ime.borrow_mut().insert_text(&text),
                None => log::error!("macOS text input supplied unsupported insertText payload"),
            }
        }

        #[unsafe(method(doCommandBySelector:))]
        unsafe fn doCommandBySelector(&self, selector: Sel) {
            if selector == sel!(cancelOperation:) {
                self.ivars().ime.borrow_mut().cancel_composition();
            }
            // Non-text commands are already represented by the raw KeyDown
            // event collected before AppKit dispatches this callback.
        }

        #[unsafe(method(setMarkedText:selectedRange:replacementRange:))]
        unsafe fn setMarkedText_selectedRange_replacementRange(
            &self,
            string: &AnyObject,
            selected_range: NSRange,
            _replacement_range: NSRange,
        ) {
            match text_from_native_object(string) {
                Some(text) => self
                    .ivars()
                    .ime
                    .borrow_mut()
                    .set_marked_text(&text, selected_range),
                None => {
                    log::error!("macOS text input supplied unsupported setMarkedText payload")
                }
            }
        }

        #[unsafe(method(unmarkText))]
        fn unmarkText(&self) {
            self.ivars().ime.borrow_mut().commit_marked_text();
        }

        #[unsafe(method(selectedRange))]
        fn selectedRange(&self) -> NSRange {
            self.ivars().ime.borrow().selected_range()
        }

        #[unsafe(method(markedRange))]
        fn markedRange(&self) -> NSRange {
            self.ivars().ime.borrow().marked_range()
        }

        #[unsafe(method(hasMarkedText))]
        fn hasMarkedText(&self) -> bool {
            self.ivars().ime.borrow().has_marked_text()
        }

        #[unsafe(method_id(attributedSubstringForProposedRange:actualRange:))]
        unsafe fn attributedSubstringForProposedRange_actualRange(
            &self,
            _range: NSRange,
            _actual_range: NSRangePointer,
        ) -> Option<Retained<NSAttributedString>> {
            None
        }

        #[unsafe(method_id(validAttributesForMarkedText))]
        fn validAttributesForMarkedText(&self) -> Retained<NSArray<NSAttributedStringKey>> {
            NSArray::new()
        }

        #[unsafe(method(firstRectForCharacterRange:actualRange:))]
        unsafe fn firstRectForCharacterRange_actualRange(
            &self,
            range: NSRange,
            actual_range: NSRangePointer,
        ) -> NSRect {
            if let Some(actual_range) = unsafe { actual_range.as_mut() } {
                *actual_range = range;
            }
            let bounds = self.bounds();
            self.window()
                .map_or(bounds, |window| window.convertRectToScreen(bounds))
        }

        #[unsafe(method(characterIndexForPoint:))]
        fn characterIndexForPoint(&self, _point: NSPoint) -> NSUInteger {
            NSNotFound as NSUInteger
        }
    }
);

impl RuiContentView {
    pub(crate) fn new(frame: NSRect, mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(RuiContentViewIvars {
            ime: RefCell::new(MacImeSession::default()),
        });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }

    pub(crate) fn drain_ime_events(&self) -> Vec<PlatformImeEvent> {
        self.ivars().ime.borrow_mut().drain_events()
    }
}

pub(crate) fn append_ime_events_after_native_dispatch(
    events: &mut Vec<PlatformWindowEvent>,
    ime_events: Vec<PlatformImeEvent>,
) {
    if ime_events.is_empty() {
        return;
    }

    let cancels_composition = ime_events
        .iter()
        .any(|event| matches!(event, PlatformImeEvent::CancelComposition));
    if cancels_composition
        && let Some(index) = events.iter().rposition(|event| {
            matches!(
                event,
                PlatformWindowEvent::Input(PlatformInputEvent::KeyDown(_))
            )
        })
    {
        events.remove(index);
    } else if ime_events.iter().any(|event| {
        matches!(
            event,
            PlatformImeEvent::InsertText(_)
                | PlatformImeEvent::BeginComposition(_)
                | PlatformImeEvent::UpdateComposition(_)
                | PlatformImeEvent::Commit(_)
        )
    }) && let Some(PlatformWindowEvent::Input(PlatformInputEvent::KeyDown(key))) =
        events.iter_mut().rev().find(|event| {
            matches!(
                event,
                PlatformWindowEvent::Input(PlatformInputEvent::KeyDown(_))
            )
        })
    {
        key.char = None;
    }

    events.extend(
        ime_events
            .into_iter()
            .map(|event| PlatformWindowEvent::Input(PlatformInputEvent::Ime(event))),
    );
}

fn text_from_native_object(object: &AnyObject) -> Option<String> {
    if let Some(text) = object.downcast_ref::<NSString>() {
        return Some(text.to_string());
    }
    object
        .downcast_ref::<NSAttributedString>()
        .map(|text| text.string().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::window::PlatformImeEvent;
    use objc2_foundation::{NSNotFound, NSRange};

    #[test]
    fn ime_session_emits_begin_update_commit_and_cancel() {
        let mut session = MacImeSession::default();

        session.set_marked_text("你", NSRange::new(1, 0));
        session.set_marked_text("你好", NSRange::new(2, 0));
        session.insert_text("您好");
        session.set_marked_text("draft", NSRange::new(5, 0));
        session.cancel_composition();

        assert_eq!(
            session.drain_events(),
            vec![
                PlatformImeEvent::BeginComposition("你".to_string()),
                PlatformImeEvent::UpdateComposition("你好".to_string()),
                PlatformImeEvent::Commit("您好".to_string()),
                PlatformImeEvent::BeginComposition("draft".to_string()),
                PlatformImeEvent::CancelComposition,
            ]
        );
        assert!(!session.has_marked_text());
    }

    #[test]
    fn ime_session_uses_appkit_not_found_sentinel() {
        let session = MacImeSession::default();

        assert_eq!(session.selected_range().location, NSNotFound as NSUInteger);
    }

    #[test]
    fn ime_session_uses_utf16_lengths_for_appkit_ranges() {
        let mut session = MacImeSession::default();

        session.set_marked_text("a😀", NSRange::new(3, 0));

        assert_eq!(session.marked_range(), NSRange::new(0, 3));
        assert_eq!(session.selected_range(), NSRange::new(3, 0));
    }

    #[test]
    fn ime_session_plain_insert_does_not_fake_a_composition_commit() {
        let mut session = MacImeSession::default();

        session.insert_text("a");

        assert_eq!(
            session.drain_events(),
            vec![PlatformImeEvent::InsertText("a".to_string())]
        );
    }

    #[test]
    fn ime_session_unmark_commits_current_marked_text() {
        let mut session = MacImeSession::default();

        session.set_marked_text("pending", NSRange::new(7, 0));
        session.commit_marked_text();

        assert_eq!(
            session.drain_events(),
            vec![
                PlatformImeEvent::BeginComposition("pending".to_string()),
                PlatformImeEvent::Commit("pending".to_string()),
            ]
        );
        assert!(!session.has_marked_text());
        assert_eq!(session.selected_range(), not_found_range());
    }

    #[test]
    fn ime_session_keeps_empty_marked_text_until_commit_or_cancel() {
        let mut committed = MacImeSession::default();
        committed.set_marked_text("draft", NSRange::new(5, 0));
        committed.set_marked_text("", NSRange::new(0, 0));

        assert!(committed.has_marked_text());
        assert_eq!(committed.marked_range(), NSRange::new(0, 0));
        committed.commit_marked_text();
        assert_eq!(
            committed.drain_events(),
            vec![
                PlatformImeEvent::BeginComposition("draft".to_string()),
                PlatformImeEvent::UpdateComposition(String::new()),
                PlatformImeEvent::Commit(String::new()),
            ]
        );

        let mut cancelled = MacImeSession::default();
        cancelled.set_marked_text("draft", NSRange::new(5, 0));
        cancelled.set_marked_text("", NSRange::new(0, 0));
        cancelled.cancel_composition();
        assert_eq!(
            cancelled.drain_events(),
            vec![
                PlatformImeEvent::BeginComposition("draft".to_string()),
                PlatformImeEvent::UpdateComposition(String::new()),
                PlatformImeEvent::CancelComposition,
            ]
        );
    }
}
