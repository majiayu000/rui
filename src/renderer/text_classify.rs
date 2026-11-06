//! Script and direction of grapheme clusters.

use crate::renderer::text_plan::{TextDirection, TextScript};
use crate::renderer::text_shaping::ClusterDraft;
use unicode_bidi::{BidiClass, bidi_class};

pub(crate) fn classify_script(cluster: &str) -> TextScript {
    if cluster.chars().all(char::is_whitespace) {
        return TextScript::Whitespace;
    }
    if cluster.chars().any(is_emoji) {
        return TextScript::Emoji;
    }
    if cluster.chars().any(is_cjk) {
        return TextScript::Cjk;
    }
    if cluster.chars().any(is_rtl) {
        return TextScript::Rtl;
    }
    if cluster.chars().all(char::is_numeric) {
        return TextScript::Number;
    }
    if cluster.chars().any(is_latin) {
        return TextScript::Latin;
    }
    TextScript::Other
}

pub(crate) fn classify_direction(cluster: &str) -> TextDirection {
    if cluster.chars().any(is_rtl) {
        TextDirection::RightToLeft
    } else if cluster.chars().any(is_ltr) {
        TextDirection::LeftToRight
    } else {
        TextDirection::Neutral
    }
}

pub(crate) fn resolve_text_direction(clusters: &[ClusterDraft]) -> TextDirection {
    let has_ltr = clusters
        .iter()
        .any(|cluster| cluster.direction == TextDirection::LeftToRight);
    let has_rtl = clusters
        .iter()
        .any(|cluster| cluster.direction == TextDirection::RightToLeft);
    match (has_ltr, has_rtl) {
        (true, true) => TextDirection::Mixed,
        (true, false) => TextDirection::LeftToRight,
        (false, true) => TextDirection::RightToLeft,
        (false, false) => TextDirection::Neutral,
    }
}

fn is_latin(ch: char) -> bool {
    matches!(ch as u32, 0x0041..=0x024f | 0x1e00..=0x1eff)
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3040..=0x30ff
            | 0x3400..=0x4dbf
            | 0x4e00..=0x9fff
            | 0xac00..=0xd7af
            | 0xf900..=0xfaff
    )
}

fn is_emoji(ch: char) -> bool {
    matches!(
        ch as u32,
        0x2600..=0x27bf | 0x1f1e6..=0x1f1ff | 0x1f300..=0x1faff
    )
}

fn is_ltr(ch: char) -> bool {
    ch.is_alphabetic() && bidi_class(ch) == BidiClass::L
}

fn is_rtl(ch: char) -> bool {
    ch.is_alphabetic() && matches!(bidi_class(ch), BidiClass::R | BidiClass::AL)
}

pub(crate) fn is_default_ignorable(ch: char) -> bool {
    matches!(
        ch as u32,
        0x00ad
            | 0x034f
            | 0x061c
            | 0x115f..=0x1160
            | 0x17b4..=0x17b5
            | 0x180b..=0x180f
            | 0x200b..=0x200f
            | 0x202a..=0x202e
            | 0x2060..=0x206f
            | 0xfe00..=0xfe0f
            | 0xfeff
            | 0xfff0..=0xfff8
            | 0xe0100..=0xe01ef
    )
}
