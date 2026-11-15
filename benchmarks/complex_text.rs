//! The `text_complex` fixture's content, the same on every side of the comparison:
//! RUI's `benches/compare_frame.rs` and `benchmarks/egui_compare` include this file.
//!
//! A paragraph is built like `text_paragraphs`' (8 to 127 words), from words of seven
//! kinds: Latin, Simplified Chinese, Arabic (right to left, joined), Devanagari
//! (conjuncts), Thai (no spaces between its words), emoji (a ZWJ sequence, a flag
//! and a skin tone) and Arabic with digits and brackets, a right-to-left run whose
//! numbers read left to right. Words are taken in turn from `COMPLEX_WORDS`, row by
//! row, and a paragraph takes every third, so any seven words in a row have one of
//! each kind. Every paragraph starts with a Latin word, so reads left to right.

/// One row per variant, one column per kind.
const COMPLEX_WORDS: [[&str; 7]; 4] = [
    [
        "layout",
        "文本布局",
        "مرحبا",
        "क्षत्रिय",
        "ภาษาไทยไม่มีช่องว่าง",
        "👨\u{200d}👩\u{200d}👧\u{200d}👦",
        "(العدد 2024)",
    ],
    [
        "glyph",
        "渲染框架",
        "بالعالم",
        "विद्यालय",
        "สวัสดีครับ",
        "🇯🇵",
        "(صفحة 17)",
    ],
    [
        "paragraph",
        "字形缓存",
        "النص",
        "संस्कृत",
        "การแสดงผลข้อความ",
        "👍🏽",
        "(سطر 3)",
    ],
    [
        "wrapping",
        "段落换行",
        "العربي",
        "हिन्दी",
        "ตัดบรรทัดภาษาไทย",
        "😀",
        "(رقم 42)",
    ],
];

/// A word of each kind in turn: `index % 7` is the kind.
fn complex_word(index: usize) -> &'static str {
    let index = index % (COMPLEX_WORDS.len() * 7);
    COMPLEX_WORDS[index / 7][index % 7]
}

/// Paragraph `index` of `text_complex`: 8 to 127 words, the same on every side.
pub fn complex_paragraph(index: usize) -> String {
    let words = 8 + (index * 37) % 120;
    (0..words)
        .map(|word| complex_word(index * 7 + word * 3))
        .collect::<Vec<_>>()
        .join(" ")
}
