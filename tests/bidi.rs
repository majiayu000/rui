//! Bidirectional text: laid out in the order UAX #9 gives, edited in logical
//! order.

use rui::core::Point;
use rui::core::text_editing::{TextEditLayout, TextRange};
use rui::renderer::text::{
    RichPieceText, RichSpan, TextCluster, TextMeasureCache, TextRequest, TextShapePlan,
};
use unicode_bidi::BidiInfo;

fn shape(text: &str) -> TextShapePlan {
    TextMeasureCache::new()
        .shape_single_line(TextRequest::new(text, 20.0, 400, None, 1.2))
        .unwrap_or_else(|err| panic!("shaping {text:?} failed: {err:?}"))
}

/// The clusters left to right on screen.
fn on_screen(plan: &TextShapePlan) -> Vec<&TextCluster> {
    let mut clusters: Vec<&TextCluster> = plan.clusters().iter().collect();
    clusters.sort_by(|left, right| left.x_offset.total_cmp(&right.x_offset));
    clusters
}

fn screen_text(plan: &TextShapePlan) -> String {
    on_screen(plan)
        .iter()
        .map(|cluster| cluster.text.as_str())
        .collect()
}

fn cluster<'a>(plan: &'a TextShapePlan, text: &str) -> &'a TextCluster {
    plan.clusters()
        .iter()
        .find(|cluster| cluster.text == text)
        .unwrap_or_else(|| panic!("no cluster {text:?}"))
}

/// What unicode-bidi itself puts on screen for one line.
fn reference(text: &str) -> String {
    let bidi = BidiInfo::new(text, None);
    let paragraph = &bidi.paragraphs[0];
    bidi.reorder_line(paragraph, paragraph.range.clone())
        .into_owned()
}

#[test]
fn hebrew_and_digits_inside_latin_read_in_display_order() {
    // The Hebrew words swap places, the digits between them stay left to right.
    let plan = shape("abc שלום 123 עולם");
    assert_eq!(screen_text(&plan), "abc םלוע 123 םולש");
    assert_eq!(screen_text(&plan), reference("abc שלום 123 עולם"));
    assert!(cluster(&plan, "1").x_offset < cluster(&plan, "3").x_offset);
    assert!(cluster(&plan, "ש").is_rtl() && !cluster(&plan, "1").is_rtl());
    assert!(plan.diagnostics().is_empty(), "{:?}", plan.diagnostics());
}

#[test]
fn a_right_to_left_line_puts_its_latin_and_digits_on_the_left() {
    assert_eq!(screen_text(&shape("שלום abc")), "abc םולש");
    // The paragraph is Arabic: the year comes first on the left.
    let plan = shape("مرحبا 2024");
    assert_eq!(screen_text(&plan), "2024 ابحرم");
    assert_eq!(screen_text(&plan), reference("مرحبا 2024"));
    // A space between Arabic and digits is laid out right to left.
    assert!(cluster(&plan, " ").is_rtl());
}

#[test]
fn every_mixed_line_matches_unicode_bidi_and_its_clusters_tile_it() {
    let words = [
        "abc",
        "Hello",
        "שלום",
        "עולם",
        "مرحبا",
        "كتب",
        "123",
        "4.5",
        "(x)",
        "!",
        "-",
        "🙂",
    ];
    let mut seed = 0x2545_f491_u32;
    for _ in 0..300 {
        let mut text = String::new();
        for index in 0..1 + seed as usize % 6 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            if index > 0 {
                text.push(' ');
            }
            text.push_str(words[seed as usize % words.len()]);
        }
        let plan = shape(&text);
        // Mirrored brackets aside, the screen reads as unicode-bidi orders it.
        let mirror = |text: String| text.replace(['(', ')'], "");
        assert_eq!(
            mirror(screen_text(&plan)),
            mirror(reference(&text)),
            "{text:?}"
        );
        let mut x = 0.0;
        for cluster in on_screen(&plan) {
            assert!(
                (cluster.x_offset - x).abs() < 0.01,
                "{text:?} at {cluster:?}"
            );
            x = cluster.x_offset + cluster.advance_width;
        }
        assert!((x - plan.metrics().advance_width).abs() < 0.01, "{text:?}");
    }
}

