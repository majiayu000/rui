use crate::core::geometry::{Bounds, Point};
use crate::core::text_editing::{
    TextEditError, TextInputCommand, TextInputSnapshot, Utf16TextRange,
};
use crate::platform::window::{PlatformInputEvent, PlatformWindowEvent};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSEvent, NSTextInputClient, NSView};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSAttributedString, NSAttributedStringKey, NSNotFound,
    NSObjectProtocol, NSPoint, NSRange, NSRangePointer, NSRect, NSSize, NSString, NSUInteger,
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
    Utf16TextRange::new(range.location, range.length)
        .map(Some)
        .map_err(Into::into)
}

fn ns_range(range: Option<Utf16TextRange>) -> NSRange {
    range.map_or_else(not_found_range, |range| {
        NSRange::new(range.location(), range.length())
    })
}

fn collapsed_at_end(range: NSRange) -> NSRange {
    if range.location == NSNotFound as NSUInteger {
        not_found_range()
    } else {
        NSRange::new(range.location + range.length, 0)
    }
}

fn appkit_view_caret_rect(caret_bounds: Bounds, view_height: f64) -> NSRect {
    NSRect::new(
        NSPoint::new(
            caret_bounds.x() as f64,
            view_height - caret_bounds.max_y() as f64,
        ),
        NSSize::new(0.0, caret_bounds.height() as f64),
    )
}

fn appkit_view_rect(bounds: Bounds, view_height: f64) -> NSRect {
    NSRect::new(
        NSPoint::new(bounds.x() as f64, view_height - bounds.max_y() as f64),
        NSSize::new(bounds.width() as f64, bounds.height() as f64),
    )
}

#[derive(Debug)]
struct MacImeSession {
    events: VecDeque<TextInputCommand>,
    marked_text: Option<String>,
    selected_range: NSRange,
    marked_range: NSRange,
    caret_range: NSRange,
    caret_bounds: Option<Bounds>,
    snapshot: Option<TextInputSnapshot>,
}

impl Default for MacImeSession {
    fn default() -> Self {
        Self {
            events: VecDeque::new(),
            marked_text: None,
            selected_range: not_found_range(),
            marked_range: not_found_range(),
            caret_range: not_found_range(),
            caret_bounds: None,
            snapshot: None,
        }
    }
}

impl MacImeSession {
    fn insert_text(&mut self, text: &str, replacement_range: NSRange) -> Result<(), TextEditError> {
        let replacement_range = appkit_replacement_range(replacement_range)?;
        let had_marked_text = self.marked_text.take().is_some();
        let insertion_start = replacement_range
            .map(Utf16TextRange::location)
            .or_else(|| {
                had_marked_text
                    .then_some(self.marked_range.location)
                    .filter(|location| *location != NSNotFound as NSUInteger)
            })
            .or_else(|| {
                (self.selected_range.location != NSNotFound as NSUInteger)
                    .then_some(self.selected_range.location)
            });
        let event = if had_marked_text {
            replacement_range.map_or_else(
                || TextInputCommand::CommitComposition(text.to_string()),
                |replacement_range| TextInputCommand::CommitCompositionReplacing {
                    text: text.to_string(),
                    replacement_range,
                },
            )
        } else {
            replacement_range.map_or_else(
                || TextInputCommand::InsertText(text.to_string()),
                |replacement_range| TextInputCommand::InsertTextReplacing {
                    text: text.to_string(),
                    replacement_range,
                },
            )
        };
        let selected_range = match insertion_start {
            Some(start) => {
                let location = Utf16TextRange::new(start, text.encode_utf16().count())?.end();
                NSRange::new(location, 0)
            }
            None => not_found_range(),
        };
        self.events.push_back(event);
        self.marked_range = not_found_range();
        self.selected_range = selected_range;
        self.caret_range = selected_range;
        Ok(())
    }

