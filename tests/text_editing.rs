use rui::core::color::Rgba;
use rui::core::event::{KeyCode, KeyEvent, Modifiers};
use rui::core::text_editing::{
    ClipboardError, MemoryClipboard, TextEditBuffer, TextEditError, TextEditLayout,
    TextEditPaintStyle, TextInputEvent, TextRange, TextSelection,
};
use rui::renderer::Primitive;

fn range(start: usize, end: usize) -> TextRange {
    match TextRange::new(start, end) {
        Ok(range) => range,
        Err(err) => panic!("range construction failed: {err}"),
    }
}

fn must<T>(result: Result<T, TextEditError>) -> T {
    match result {
        Ok(value) => value,
        Err(err) => panic!("text edit operation failed: {err}"),
    }
}

#[test]
fn text_editing_composition_update_commit_and_cancel_are_stateful() {
    let mut buffer = TextEditBuffer::with_text("hello");
    must(buffer.set_selection(TextSelection::new(0, 5)));
    must(buffer.apply_text_input_event(TextInputEvent::BeginComposition("你".to_string())));

    assert_eq!(buffer.text(), "你");
    assert_eq!(buffer.composition().map(|state| state.text()), Some("你"));

    must(buffer.apply_text_input_event(TextInputEvent::UpdateComposition("你好".to_string())));
    assert_eq!(buffer.text(), "你好");
    assert_eq!(buffer.composition().map(|state| state.text()), Some("你好"));

    must(buffer.apply_text_input_event(TextInputEvent::CommitComposition("您好".to_string())));
    assert_eq!(buffer.text(), "您好");
    assert!(buffer.composition().is_none());

    must(buffer.set_selection(TextSelection::new(0, buffer.text().len())));
    must(buffer.apply_text_input_event(TextInputEvent::BeginComposition("abc".to_string())));
    assert_eq!(buffer.text(), "abc");

    must(buffer.apply_text_input_event(TextInputEvent::CancelComposition));
    assert_eq!(buffer.text(), "您好");
    assert!(buffer.composition().is_none());
}

#[test]
fn text_editing_grapheme_delete_keeps_unicode_clusters_intact() {
    let mut buffer = TextEditBuffer::with_text("a e\u{301} 🧑‍💻");
    must(buffer.delete_backward());

    assert_eq!(buffer.text(), "a e\u{301} ");

    must(buffer.delete_backward());
    assert_eq!(buffer.text(), "a e\u{301}");

    must(buffer.delete_backward());
    assert_eq!(buffer.text(), "a ");
}

#[test]
fn text_editing_selection_and_word_navigation_use_explicit_ranges() {
    let mut buffer = TextEditBuffer::with_text("alpha beta");

    must(buffer.move_word_left(false));
    assert_eq!(buffer.cursor(), 6);

    must(buffer.move_word_left(false));
    assert_eq!(buffer.cursor(), 0);

    must(buffer.move_word_right(true));
    assert_eq!(buffer.selected_range(), range(0, 5));
    assert_eq!(buffer.selected_text(), "alpha");

    must(buffer.insert_text("gamma"));
    assert_eq!(buffer.text(), "gamma beta");
    assert_eq!(buffer.cursor(), 5);
}

#[test]
fn text_editing_key_events_handle_submit_and_multiline_enter() {
    let mut single = TextEditBuffer::with_text("query");
    let enter = KeyEvent::new(KeyCode::Enter, Modifiers::none());
    let outcome = must(single.apply_key_event(&enter));
    assert!(!outcome.changed);
    assert!(outcome.submitted);
    assert_eq!(single.text(), "query");

    let mut multi = TextEditBuffer::multiline_with_text("a");
    let outcome = must(multi.apply_key_event(&enter));
    assert!(outcome.changed);
    assert!(!outcome.submitted);
    assert_eq!(multi.text(), "a\n");
}

#[test]
fn text_editing_multiline_vertical_motion_preserves_grapheme_column() {
    let mut buffer = TextEditBuffer::multiline_with_text("ab\ncde\nf");
    must(buffer.set_cursor(5));

    must(buffer.move_up(false));
    assert_eq!(buffer.cursor(), 2);

    must(buffer.move_down(false));
    assert_eq!(buffer.cursor(), 5);

    must(buffer.move_down(false));
    assert_eq!(buffer.cursor(), 8);
}

