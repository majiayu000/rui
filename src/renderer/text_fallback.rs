//! Splitting text into script runs and choosing a font for each run.
//!
//! Each grapheme cluster gets the Unicode script of its first character that has
//! one; rustybuzz carries the script table and reports it through
//! [`UnicodeBuffer::guess_segment_properties`]. Characters every script shares
//! (spaces, punctuation, digits) join the run before them, or the run after them
//! at the start of the text, so a run is only cut where the script changes.
//! Emoji are a run of their own. Each cluster takes the first font of its run's
//! fallback chain that has it: the requested font, then the fonts preferred for
//! the script, then every other loaded font; emoji try the emoji fonts first.
//! A cluster no font has stays in the requested font and shapes to `.notdef`,
//! which the plan reports as a missing glyph.

use crate::renderer::text_shaping::{ClusterDraft, TextScript, TextShapingFont, faces_for_weight};
use rustybuzz::{Direction, Script, UnicodeBuffer, script};
use std::ops::Range;

/// What a run is shaped as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunScript {
    /// A Unicode script and whether it is written right to left.
    Script {
        script: Script,
        rtl: bool,
    },
    Emoji,
    /// Only characters every script shares.
    Common,
}

impl RunScript {
    /// ISO 15924 code: the script's own, `Zsye` for emoji, `Zyyy` for common.
    pub(crate) fn tag(self) -> [u8; 4] {
        match self {
            Self::Script { script, .. } => script.tag().to_bytes(),
            Self::Emoji => *b"Zsye",
            Self::Common => *b"Zyyy",
        }
    }

    pub(crate) fn script(self) -> Option<Script> {
        match self {
            Self::Script { script, .. } => Some(script),
            Self::Emoji | Self::Common => None,
        }
    }

