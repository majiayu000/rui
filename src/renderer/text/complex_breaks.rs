//! Line breaks inside runs of scripts written without spaces between words:
//! Thai, Lao, Khmer and Myanmar (UAX #14 class SA). Their words are found in
//! a dictionary; text without them never reaches it.

use icu_segmenter::LineSegmenter;
use icu_segmenter::options::LineBreakOptions;

/// Whether `ch` is in a block of a script that breaks between dictionary words.
fn breaks_by_dictionary(ch: char) -> bool {
    matches!(
        ch,
        '\u{0E00}'..='\u{0EFF}' // Thai, Lao
            | '\u{1000}'..='\u{109F}' // Myanmar
            | '\u{1780}'..='\u{17FF}' // Khmer
            | '\u{19E0}'..='\u{19FF}' // Khmer symbols
            | '\u{A9E0}'..='\u{A9FF}' // Myanmar extended B
            | '\u{AA60}'..='\u{AA7F}' // Myanmar extended A
    )
}

/// Byte offsets of `text`, in order, where a line may break between two words
/// of one such run. Empty, and found without a dictionary, when `text` has none.
pub(super) fn dictionary_breaks(text: &str) -> Vec<usize> {
    let mut breaks = Vec::new();
    if text.is_ascii() || !text.chars().any(breaks_by_dictionary) {
        return breaks;
    }
    let segmenter = LineSegmenter::new_dictionary(LineBreakOptions::default());
    let mut chars = text.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        if !breaks_by_dictionary(ch) {
            continue;
        }
        let mut end = start + ch.len_utf8();
        while let Some(&(index, ch)) = chars.peek()
            && breaks_by_dictionary(ch)
        {
            end = index + ch.len_utf8();
            chars.next();
        }
        let run = &text[start..end];
        breaks.extend(
            segmenter
                .segment_str(run)
                .filter(|&offset| offset > 0 && offset < run.len())
                .map(|offset| start + offset),
        );
    }
    breaks
}

#[cfg(test)]
mod tests {
    use super::super::{TextMeasureCache, TextRequest};
    use super::*;

    /// `text` cut at `breaks`.
    fn pieces<'a>(text: &'a str, breaks: &[usize]) -> Vec<&'a str> {
        let ends = breaks.iter().copied().chain([text.len()]);
        let starts = [0].into_iter().chain(breaks.iter().copied());
        starts
            .zip(ends)
            .map(|(start, end)| &text[start..end])
            .collect()
    }

    #[test]
    fn thai_breaks_between_words_and_other_text_not_at_all() {
        let text = "ภาษาไทยไม่มีช่องว่าง";
        let words = pieces(text, &dictionary_breaks(text));
        assert_eq!(words, ["ภาษา", "ไทย", "ไม่มี", "ช่อง", "ว่าง"]);
        assert!(dictionary_breaks("layout 文本布局 مرحبا").is_empty());
    }

    #[test]
    fn a_long_thai_sentence_wraps_between_words() {
        let mut cache = TextMeasureCache::new();
        if !cache.has_fonts() {
            return;
        }
        let text = "การแสดงผลข้อความภาษาไทยไม่มีช่องว่างระหว่างคำจึงต้องตัดบรรทัดตามพจนานุกรม";
        let request = TextRequest::new(text, 20.0, 400, None, 1.2);
        let lines = cache.wrap(request, 120.0, None).expect("wrap").lines;
        let lines: Vec<&str> = lines.iter().map(|line| &*line.content).collect();
        assert!(lines.len() > 2, "{lines:?}");
        assert_eq!(lines.concat(), text);
        let words = pieces(text, &dictionary_breaks(text));
        let mut words = words.iter();
        for line in lines {
            // Each line is whole words.
            let mut rest = line;
            while !rest.is_empty() {
                let word = words.next().expect("a word");
                rest = rest
                    .strip_prefix(word)
                    .unwrap_or_else(|| panic!("{line} splits {word}"));
            }
        }
    }
}
