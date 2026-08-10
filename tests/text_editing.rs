use rui::core::color::Rgba;
use rui::core::event::{KeyCode, KeyEvent, Modifiers};
use rui::core::geometry::Point;
use rui::core::text_editing::{
    ClipboardError, MemoryClipboard, TextEditBuffer, TextEditError, TextEditLayout,
    TextEditPaintStyle, TextInputEvent, TextRange, TextSelection, Utf16TextRange,
};
use rui::renderer::Primitive;
use rui::renderer::text::{TextMeasureCache, TextRequest};

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

fn assert_close(left: f32, right: f32) {
    assert!(
        (left - right).abs() <= 0.01,
        "{left} should be within 0.01 of {right}"
    );
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
fn text_editing_commit_without_active_composition_inserts_text() {
    let mut buffer = TextEditBuffer::with_text("hello ");
    must(buffer.apply_text_input_event(TextInputEvent::CommitComposition("你好".to_string())));

    assert_eq!(buffer.text(), "hello 你好");
    assert!(buffer.composition().is_none());
    assert_eq!(buffer.cursor(), "hello 你好".len());
}

#[test]
fn text_editing_replacement_ranges_convert_utf16_without_splitting_surrogates() {
    let mut buffer = TextEditBuffer::with_text("a😀bc");
    let emoji = must(Utf16TextRange::new(1, 2));

    must(
        buffer.apply_text_input_event(TextInputEvent::InsertTextReplacing {
            text: "X".to_string(),
            replacement_range: emoji,
        }),
    );

    assert_eq!(buffer.text(), "aXbc");
    assert_eq!(buffer.cursor(), 2);

    let invalid = must(Utf16TextRange::new(0, 1));
    let mut surrogate = TextEditBuffer::with_text("😀");
    let error = surrogate
        .apply_text_input_event(TextInputEvent::InsertTextReplacing {
            text: "X".to_string(),
            replacement_range: invalid,
        })
        .expect_err("a range ending inside a surrogate pair must fail");
    assert!(matches!(error, TextEditError::InvalidUtf16Range { .. }));
    assert_eq!(surrogate.text(), "😀");

    assert!(matches!(
        Utf16TextRange::new(usize::MAX, 1),
        Err(TextEditError::InvalidUtf16Range { .. })
    ));
}

#[test]
fn text_editing_composition_honors_concrete_utf16_replacement_ranges() {
    let mut buffer = TextEditBuffer::with_text("a😀bc");
    must(
        buffer.apply_text_input_event(TextInputEvent::BeginCompositionReplacing {
            text: "你".to_string(),
            replacement_range: must(Utf16TextRange::new(1, 2)),
        }),
    );
    assert_eq!(buffer.text(), "a你bc");

    must(
        buffer.apply_text_input_event(TextInputEvent::UpdateCompositionReplacing {
            text: "好".to_string(),
            replacement_range: must(Utf16TextRange::new(1, 1)),
        }),
    );
    assert_eq!(buffer.text(), "a好bc");

    must(buffer.apply_text_input_event(TextInputEvent::CancelComposition));
    assert_eq!(buffer.text(), "a😀bc");
}

#[test]
fn text_editing_applies_marked_selection_relative_to_composition_text() {
    let mut buffer = TextEditBuffer::with_text("ab");
    must(buffer.set_cursor(1));
    must(buffer.apply_text_input_event(TextInputEvent::BeginComposition("a😀z".to_string())));

    must(
        buffer.apply_text_input_event(TextInputEvent::SetCompositionSelection(must(
            Utf16TextRange::new(1, 2),
        ))),
    );
    assert_eq!(buffer.selection(), TextSelection::new(2, 6));

    let error = buffer
        .apply_text_input_event(TextInputEvent::SetCompositionSelection(must(
            Utf16TextRange::new(2, 0),
        )))
        .expect_err("selection inside a surrogate pair must fail");
    assert!(matches!(error, TextEditError::InvalidUtf16Range { .. }));
    assert_eq!(buffer.selection(), TextSelection::new(2, 6));
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
    let paint_origin = Point::new(100.0, 50.0);

    let caret = match layout.caret_primitive(1, paint_origin, style) {
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
            assert_eq!(bounds.x(), 110.0);
            assert_eq!(bounds.y(), 50.0);
            assert_eq!(bounds.width(), 2.0);
            assert_eq!(bounds.height(), 20.0);
            assert_eq!(background, Rgba::RED);
            assert_eq!(border_color, Rgba::TRANSPARENT);
            assert_eq!(border_widths, rui::Edges::ZERO);
            assert_eq!(corner_radii, rui::Corners::ZERO);
        }
        other => panic!("expected caret quad primitive, got {other:?}"),
    }

    let selection = match layout.selection_primitives(range(1, 5), paint_origin, style) {
        Ok(primitives) => primitives,
        Err(err) => panic!("selection primitives failed: {err}"),
    };
    assert_eq!(selection.len(), 2);

    match &selection[0] {
        Primitive::Quad {
            bounds, background, ..
        } => {
            assert_eq!(bounds.x(), 110.0);
            assert_eq!(bounds.y(), 50.0);
            assert_eq!(bounds.width(), 10.0);
            assert_eq!(*background, Rgba::BLUE.with_alpha(0.25));
        }
        other => panic!("expected selection quad primitive, got {other:?}"),
    }

    match &selection[1] {
        Primitive::Quad {
            bounds, background, ..
        } => {
            assert_eq!(bounds.x(), 100.0);
            assert_eq!(bounds.y(), 70.0);
            assert_eq!(bounds.width(), 20.0);
            assert_eq!(*background, Rgba::BLUE.with_alpha(0.25));
        }
        other => panic!("expected selection quad primitive, got {other:?}"),
    }

    let empty = match layout.selection_primitives(range(2, 2), paint_origin, style) {
        Ok(primitives) => primitives,
        Err(err) => panic!("empty selection primitives failed: {err}"),
    };
    assert!(empty.is_empty());
}