    pub(crate) fn is_rtl(self) -> bool {
        matches!(self, Self::Script { rtl: true, .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScriptRun {
    pub(crate) clusters: Range<usize>,
    pub(crate) script: RunScript,
}

fn cluster_script(buffer: &mut UnicodeBuffer, cluster: &ClusterDraft) -> RunScript {
    if cluster.script == TextScript::Emoji {
        return RunScript::Emoji;
    }
    buffer.clear();
    buffer.push_str(cluster.text);
    buffer.guess_segment_properties();
    match buffer.script() {
        script::COMMON | script::INHERITED | script::UNKNOWN => RunScript::Common,
        script => RunScript::Script {
            script,
            rtl: buffer.direction() == Direction::RightToLeft,
        },
    }
}

/// Whether a shared character may join a run of `neighbour`. Digits stay out of
/// right-to-left runs: they read left to right and are shaped that way.
fn joins(cluster: &ClusterDraft, neighbour: RunScript) -> bool {
    match neighbour {
        RunScript::Script { rtl, .. } => !(rtl && cluster.script == TextScript::Number),
        RunScript::Emoji | RunScript::Common => false,
    }
}

/// Split `clusters` into maximal runs of one script.
pub(crate) fn script_runs(clusters: &[ClusterDraft]) -> Vec<ScriptRun> {
    let mut buffer = UnicodeBuffer::new();
    let mut scripts: Vec<RunScript> = clusters
        .iter()
        .map(|cluster| cluster_script(&mut buffer, cluster))
        .collect();

    let mut previous = None;
    for (index, cluster) in clusters.iter().enumerate() {
        match scripts[index] {
            RunScript::Common => {
                if let Some(script) = previous.filter(|&script| joins(cluster, script)) {
                    scripts[index] = script;
                } else {
                    previous = None;
                }
            }
            RunScript::Emoji => previous = None,
            script => previous = Some(script),
        }
    }
    let mut next = None;
    for (index, cluster) in clusters.iter().enumerate().rev() {
        match scripts[index] {
            RunScript::Common => {
                if let Some(script) = next.filter(|&script| joins(cluster, script)) {
                    scripts[index] = script;
                } else {
                    next = None;
                }
            }
            RunScript::Emoji => next = None,
            script => next = Some(script),
        }
    }

    let mut runs: Vec<ScriptRun> = Vec::new();
    for (index, script) in scripts.into_iter().enumerate() {
        match runs.last_mut() {
            Some(run) if run.script == script => run.clusters.end = index + 1,
            _ => runs.push(ScriptRun {
                clusters: index..index + 1,
                script,
            }),
        }
    }
    runs
}

/// Families preferred for a script, best first, ahead of the other loaded fonts.
fn preferred_families(script: RunScript) -> &'static [&'static str] {
    const CJK: &[&str] = &[
        "Hiragino Sans GB",
        "STHeiti",
        "Noto Sans CJK",
        "Arial Unicode",
    ];
    match script {
        RunScript::Emoji => &["Apple Color Emoji", "Noto Color Emoji", "Apple Symbols"],
        RunScript::Common => &[],
        RunScript::Script { script, .. } => match &script.tag().to_bytes() {
            b"Hani" | b"Hira" | b"Kana" | b"Bopo" | b"Hang" => CJK,
            b"Arab" => &[
                "SF Arabic",
                "Geeza Pro",
                "Noto Sans Arabic",
                "Arial Unicode",
                "DejaVu Sans",
            ],
            b"Hebr" => &["SF Hebrew", "Arial", "Arial Unicode", "DejaVu Sans"],
            _ => &[],
        },
    }
}

/// Font indices in the order a run of `script` tries them. Emoji try the
/// emoji fonts before the requested one, since most text fonts carry plain
/// glyphs for some emoji characters. A preferred family's faces come closest
/// to `weight` first.
pub(crate) fn fallback_chain(
    fonts: &[TextShapingFont],
    primary_index: usize,
    script: RunScript,
    weight: u16,
) -> Vec<usize> {
    let preferred: Vec<usize> = preferred_families(script)
        .iter()
        .flat_map(|family| faces_for_weight(fonts, family, weight))
        .collect();
    let first: Vec<usize> = if script == RunScript::Emoji {
        preferred.into_iter().chain([primary_index]).collect()
    } else {
        [primary_index].into_iter().chain(preferred).collect()
    };
    let mut chain = Vec::with_capacity(fonts.len());
    for index in first.into_iter().chain(0..fonts.len()) {
        if !chain.contains(&index) {
            chain.push(index);
        }
    }
    chain
}

/// Set the font of every cluster in `clusters`, one run of `script`: the first
/// font of the run's chain that has the cluster, except that whitespace stays in
/// the font before it when that font has it, so a fallback run is not cut at
/// every space.
pub(crate) fn assign_fonts(
    fonts: &[TextShapingFont],
    primary_index: usize,
    script: RunScript,
    weight: u16,
    clusters: &mut [ClusterDraft],
) {
    let mut chain: Option<Vec<usize>> = None;
    let mut previous: Option<usize> = None;
    for cluster in clusters {
        let supports = |font: usize| fonts[font].supports_cluster(cluster.text);
        let font = if let Some(font) =
            previous.filter(|&font| cluster.script == TextScript::Whitespace && supports(font))
        {
            font
        } else if script != RunScript::Emoji && supports(primary_index) {
            primary_index
        } else {
            chain
                .get_or_insert_with(|| fallback_chain(fonts, primary_index, script, weight))
                .iter()
                .copied()
                .find(|&font| supports(font))
                .unwrap_or(primary_index)
        };
        cluster.font_index = font;
        previous = Some(font);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::text::TextRequest;
    use crate::renderer::text_shaping::build_cluster_drafts;

    fn runs(content: &str) -> Vec<(String, String)> {
        let request = TextRequest::new(content, 14.0, 400, None, 1.2);
        let clusters = build_cluster_drafts(0, request);
        script_runs(&clusters)
            .into_iter()
            .map(|run| {
                let start = clusters[run.clusters.start].byte_start;
                let end = clusters[run.clusters.end - 1].byte_end;
                (
                    String::from_utf8_lossy(&run.script.tag()).into_owned(),
                    content[start..end].to_string(),
                )
            })
            .collect()
    }

    fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
        expected
            .iter()
            .map(|(tag, text)| (tag.to_string(), text.to_string()))
            .collect()
    }

    #[test]
    fn runs_change_only_where_the_script_does() {
        assert_eq!(
            runs("Hello, 世界! مرحبا بالعالم 123 👋🏽 ok"),
            pairs(&[
                ("Latn", "Hello, "),
                ("Hani", "世界! "),
                ("Arab", "مرحبا بالعالم "),
                ("Zyyy", "123 "),
                ("Zsye", "👋🏽"),
                ("Latn", " ok"),
            ])
        );
    }

    #[test]
    fn leading_shared_characters_join_the_run_after_them() {
        assert_eq!(runs("  (hi)"), pairs(&[("Latn", "  (hi)")]));
        assert_eq!(runs("12 ab"), pairs(&[("Latn", "12 ab")]));
        assert_eq!(runs("..."), pairs(&[("Zyyy", "...")]));
    }

    #[test]
    fn combining_marks_and_kana_stay_in_their_runs() {
        assert_eq!(runs("cafe\u{301}"), pairs(&[("Latn", "cafe\u{301}")]));
        assert_eq!(
            runs("カタカナ ひらがな"),
            pairs(&[("Kana", "カタカナ "), ("Hira", "ひらがな")])
        );
    }

    #[test]
    fn digits_do_not_join_right_to_left_runs() {
        assert_eq!(
            runs("שלום 42 עולם"),
            pairs(&[("Hebr", "שלום "), ("Zyyy", "42"), ("Hebr", " עולם")])
        );
    }
}
