use super::*;
use crate::elements::text::text;

// ========== ListStyle tests ==========

#[test]
fn test_bullet_marker() {
    let style = ListStyle::Bullet;
    assert_eq!(style.marker(0), "\u{2022}");
    assert_eq!(style.marker(5), "\u{2022}");
    assert_eq!(style.marker(100), "\u{2022}");
}

#[test]
fn test_numbered_marker() {
    let style = ListStyle::Numbered;
    assert_eq!(style.marker(0), "1.");
    assert_eq!(style.marker(1), "2.");
    assert_eq!(style.marker(9), "10.");
    assert_eq!(style.marker(99), "100.");
}

#[test]
fn test_lowercase_alpha_marker() {
    let style = ListStyle::LowercaseAlpha;
    assert_eq!(style.marker(0), "a.");
    assert_eq!(style.marker(1), "b.");
    assert_eq!(style.marker(25), "z.");
    assert_eq!(style.marker(26), "aa.");
    assert_eq!(style.marker(27), "ab.");
    assert_eq!(style.marker(51), "az.");
    assert_eq!(style.marker(52), "ba.");
}

#[test]
fn test_uppercase_alpha_marker() {
    let style = ListStyle::UppercaseAlpha;
    assert_eq!(style.marker(0), "A.");
    assert_eq!(style.marker(1), "B.");
    assert_eq!(style.marker(25), "Z.");
    assert_eq!(style.marker(26), "AA.");
}

#[test]
fn test_lowercase_roman_marker() {
    let style = ListStyle::LowercaseRoman;
    assert_eq!(style.marker(0), "i.");
    assert_eq!(style.marker(1), "ii.");
    assert_eq!(style.marker(2), "iii.");
    assert_eq!(style.marker(3), "iv.");
    assert_eq!(style.marker(4), "v.");
    assert_eq!(style.marker(8), "ix.");
    assert_eq!(style.marker(9), "x.");
    assert_eq!(style.marker(49), "l.");
    assert_eq!(style.marker(99), "c.");
    assert_eq!(style.marker(999), "m.");
}

#[test]
fn test_uppercase_roman_marker() {
    let style = ListStyle::UppercaseRoman;
    assert_eq!(style.marker(0), "I.");
    assert_eq!(style.marker(3), "IV.");
    assert_eq!(style.marker(4), "V.");
    assert_eq!(style.marker(9), "X.");
    assert_eq!(style.marker(49), "L.");
    assert_eq!(style.marker(99), "C.");
    assert_eq!(style.marker(499), "D.");
    assert_eq!(style.marker(999), "M.");
}

#[test]
fn test_none_marker() {
    let style = ListStyle::None;
    assert_eq!(style.marker(0), "");
    assert_eq!(style.marker(100), "");
}

#[test]
fn test_complex_roman_numerals() {
    // Test specific roman numeral conversions
    assert_eq!(ListStyle::to_roman(1, true), "I");
    assert_eq!(ListStyle::to_roman(4, true), "IV");
    assert_eq!(ListStyle::to_roman(9, true), "IX");
    assert_eq!(ListStyle::to_roman(14, true), "XIV");
    assert_eq!(ListStyle::to_roman(40, true), "XL");
    assert_eq!(ListStyle::to_roman(49, true), "XLIX");
    assert_eq!(ListStyle::to_roman(90, true), "XC");
    assert_eq!(ListStyle::to_roman(99, true), "XCIX");
    assert_eq!(ListStyle::to_roman(400, true), "CD");
    assert_eq!(ListStyle::to_roman(900, true), "CM");
    assert_eq!(ListStyle::to_roman(1994, true), "MCMXCIV");
    assert_eq!(ListStyle::to_roman(2024, true), "MMXXIV");
}

#[test]
fn test_alpha_sequence() {
    // Test the full alphabet sequence
    for i in 0..26 {
        let expected = ((b'a' + i as u8) as char).to_string() + ".";
        assert_eq!(ListStyle::LowercaseAlpha.marker(i), expected);
    }
}

// ========== List builder tests ==========

#[test]
fn test_list_new() {
    let l = List::new();
    assert!(l.is_empty());
    assert_eq!(l.len(), 0);
    assert_eq!(l.get_list_style(), ListStyle::Bullet);
}

#[test]
fn test_list_default() {
    let l = List::default();
    assert!(l.is_empty());
    assert_eq!(l.get_list_style(), ListStyle::Bullet);
}

#[test]
fn test_list_ordered() {
    let l = List::new().ordered();
    assert_eq!(l.get_list_style(), ListStyle::Numbered);
}

#[test]
fn test_list_unordered() {
    let l = List::new().unordered();
    assert_eq!(l.get_list_style(), ListStyle::Bullet);
}

#[test]
fn test_list_alpha() {
    let l = List::new().alpha();
    assert_eq!(l.get_list_style(), ListStyle::LowercaseAlpha);
}

#[test]
fn test_list_alpha_upper() {
    let l = List::new().alpha_upper();
    assert_eq!(l.get_list_style(), ListStyle::UppercaseAlpha);
}

#[test]
fn test_list_roman() {
    let l = List::new().roman();
    assert_eq!(l.get_list_style(), ListStyle::LowercaseRoman);
}

#[test]
fn test_list_roman_upper() {
    let l = List::new().roman_upper();
    assert_eq!(l.get_list_style(), ListStyle::UppercaseRoman);
}

#[test]
fn test_list_no_marker() {
    let l = List::new().no_marker();
    assert_eq!(l.get_list_style(), ListStyle::None);
}

#[test]
fn test_list_style_setter() {
    let l = List::new().list_style(ListStyle::UppercaseRoman);
    assert_eq!(l.get_list_style(), ListStyle::UppercaseRoman);
}

