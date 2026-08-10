use crate::core::text_editing::{TextEditError, Utf16TextRange};
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

fn appkit_replacement_range(range: NSRange) -> Result<Option<Utf16TextRange>, TextEditError> {
    if range.location == NSNotFound as NSUInteger {
        return Ok(None);
    }
    Utf16TextRange::new(range.location, range.length).map(Some)
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
    fn insert_text(&mut self, text: &str, replacement_range: NSRange) -> Result<(), TextEditError> {
        let replacement_range = appkit_replacement_range(replacement_range)?;
        let event = if self.marked_text.take().is_some() {
            replacement_range.map_or_else(
                || PlatformImeEvent::Commit(text.to_string()),
                |replacement_range| PlatformImeEvent::CommitReplacing {
                    text: text.to_string(),
                    replacement_range,
                },
            )
        } else {
            replacement_range.map_or_else(
                || PlatformImeEvent::InsertText(text.to_string()),
                |replacement_range| PlatformImeEvent::InsertTextReplacing {
                    text: text.to_string(),
                    replacement_range,
                },
            )
        };
        self.events.push_back(event);
        self.selected_range = not_found_range();
        Ok(())
    }

    fn set_marked_text(
        &mut self,
        text: &str,
        selected_range: NSRange,
        replacement_range: NSRange,
    ) -> Result<(), TextEditError> {
        let replacement_range = appkit_replacement_range(replacement_range)?;
        let event = if self.marked_text.is_some() {
            replacement_range.map_or_else(
                || PlatformImeEvent::UpdateComposition(text.to_string()),
                |replacement_range| PlatformImeEvent::UpdateCompositionReplacing {
                    text: text.to_string(),
                    replacement_range,
                },
            )
        } else {
            replacement_range.map_or_else(
                || PlatformImeEvent::BeginComposition(text.to_string()),
                |replacement_range| PlatformImeEvent::BeginCompositionReplacing {
                    text: text.to_string(),
                    replacement_range,
                },
            )
        };
        self.marked_text = Some(text.to_string());
        self.selected_range = selected_range;
        self.events.push_back(event);
        Ok(())
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

    fn discard_marked_text(&mut self) -> bool {
        let had_marked_text = self.marked_text.take().is_some();
        self.selected_range = not_found_range();
        had_marked_text
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
            replacement_range: NSRange,
        ) {
            match text_from_native_object(string) {
                Some(text) => {
                    if let Err(err) = self
                        .ivars()
                        .ime
                        .borrow_mut()
                        .insert_text(&text, replacement_range)
                    {
                        log::error!("macOS insertText supplied an invalid replacement range: {err}");
                    }
                }
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
            replacement_range: NSRange,
        ) {
            match text_from_native_object(string) {
                Some(text) => {
                    if let Err(err) = self.ivars().ime.borrow_mut().set_marked_text(
                        &text,
                        selected_range,
                        replacement_range,
                    ) {
                        log::error!(
                            "macOS setMarkedText supplied an invalid replacement range: {err}"
                        );
                    }
                }
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

    pub(crate) fn discard_marked_text(&self) {
        let had_marked_text = self.ivars().ime.borrow_mut().discard_marked_text();
        if had_marked_text && let Some(input_context) = self.inputContext() {
            input_context.discardMarkedText();
        }
    }
}

pub(crate) fn append_ime_events_after_native_dispatch(
    events: &mut Vec<PlatformWindowEvent>,
    ime_events: Vec<PlatformImeEvent>,
) -> bool {
    if ime_events.is_empty() {
        return false;
    }

    let consumed_key_down = if let Some(index) = events.iter().rposition(|event| {
        matches!(
            event,
            PlatformWindowEvent::Input(PlatformInputEvent::KeyDown(_))
        )
    }) {
        events.remove(index);
        true
    } else {
        false
    };

    events.extend(
        ime_events
            .into_iter()
            .map(|event| PlatformWindowEvent::Input(PlatformInputEvent::Ime(event))),
    );
    consumed_key_down
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

        session
            .set_marked_text("你", NSRange::new(1, 0), not_found_range())
            .expect("composition should begin");
        session
            .set_marked_text("你好", NSRange::new(2, 0), not_found_range())
            .expect("composition should update");
        session
            .insert_text("您好", not_found_range())
            .expect("composition should commit");
        session
            .set_marked_text("draft", NSRange::new(5, 0), not_found_range())
            .expect("composition should begin");
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

        session
            .set_marked_text("a😀", NSRange::new(3, 0), not_found_range())
            .expect("composition should begin");

        assert_eq!(session.marked_range(), NSRange::new(0, 3));
        assert_eq!(session.selected_range(), NSRange::new(3, 0));
    }

    #[test]
    fn ime_session_plain_insert_does_not_fake_a_composition_commit() {
        let mut session = MacImeSession::default();

        session
            .insert_text("a", not_found_range())
            .expect("plain insert should succeed");

        assert_eq!(
            session.drain_events(),
            vec![PlatformImeEvent::InsertText("a".to_string())]
        );
    }

    #[test]
    fn ime_session_preserves_concrete_replacement_ranges_for_all_text_callbacks() {
        let mut session = MacImeSession::default();
        session
            .insert_text("plain", NSRange::new(1, 2))
            .expect("plain replacement should be accepted");
        session
            .set_marked_text("draft", NSRange::new(5, 0), NSRange::new(3, 4))
            .expect("marked replacement should begin");
        session
            .set_marked_text("updated", NSRange::new(7, 0), NSRange::new(3, 5))
            .expect("marked replacement should update");
        session
            .insert_text("committed", NSRange::new(3, 7))
            .expect("marked replacement should commit");

        assert_eq!(
            session.drain_events(),
            vec![
                PlatformImeEvent::InsertTextReplacing {
                    text: "plain".to_string(),
                    replacement_range: Utf16TextRange::new(1, 2)
                        .expect("test range should be valid"),
                },
                PlatformImeEvent::BeginCompositionReplacing {
                    text: "draft".to_string(),
                    replacement_range: Utf16TextRange::new(3, 4)
                        .expect("test range should be valid"),
                },
                PlatformImeEvent::UpdateCompositionReplacing {
                    text: "updated".to_string(),
                    replacement_range: Utf16TextRange::new(3, 5)
                        .expect("test range should be valid"),
                },
                PlatformImeEvent::CommitReplacing {
                    text: "committed".to_string(),
                    replacement_range: Utf16TextRange::new(3, 7)
                        .expect("test range should be valid"),
                },
            ]
        );
    }

    #[test]
    fn ime_session_unmark_commits_current_marked_text() {
        let mut session = MacImeSession::default();

        session
            .set_marked_text("pending", NSRange::new(7, 0), not_found_range())
            .expect("composition should begin");
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
        committed
            .set_marked_text("draft", NSRange::new(5, 0), not_found_range())
            .expect("composition should begin");
        committed
            .set_marked_text("", NSRange::new(0, 0), not_found_range())
            .expect("composition should update");

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
        cancelled
            .set_marked_text("draft", NSRange::new(5, 0), not_found_range())
            .expect("composition should begin");
        cancelled
            .set_marked_text("", NSRange::new(0, 0), not_found_range())
            .expect("composition should update");
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

    #[test]
    fn ime_session_discards_marked_text_without_queuing_an_event() {
        let mut session = MacImeSession::default();
        session
            .set_marked_text("draft", NSRange::new(5, 0), not_found_range())
            .expect("composition should begin");
        session.drain_events();

        assert!(session.discard_marked_text());
        assert!(!session.has_marked_text());
        assert_eq!(session.selected_range(), not_found_range());
        assert!(session.drain_events().is_empty());
    }
}