#[test]
fn latin_only_text_keeps_every_cluster_at_level_zero() {
    let plan = shape("plain text 123");
    assert!(
        plan.clusters()
            .iter()
            .all(|cluster| cluster.bidi_level == 0)
    );
    assert_eq!(screen_text(&plan), "plain text 123");
}

fn layout(text: &str) -> TextEditLayout {
    TextEditLayout::from_shape_plan(text, &shape(text)).expect("layout")
}

#[test]
fn the_caret_stays_logical_and_lands_where_the_text_is_drawn() {
    let text = "abc שלום 123 עולם";
    let plan = shape(text);
    let layout = layout(text);
    let x = |offset: usize| layout.caret_for_offset(offset).expect("caret").position.x;

    // The end of the text is the end of עולם: the left edge of its last
    // letter, drawn right after "abc ".
    let last = plan.clusters().last().expect("clusters");
    assert_eq!(last.byte_end, text.len());
    assert!((x(text.len()) - last.x_offset).abs() < 0.01);
    let abc_space = &plan.clusters()[3];
    assert!((x(text.len()) - (abc_space.x_offset + abc_space.advance_width)).abs() < 0.01);
    // After ש, the caret is on its left.
    let shin = cluster(&plan, "ש");
    let after_shin = 4 + "ש".len();
    assert!((x(after_shin) - shin.x_offset).abs() < 0.01);

    // The line's left edge is the start, its right edge the start of שלום.
    assert_eq!(layout.visual_line_start(10).expect("line start"), 0);
    assert_eq!(layout.visual_line_end(10).expect("line end"), 4);
}

#[test]
fn hit_testing_and_selection_follow_the_display_order() {
    let text = "abc שלום 123 עולם";
    let plan = shape(text);
    let layout = layout(text);
    let one = cluster(&plan, "1");
    let middle = Point::new(one.x_offset + one.advance_width / 2.0, 5.0);
    assert_eq!(layout.text_offset_for_point(middle), Some(one.byte_start));
    // The far right of the line is the start of שלום, drawn last.
    let far_right = Point::new(plan.metrics().advance_width + 50.0, 5.0);
    assert_eq!(layout.offset_for_point(far_right), 4);

    // "abc שלום" in logical order is the left and right ends of the line.
    let rects = layout
        .selection_rects(TextRange::new(0, 4 + "שלום".len()).expect("range"))
        .expect("selection");
    assert_eq!(rects.len(), 2, "{rects:?}");
    assert!(rects[0].bounds.x() < 1.0);
    assert!((rects[1].bounds.max_x() - plan.metrics().advance_width).abs() < 0.01);
}

#[test]
fn rich_pieces_of_a_right_to_left_paragraph_are_placed_in_display_order() {
    let content = "שלום abc";
    let span = |len: usize, font_weight: u16| RichSpan {
        len,
        font_size: 20.0,
        font_weight,
        font_family: None,
        line_height: 1.2,
    };
    let hebrew = "שלום ".len();
    let wrapped = TextMeasureCache::new()
        .wrap_rich(
            content,
            &[span(hebrew, 400), span(content.len() - hebrew, 700)],
            f32::INFINITY,
            None,
        )
        .expect("wrap");
    let pieces = &wrapped.lines[0].pieces;
    let piece = |bytes: std::ops::Range<usize>| {
        pieces
            .iter()
            .find(|piece| piece.text == RichPieceText::Content(bytes.clone()))
            .unwrap_or_else(|| panic!("no piece for {bytes:?} in {pieces:?}"))
    };
    let latin = piece(hebrew..content.len());
    assert_eq!(latin.x, 0.0, "abc is on the left");
    assert!((piece(0..hebrew).x - latin.width).abs() < 0.01);
}