    fn set_marked_text(
        &mut self,
        text: &str,
        selected_range: NSRange,
        replacement_range: NSRange,
    ) -> Result<(), TextEditError> {
        let replacement_range = appkit_replacement_range(replacement_range)?;
        let marked_selection = Utf16TextRange::new(selected_range.location, selected_range.length)?;
        marked_selection.to_text_range(text)?;
        let marked_start = replacement_range
            .map(Utf16TextRange::location)
            .or_else(|| {
                (self.marked_range.location != NSNotFound as NSUInteger)
                    .then_some(self.marked_range.location)
            })
            .or_else(|| {
                (self.selected_range.location != NSNotFound as NSUInteger)
                    .then_some(self.selected_range.location)
            });
        let document_ranges = match marked_start {
            Some(marked_start) => {
                let marked_range = Utf16TextRange::new(marked_start, text.encode_utf16().count())?;
                let selected_location =
                    Utf16TextRange::new(marked_start, marked_selection.location())?.end();
                let selected_range =
                    Utf16TextRange::new(selected_location, marked_selection.length())?;
                (ns_range(Some(selected_range)), ns_range(Some(marked_range)))
            }
            None => (not_found_range(), not_found_range()),
        };
        let event = if self.marked_text.is_some() {
            replacement_range.map_or_else(
                || TextInputCommand::UpdateComposition(text.to_string()),
                |replacement_range| TextInputCommand::UpdateCompositionReplacing {
                    text: text.to_string(),
                    replacement_range,
                },
            )
        } else {
            replacement_range.map_or_else(
                || TextInputCommand::BeginComposition(text.to_string()),
                |replacement_range| TextInputCommand::BeginCompositionReplacing {
                    text: text.to_string(),
                    replacement_range,
                },
            )
        };
        self.marked_text = Some(text.to_string());
        self.events.push_back(event);
        self.events
            .push_back(TextInputCommand::SetCompositionSelection(marked_selection));
        self.selected_range = document_ranges.0;
        self.marked_range = document_ranges.1;
        self.caret_range = collapsed_at_end(self.selected_range);
        Ok(())
    }

    fn cancel_composition(&mut self) {
        if self.marked_text.take().is_some() {
            self.events.push_back(TextInputCommand::CancelComposition);
        }
        self.marked_range = not_found_range();
    }

    fn commit_marked_text(&mut self) {
        if let Some(text) = self.marked_text.take() {
            self.events
                .push_back(TextInputCommand::CommitComposition(text));
        }
        self.selected_range = collapsed_at_end(self.marked_range);
        self.caret_range = self.selected_range;
        self.marked_range = not_found_range();
    }

    fn discard_marked_text(&mut self) -> bool {
        let had_marked_text = self.marked_text.take().is_some();
        self.marked_range = not_found_range();
        had_marked_text
    }

    fn has_marked_text(&self) -> bool {
        self.marked_text.is_some()
    }

    fn marked_range(&self) -> NSRange {
        self.marked_range
    }

    fn selected_range(&self) -> NSRange {
        self.selected_range
    }

    fn update_text_input_state(
        &mut self,
        snapshot: Option<TextInputSnapshot>,
        selected_range: Option<Utf16TextRange>,
        marked_range: Option<Utf16TextRange>,
        caret_range: Option<Utf16TextRange>,
        caret_bounds: Option<Bounds>,
    ) -> bool {
        let selected_range = ns_range(selected_range);
        let marked_range = ns_range(marked_range);
        let caret_range = ns_range(caret_range);
        let changed = self.selected_range != selected_range
            || self.marked_range != marked_range
            || self.caret_range != caret_range
            || self.caret_bounds != caret_bounds
            || self.snapshot != snapshot;
        self.selected_range = selected_range;
        self.marked_range = marked_range;
        self.caret_range = caret_range;
        self.caret_bounds = caret_bounds;
        self.snapshot = snapshot;
        changed
    }

    fn attributed_substring(&self, range: NSRange) -> Option<(NSRange, String)> {
        let snapshot = self.snapshot.as_ref()?;
        let utf16 = appkit_replacement_range(range).ok()??;
        let range = utf16.to_text_range(snapshot.text()).ok()?;
        Some((
            ns_range(Some(utf16)),
            snapshot.text()[range.start()..range.end()].to_string(),
        ))
    }

    fn range_geometry(&self, range: NSRange) -> Option<(NSRange, Bounds)> {
        let snapshot = self.snapshot.as_ref()?;
        let geometry = snapshot.geometry()?;
        let requested = appkit_replacement_range(range).ok()??;
        let requested = requested.to_text_range(snapshot.text()).ok()?;
        let (actual, bounds) = geometry.first_bounds_for_range(requested).ok()??;
        let actual = Utf16TextRange::from_text_range(snapshot.text(), actual).ok()?;
        Some((ns_range(Some(actual)), bounds))
    }

