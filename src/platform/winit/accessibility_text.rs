//! A text input's value as AccessKit text runs, one per line, so AT-SPI
//! exposes it through its Text and EditableText interfaces and the caret and
//! selection have positions to point at.

use crate::core::accessibility::AccessibilityNode;
use accesskit::{Node, NodeId, Role, TextPosition, TextSelection};
use unicode_segmentation::UnicodeSegmentation;

/// Mixed into an input's id before hashing it, so its runs' ids are spread
/// far from the element ids around them.
const RUN_ID: u64 = 0x5255_495f_5255_4e00;

/// The id of line `line` of the input `id`: a hash of the input's id plus the
/// line, which no element and no other input's run is expected to share.
fn run_id(id: u64, line: usize) -> NodeId {
    // splitmix64.
    let mut z = (id ^ RUN_ID).wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    NodeId((z ^ (z >> 31)).wrapping_add(line as u64))
}

/// One line of the value: its node id, where it starts in the value, and the
/// UTF-8 lengths of its grapheme clusters (split if oversized), including the newline.
struct Run {
    id: NodeId,
    start: usize,
    text: String,
    lengths: Vec<u8>,
}

/// The runs of a text input and the selection over them: the selection when
/// there is one, otherwise the caret.
pub(super) fn text_runs(node: &AccessibilityNode) -> (Vec<(NodeId, Node)>, Option<TextSelection>) {
    let value = node.a11y_value().unwrap_or_default();
    let runs = lines(node.a11y_id().0, value);
    let range = node
        .a11y_text_selection()
        .map(|range| (range.start(), range.end()))
        .or_else(|| node.a11y_text_caret().map(|caret| (caret, caret)));
    let selection = range.map(|(anchor, focus)| TextSelection {
        anchor: position(&runs, anchor),
        focus: position(&runs, focus),
    });
    let nodes = runs
        .into_iter()
        .map(|run| {
            let mut out = Node::new(Role::TextRun);
            out.set_value(run.text);
            out.set_character_lengths(run.lengths);
            (run.id, out)
        })
        .collect();
    (nodes, selection)
}

fn lines(id: u64, value: &str) -> Vec<Run> {
    let mut runs = Vec::new();
    let mut start = 0;
    for line in value.split_inclusive('\n') {
        runs.push(Run {
            id: run_id(id, runs.len()),
            start,
            text: line.to_string(),
            lengths: character_lengths(line),
        });
        start += line.len();
    }
    // An empty value, or one ending in a newline, still has a line to put
    // the caret on.
    if value.is_empty() || value.ends_with('\n') {
        runs.push(Run {
            id: run_id(id, runs.len()),
            start,
            text: String::new(),
            lengths: Vec::new(),
        });
    }
    runs
}

fn character_lengths(line: &str) -> Vec<u8> {
    let limit = usize::from(u8::MAX);
    let mut lengths = Vec::new();
    for mut cluster in line.graphemes(true) {
        // AccessKit stores the lengths in bytes as u8 and sums them to slice
        // the text, so oversized graphemes have to yield aligned chunks.
        while cluster.len() > limit {
            let mut boundary = limit;
            while !cluster.is_char_boundary(boundary) {
                boundary -= 1;
            }
            lengths.push(boundary as u8);
            cluster = &cluster[boundary..];
        }
        lengths.push(cluster.len() as u8);
    }
    lengths
}

/// The run and cluster index at UTF-8 offset `offset` of the value. An offset
/// at a line's end is the start of the next line, except on the last one.
fn position(runs: &[Run], offset: usize) -> TextPosition {
    let index = runs
        .iter()
        .rposition(|run| run.start <= offset)
        .unwrap_or_default();
    let run = &runs[index];
    let mut consumed = run.start;
    let mut character_index = 0;
    for length in &run.lengths {
        if consumed >= offset {
            break;
        }
        consumed += usize::from(*length);
        character_index += 1;
    }
    TextPosition {
        node: run.id,
        character_index,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offsets(value: &str, offset: usize) -> (usize, usize) {
        let runs = lines(1, value);
        let position = position(&runs, offset);
        let line = runs
            .iter()
            .position(|run| run.id == position.node)
            .expect("the position is on a run");
        (line, position.character_index)
    }

    #[test]
    fn lines_keep_their_newline_and_count_clusters() {
        let runs = lines(1, "ab\n中é\n");
        let texts: Vec<_> = runs.iter().map(|run| run.text.as_str()).collect();
        assert_eq!(texts, ["ab\n", "中é\n", ""]);
        assert_eq!(runs[1].lengths, [3, 2, 1]);
        assert_eq!(runs[1].start, 3);
        assert_eq!(lines(1, "").len(), 1, "an empty value has one empty line");
    }

    #[test]
    fn byte_offsets_become_line_and_cluster_positions() {
        let value = "ab\n中é";
        assert_eq!(offsets(value, 0), (0, 0));
        assert_eq!(offsets(value, 2), (0, 2));
        assert_eq!(offsets(value, 3), (1, 0), "after the newline");
        assert_eq!(offsets(value, 6), (1, 1), "after the CJK character");
        assert_eq!(offsets(value, value.len()), (1, 2), "the end");
    }

    #[test]
    fn runs_of_different_inputs_do_not_share_ids() {
        let first: Vec<_> = lines(10, "a\nb").iter().map(|run| run.id).collect();
        let second: Vec<_> = lines(11, "a\nb").iter().map(|run| run.id).collect();
        assert!(first.iter().all(|id| !second.contains(id)));
    }
}