#[test]
fn text_editing_layout_maps_combining_and_emoji_clusters_to_columns() {
    let text = "e\u{301} 🧑‍💻";
    let layout = TextEditLayout::new(text, 10.0, 20.0);

    let after_combining = "e\u{301}".len();
    let caret = match layout.caret_for_offset(after_combining) {
        Ok(caret) => caret,
        Err(err) => panic!("combining caret geometry failed: {err}"),
    };
    assert_eq!(caret.column, 1);
    assert_eq!(caret.position.x, 10.0);

    let caret = match layout.caret_for_offset(text.len()) {
        Ok(caret) => caret,
        Err(err) => panic!("emoji caret geometry failed: {err}"),
    };
    assert_eq!(caret.column, 3);
    assert_eq!(caret.position.x, 30.0);

    let rects = match layout.selection_rects(range(0, after_combining)) {
        Ok(rects) => rects,
        Err(err) => panic!("combining selection geometry failed: {err}"),
    };
    assert_eq!(rects.len(), 1);
    assert_eq!(rects[0].bounds.width(), 10.0);
}

#[test]
fn text_editing_layout_can_use_shaped_cluster_positions() {
    let text = "Wi界";
    let mut cache = TextMeasureCache::new();
    let plan = match cache.shape_single_line(TextRequest::new(text, 24.0, 400, None, 1.2)) {
        Ok(plan) => plan,
        Err(err) => panic!("text shaping failed: {err:?}"),
    };
    let layout = must(TextEditLayout::from_shape_plan(text, &plan));

    let first = &plan.clusters()[0];
    let second = &plan.clusters()[1];
    let caret = must(layout.caret_for_offset(first.byte_end));
    assert_close(caret.position.x, first.advance_width);

    let rects = must(layout.selection_rects(range(first.byte_start, second.byte_end)));
    assert_eq!(rects.len(), 1);
    assert_close(
        rects[0].bounds.width(),
        first.advance_width + second.advance_width,
    );
}

#[test]
fn text_editing_selection_uses_visual_cluster_bounds_for_rtl_shape_plans() {
    let text = "שלום";
    let mut cache = TextMeasureCache::new();
    let plan = match cache.shape_single_line(TextRequest::new(text, 24.0, 400, None, 1.2)) {
        Ok(plan) => plan,
        Err(err) => panic!("text shaping failed: {err:?}"),
    };
    let layout = must(TextEditLayout::from_shape_plan(text, &plan));
    let first = &plan.clusters()[0];

    let first_rects = must(layout.selection_rects(range(first.byte_start, first.byte_end)));
    assert_eq!(first_rects.len(), 1);
    assert_close(first_rects[0].bounds.x(), first.x_offset);
    assert_close(first_rects[0].bounds.width(), first.advance_width);

    let min_x = plan
        .clusters()
        .iter()
        .map(|cluster| cluster.x_offset)
        .fold(f32::INFINITY, f32::min);
    let max_x = plan
        .clusters()
        .iter()
        .map(|cluster| cluster.x_offset + cluster.advance_width)
        .fold(f32::NEG_INFINITY, f32::max);
    let full_rects = must(layout.selection_rects(range(0, text.len())));
    assert_eq!(full_rects.len(), 1);
    assert_close(full_rects[0].bounds.x(), min_x);
    assert_close(full_rects[0].bounds.width(), max_x - min_x);
}