#[test]
fn text_editing_clipboard_copy_cut_paste_and_errors_are_explicit() {
    let mut buffer = TextEditBuffer::with_text("alpha beta");
    let mut clipboard = MemoryClipboard::new();
    must(buffer.set_selection(TextSelection::new(0, 5)));

    assert!(must(buffer.copy_selection_to(&mut clipboard)));
    assert_eq!(clipboard.text(), "alpha");

    let outcome = must(buffer.cut_selection_to(&mut clipboard));
    assert!(outcome.changed);
    assert_eq!(buffer.text(), " beta");

    must(buffer.paste_from(&mut clipboard));
    assert_eq!(buffer.text(), "alpha beta");

    let mut read_error = MemoryClipboard::with_read_error("denied");
    let error = match buffer.paste_from(&mut read_error) {
        Ok(_) => panic!("clipboard read should fail"),
        Err(err) => err,
    };
    assert!(matches!(
        error,
        TextEditError::Clipboard(ClipboardError::ReadFailed { .. })
    ));

    let mut write_error = MemoryClipboard::with_write_error("readonly");
    must(buffer.set_selection(TextSelection::new(0, 5)));
    let error = match buffer.copy_selection_to(&mut write_error) {
        Ok(_) => panic!("clipboard write should fail"),
        Err(err) => err,
    };
    assert!(matches!(
        error,
        TextEditError::Clipboard(ClipboardError::WriteFailed { .. })
    ));
}

#[test]
fn text_editing_layout_reports_caret_and_selection_geometry() {
    let layout = TextEditLayout::new("ab\ncde", 10.0, 20.0);
    assert_eq!(layout.lines().len(), 2);
    assert_eq!(layout.lines()[0].text_range(), range(0, 2));
    assert_eq!(layout.lines()[1].text_range(), range(3, 6));

    let caret = match layout.caret_for_offset(5) {
        Ok(caret) => caret,
        Err(err) => panic!("caret geometry failed: {err}"),
    };
    assert_eq!(caret.line_index, 1);
    assert_eq!(caret.column, 2);
    assert_eq!(caret.position.x, 20.0);
    assert_eq!(caret.position.y, 20.0);

    let rects = match layout.selection_rects(range(1, 5)) {
        Ok(rects) => rects,
        Err(err) => panic!("selection geometry failed: {err}"),
    };
    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0].range, range(1, 2));
    assert_eq!(rects[0].bounds.width(), 10.0);
    assert_eq!(rects[1].range, range(3, 5));
    assert_eq!(rects[1].bounds.width(), 20.0);
}

#[test]
fn text_editing_layout_exposes_renderer_primitives_for_caret_and_selection() {
    let layout = TextEditLayout::new("ab\ncde", 10.0, 20.0);
    let style = TextEditPaintStyle::new(2.0, Rgba::RED, Rgba::BLUE.with_alpha(0.25));

    let caret = match layout.caret_primitive(1, style) {
        Ok(primitive) => primitive,
        Err(err) => panic!("caret primitive failed: {err}"),
    };

    match caret {
        Primitive::Quad {
            bounds,
            background,
            border_color,
            border_widths,
            corner_radii,
        } => {
            assert_eq!(bounds.x(), 10.0);
            assert_eq!(bounds.y(), 0.0);
            assert_eq!(bounds.width(), 2.0);
            assert_eq!(bounds.height(), 20.0);
            assert_eq!(background, Rgba::RED);
            assert_eq!(border_color, Rgba::TRANSPARENT);
            assert_eq!(border_widths, rui::Edges::ZERO);
            assert_eq!(corner_radii, rui::Corners::ZERO);
        }
        other => panic!("expected caret quad primitive, got {other:?}"),
    }

    let selection = match layout.selection_primitives(range(1, 5), style) {
        Ok(primitives) => primitives,
        Err(err) => panic!("selection primitives failed: {err}"),
    };
    assert_eq!(selection.len(), 2);

    match &selection[0] {
        Primitive::Quad {
            bounds, background, ..
        } => {
            assert_eq!(bounds.x(), 10.0);
            assert_eq!(bounds.y(), 0.0);
            assert_eq!(bounds.width(), 10.0);
            assert_eq!(*background, Rgba::BLUE.with_alpha(0.25));
        }
        other => panic!("expected selection quad primitive, got {other:?}"),
    }

    match &selection[1] {
        Primitive::Quad {
            bounds, background, ..
        } => {
            assert_eq!(bounds.x(), 0.0);
            assert_eq!(bounds.y(), 20.0);
            assert_eq!(bounds.width(), 20.0);
            assert_eq!(*background, Rgba::BLUE.with_alpha(0.25));
        }
        other => panic!("expected selection quad primitive, got {other:?}"),
    }

    let empty = match layout.selection_primitives(range(2, 2), style) {
        Ok(primitives) => primitives,
        Err(err) => panic!("empty selection primitives failed: {err}"),
    };
    assert!(empty.is_empty());
}

#[test]
fn text_editing_invalid_offsets_and_multiline_policy_return_errors() {
    let mut buffer = TextEditBuffer::with_text("é");
    let error = match buffer.set_cursor(1) {
        Ok(_) => panic!("mid-codepoint cursor should fail"),
        Err(err) => err,
    };
    assert_eq!(error, TextEditError::InvalidBoundary { index: 1 });

    let error = match buffer.insert_text("a\nb") {
        Ok(_) => panic!("single-line newline insertion should fail"),
        Err(err) => err,
    };
    assert_eq!(error, TextEditError::MultilineDisabled);
}
