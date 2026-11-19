//! Fonts an app registers, and installed fonts outside the built-in table,
//! are found by family name (#325).

use rui::renderer::text::{TextMeasureCache, TextRequest, register_font, register_font_file};

/// `-`, `>` and `0` of JetBrains Mono Regular (OFL-1.1, see fonts/OFL.txt),
/// with its `calt` and `liga` lookups.
const JETBRAINS_MONO: &[u8] = include_bytes!("fonts/JetBrainsMono-subset.ttf");

/// The advance of `0` at an 18 px em, and the families that fell back.
fn zero(family: &str, weight: u16) -> (f32, Vec<String>) {
    let mut cache = TextMeasureCache::new();
    let request = TextRequest::new("0", 18.0, weight, Some(family), 1.2);
    let metrics = cache.measure_single_line(request).expect("measure");
    (
        metrics.advance_width,
        cache.unknown_font_families().to_vec(),
    )
}

#[test]
fn a_font_registered_from_bytes_shapes_text_naming_its_family() {
    assert_eq!(
        register_font(JETBRAINS_MONO).expect("register"),
        ["JetBrains Mono"]
    );

    let mut cache = TextMeasureCache::new();
    let request = TextRequest::new("0", 18.0, 400, Some("JetBrains Mono"), 1.2);
    let plan = cache.shape_single_line(request).expect("shape");
    assert!(cache.unknown_font_families().is_empty());
    assert_eq!(plan.glyphs()[0].font_family, "JetBrains Mono");
    // 600 of 1000 units per em.
    assert!((plan.glyphs()[0].advance_width - 10.8).abs() < 0.001);
}

#[test]
fn a_font_registered_from_a_file_returns_its_families() {
    let path = std::env::temp_dir().join(format!("rui-register-{}.ttf", std::process::id()));
    std::fs::write(&path, JETBRAINS_MONO).expect("write font");
    let families = register_font_file(&path);
    std::fs::remove_file(&path).expect("remove font");
    assert_eq!(families.expect("register"), ["JetBrains Mono"]);

    assert!(register_font(b"not a font".to_vec()).is_err());
    assert!(register_font_file(std::env::temp_dir().join("rui-no-such-font.ttf")).is_err());
}

#[cfg(target_os = "macos")]
#[test]
fn installed_families_outside_the_table_are_found_by_name() {
    // Menlo.ttc: 1233 of 2048 units per em.
    let (advance, unknown) = zero("Menlo", 400);
    assert!(unknown.is_empty(), "{unknown:?}");
    assert!((advance - 10.837).abs() < 0.001, "{advance}");
    let (bold, unknown) = zero("Menlo Italic", 700);
    assert!(unknown.is_empty(), "{unknown:?}");
    assert!((bold - 10.837).abs() < 0.001, "{bold}");

    // A collection of 18 faces, past the table loader's 16.
    let (_, unknown) = zero("Apple SD Gothic Neo", 400);
    assert!(unknown.is_empty(), "{unknown:?}");

    let (_, unknown) = zero("No Such Family 325", 400);
    assert_eq!(unknown, ["No Such Family 325"]);
}