#[test]
fn text_editing_shape_layout_drives_rtl_navigation_and_hit_testing() {
    let text = "שלום";
    let mut cache = TextMeasureCache::new();
    let plan = cache
        .shape_single_line(TextRequest::new(text, 24.0, 400, None, 1.2))
        .unwrap_or_else(|err| panic!("text shaping failed: {err:?}"));
    let layout = must(TextEditLayout::from_shape_plan(text, &plan));

    let logical_end = text.len();
    let right = must(layout.visual_offset_right(logical_end));
    assert_ne!(right, logical_end);
    assert!(
        must(layout.caret_for_offset(right)).position.x
            > must(layout.caret_for_offset(logical_end)).position.x
    );
    assert_eq!(must(layout.visual_line_start(0)), logical_end);
    assert_eq!(must(layout.visual_line_end(0)), 0);

    let first = &plan.clusters()[0];
    let hit = layout.offset_for_point(Point::new(
        first.x_offset + first.advance_width,
        plan.metrics().size.height / 2.0,
    ));
    assert!(hit == first.byte_start || hit == first.byte_end);
}

#[test]
fn text_editing_multiline_shape_layout_uses_visual_x_for_vertical_navigation() {
    let text = "Wi\nשלום";
    let mut cache = TextMeasureCache::new();
    let plans = [
        cache
            .shape_single_line(TextRequest::new("Wi", 14.0, 400, None, 1.0))
            .unwrap_or_else(|err| panic!("first line shaping failed: {err:?}")),
        cache
            .shape_single_line(TextRequest::new("שלום", 14.0, 400, None, 1.0))
            .unwrap_or_else(|err| panic!("second line shaping failed: {err:?}")),
    ];
    let layout = must(TextEditLayout::from_line_shape_plans(text, &plans, 20.0));
    let second_line_end = text.len();
    let target = must(layout.visual_offset_up(second_line_end));
    let source_x = must(layout.caret_for_offset(second_line_end)).position.x;
    let target_x = must(layout.caret_for_offset(target)).position.x;
    let alternatives = [
        must(layout.caret_for_offset(0)).position.x,
        must(layout.caret_for_offset(1)).position.x,
        must(layout.caret_for_offset(2)).position.x,
    ];

    assert!(
        alternatives
            .iter()
            .all(|x| { (target_x - source_x).abs() <= (*x - source_x).abs() + f32::EPSILON })
    );
    assert_eq!(must(layout.visual_offset_down(target)), second_line_end);
}

#[test]
fn text_editing_shaped_layout_clamps_negative_line_height() {
    let mut cache = TextMeasureCache::new();
    let plan = cache
        .shape_single_line(TextRequest::new("text", 14.0, 400, None, 1.0))
        .unwrap_or_else(|err| panic!("text shaping failed: {err:?}"));
    let layout = must(TextEditLayout::from_shape_plan_with_line_height(
        "text", &plan, -10.0,
    ));

    assert_eq!(layout.lines()[0].measured_size().height, 0.0);
    assert_eq!(must(layout.caret_for_offset(0)).height, 0.0);
}

#[test]
fn text_editing_layout_rejects_mid_grapheme_geometry_offsets() {
    let layout = TextEditLayout::new("e\u{301}", 10.0, 20.0);
    let error = match layout.caret_for_offset(1) {
        Ok(caret) => panic!("mid-grapheme caret should fail, got {caret:?}"),
        Err(err) => err,
    };
    assert_eq!(error, TextEditError::InvalidBoundary { index: 1 });
}

#[test]
fn text_editing_invalid_offsets_and_multiline_policy_return_errors() {
    let mut buffer = TextEditBuffer::with_text("é");
    let error = match buffer.set_cursor(1) {
        Ok(_) => panic!("mid-codepoint cursor should fail"),
        Err(err) => err,
    };
    assert_eq!(error, TextEditError::InvalidBoundary { index: 1 });

    let mut buffer = TextEditBuffer::with_text("e\u{301}");
    let error = match buffer.set_cursor(1) {
        Ok(_) => panic!("mid-grapheme cursor should fail"),
        Err(err) => err,
    };
    assert_eq!(error, TextEditError::InvalidBoundary { index: 1 });

    let error = match buffer.insert_text("a\nb") {
        Ok(_) => panic!("single-line newline insertion should fail"),
        Err(err) => err,
    };
    assert_eq!(error, TextEditError::MultilineDisabled);
}