    fn character_index_for_point(&self, point: Point) -> Option<NSUInteger> {
        let snapshot = self.snapshot.as_ref()?;
        let offset = snapshot.geometry()?.text_offset_for_point(point)?;
        Utf16TextRange::from_text_range(
            snapshot.text(),
            crate::core::text_editing::TextRange::collapsed(offset),
        )
        .ok()
        .map(Utf16TextRange::location)
    }

    fn caret_geometry(&self) -> (NSRange, Option<Bounds>) {
        (self.caret_range, self.caret_bounds)
    }

    fn drain_events(&mut self) -> Vec<TextInputCommand> {
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
            range: NSRange,
            actual_range: NSRangePointer,
        ) -> Option<Retained<NSAttributedString>> {
            match self.ivars().ime.borrow().attributed_substring(range) {
                Some((range, text)) => {
                    if let Some(actual_range) = unsafe { actual_range.as_mut() } {
                        *actual_range = range;
                    }
                    Some(NSAttributedString::from_nsstring(&NSString::from_str(
                        &text,
                    )))
                }
                None => {
                    if let Some(actual_range) = unsafe { actual_range.as_mut() } {
                        *actual_range = not_found_range();
                    }
                    None
                }
            }
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
            let geometry = self.ivars().ime.borrow().range_geometry(range);
            let fallback = (range.length == 0).then(|| {
                let (range, bounds) = self.ivars().ime.borrow().caret_geometry();
                bounds.map(|bounds| (range, bounds))
            });
            let Some((actual, bounds)) = geometry.or_else(|| fallback.flatten()) else {
                if let Some(actual_range) = unsafe { actual_range.as_mut() } {
                    *actual_range = not_found_range();
                }
                return NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
            };
            let Some(window) = self.window() else {
                if let Some(actual_range) = unsafe { actual_range.as_mut() } {
                    *actual_range = not_found_range();
                }
                return NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
            };
            if let Some(actual_range) = unsafe { actual_range.as_mut() } {
                *actual_range = actual;
            }
            let bounds = if range.length == 0 {
                appkit_view_caret_rect(bounds, self.bounds().size.height)
            } else {
                appkit_view_rect(bounds, self.bounds().size.height)
            };
            let window_bounds = self.convertRect_toView(bounds, None);
            window.convertRectToScreen(window_bounds)
        }

        #[unsafe(method(characterIndexForPoint:))]
        fn characterIndexForPoint(&self, point: NSPoint) -> NSUInteger {
            let Some(window) = self.window() else {
                return NSNotFound as NSUInteger;
            };
            let window_point = window.convertPointFromScreen(point);
            let view_point = self.convertPoint_fromView(window_point, None);
            let framework_point = Point::new(
                view_point.x as f32,
                (self.bounds().size.height - view_point.y) as f32,
            );
            self.ivars()
                .ime
                .borrow()
                .character_index_for_point(framework_point)
                .unwrap_or(NSNotFound as NSUInteger)
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

    pub(crate) fn drain_ime_events(&self) -> Vec<TextInputCommand> {
        self.ivars().ime.borrow_mut().drain_events()
    }

    pub(crate) fn discard_marked_text(&self) {
        let had_marked_text = self.ivars().ime.borrow_mut().discard_marked_text();
        if had_marked_text && let Some(input_context) = self.inputContext() {
            input_context.discardMarkedText();
        }
    }

    pub(crate) fn update_text_input_state(
        &self,
        snapshot: Option<TextInputSnapshot>,
        selected_range: Option<Utf16TextRange>,
        marked_range: Option<Utf16TextRange>,
        caret_range: Option<Utf16TextRange>,
        caret_bounds: Option<Bounds>,
    ) {
        let changed = self.ivars().ime.borrow_mut().update_text_input_state(
            snapshot,
            selected_range,
            marked_range,
            caret_range,
            caret_bounds,
        );
        if changed && let Some(input_context) = self.inputContext() {
            input_context.invalidateCharacterCoordinates();
        }
    }
}

pub(crate) fn suppress_key_down_for_ime(
    events: &mut Vec<PlatformWindowEvent>,
    ime_events: &[TextInputCommand],
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
#[path = "text_input_tests.rs"]
mod tests;