#[test]
fn test_list_add_item() {
    let l = list().item(text("Item 1"));
    assert_eq!(l.len(), 1);
    assert!(!l.is_empty());
}

#[test]
fn test_list_add_items() {
    let texts = vec![text("Item 1"), text("Item 2"), text("Item 3")];
    let l = list().items(texts);
    assert_eq!(l.len(), 3);
}

#[test]
fn test_list_chained_items() {
    let l = list()
        .item(text("First"))
        .item(text("Second"))
        .item(text("Third"));
    assert_eq!(l.len(), 3);
}

#[test]
fn test_list_gap() {
    let l = list().gap(16.0);
    assert_eq!(l.gap, 16.0);
}

#[test]
fn test_list_marker_color() {
    let l = list().marker_color(Color::RED);
    assert_eq!(l.marker_color, Color::RED);
}

#[test]
fn test_list_marker_size() {
    let l = list().marker_size(18.0);
    assert_eq!(l.marker_font_size, 18.0);
}

#[test]
fn test_list_marker_width() {
    let l = list().marker_width(32.0);
    assert_eq!(l.marker_width, 32.0);
}

#[test]
fn test_list_start_index() {
    let l = list().start(5);
    assert_eq!(l.start_index, 5);
}

#[test]
fn test_list_id() {
    let id = ElementId::new();
    let l = list().id(id);
    assert_eq!(Element::id(&l), Some(id));
}

// ========== ListItem tests ==========

#[test]
fn test_list_item_new() {
    let item = ListItem::new(text("Test"));
    assert!(item.id.is_none());
}

#[test]
fn test_list_item_id() {
    let id = ElementId::new();
    let item = ListItem::new(text("Test")).id(id);
    assert_eq!(Element::id(&item), Some(id));
}

// ========== Helper function tests ==========

#[test]
fn test_list_helper() {
    let l = list();
    assert_eq!(l.get_list_style(), ListStyle::Bullet);
}

#[test]
fn test_ordered_list_helper() {
    let l = ordered_list();
    assert_eq!(l.get_list_style(), ListStyle::Numbered);
}

#[test]
fn test_unordered_list_helper() {
    let l = unordered_list();
    assert_eq!(l.get_list_style(), ListStyle::Bullet);
}

// ========== Element trait tests ==========

#[test]
fn test_list_style_method() {
    let l = list();
    // Just verify we can access the style
    let _style = l.style();
}

#[test]
fn test_list_item_style_method() {
    let item = ListItem::new(text("Test"));
    let _style = item.style();
}

// ========== Complex scenario tests ==========

#[test]
fn test_nested_configuration() {
    let l = list()
        .ordered()
        .gap(12.0)
        .marker_color(Color::BLUE)
        .marker_size(16.0)
        .marker_width(30.0)
        .start(3)
        .item(text("Fourth item"))
        .item(text("Fifth item"));

    assert_eq!(l.get_list_style(), ListStyle::Numbered);
    assert_eq!(l.gap, 12.0);
    assert_eq!(l.marker_color, Color::BLUE);
    assert_eq!(l.marker_font_size, 16.0);
    assert_eq!(l.marker_width, 30.0);
    assert_eq!(l.start_index, 3);
    assert_eq!(l.len(), 2);
}

#[test]
fn test_style_override_chain() {
    // Test that the last style call wins
    let l = list().ordered().unordered().alpha().roman().roman_upper();

    assert_eq!(l.get_list_style(), ListStyle::UppercaseRoman);
}

#[test]
fn test_default_values() {
    let l = List::new();
    assert_eq!(l.gap, 8.0);
    assert_eq!(l.marker_color, Color::BLACK);
    assert_eq!(l.marker_font_size, 14.0);
    assert_eq!(l.marker_width, 24.0);
    assert_eq!(l.start_index, 0);
}

#[test]
fn test_list_style_default() {
    let style = ListStyle::default();
    assert_eq!(style, ListStyle::Bullet);
}

#[test]
fn test_list_style_clone() {
    let style = ListStyle::Numbered;
    let cloned = style.clone();
    assert_eq!(style, cloned);
}

#[test]
fn test_list_style_copy() {
    let style = ListStyle::UppercaseAlpha;
    let copied: ListStyle = style;
    assert_eq!(style, copied);
}

#[test]
fn test_list_style_debug() {
    let style = ListStyle::Bullet;
    let debug_str = format!("{:?}", style);
    assert!(debug_str.contains("Bullet"));
}

// ========== Edge case tests ==========

#[test]
fn test_large_index_numbered() {
    let style = ListStyle::Numbered;
    assert_eq!(style.marker(9999), "10000.");
}

#[test]
fn test_large_index_alpha() {
    let style = ListStyle::LowercaseAlpha;
    // 702 = 26 * 27 = aaa (26 + 26*26 + 26*26*26 would be...)
    // Actually: 26^0 + 26^1 positions = 26 + 676 = 702 for "zz"
    // Let's just verify it doesn't panic
    let marker = style.marker(1000);
    assert!(!marker.is_empty());
}

#[test]
fn test_large_roman_numeral() {
    let style = ListStyle::UppercaseRoman;
    let marker = style.marker(3999); // 4000 in 1-based, max for standard roman
    assert_eq!(marker, "MMMM."); // 4000 = MMMM in extended form
}

#[test]
fn test_zero_gap() {
    let l = list().gap(0.0);
    assert_eq!(l.gap, 0.0);
}

#[test]
fn test_empty_list_len() {
    let l = list();
    assert_eq!(l.len(), 0);
    assert!(l.is_empty());
}

#[test]
fn test_single_item_list() {
    let l = list().item(text("Only item"));
    assert_eq!(l.len(), 1);
    assert!(!l.is_empty());
}
