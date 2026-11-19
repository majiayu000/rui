//! A text input's value as AccessKit text runs, one per line, so AT-SPI
//! exposes it through its Text and EditableText interfaces and the caret and
//! selection have positions to point at. With the input's layout the runs
//! also carry where each character is.

use crate::core::accessibility::AccessibilityNode;
use crate::core::text_editing::TextInputGeometry;
use accesskit::{Node, NodeId, Rect, Role, TextDirection, TextPosition, TextSelection};
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
/// there is one, otherwise the caret. `geometry` is the input's layout, when
/// it has one for this value: each run then has bounds (logical, like the
/// element's) and each character a position and width in it.
pub(super) fn text_runs(
    node: &AccessibilityNode,
    geometry: Option<&TextInputGeometry>,
) -> (Vec<(NodeId, Node)>, Option<TextSelection>) {
    let value = node.a11y_value().unwrap_or_default();
    let runs = lines(node.a11y_id().0, value);
    let geometry = geometry.filter(|geometry| {
        geometry.layout().text() == value && geometry.layout().lines().len() == runs.len()
    });
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
        .enumerate()
        .map(|(index, run)| {
            let mut out = Node::new(Role::TextRun);
            if let Some(geometry) = geometry {
                place_characters(&mut out, geometry, index, &run.lengths);
            }
            out.set_value(run.text);
            out.set_character_lengths(run.lengths);
            (run.id, out)
        })
        .collect();
    (nodes, selection)
}

/// Give run `line` its line's bounds and each character its left edge and
/// width from the run's left edge.
fn place_characters(out: &mut Node, geometry: &TextInputGeometry, line: usize, lengths: &[u8]) {
    let layout = geometry.layout();
    let text_line = layout.lines()[line];
    let extents = layout.character_extents(line, lengths);
    let right = extents
        .iter()
        .map(|(left, width)| left + width)
        .fold(text_line.measured_size().width, f32::max);
    let x = geometry.origin().x + text_line.origin().x;
    let y = geometry.origin().y + text_line.origin().y;
    out.set_bounds(Rect::new(
        f64::from(x),
        f64::from(y),
        f64::from(x + right),
        f64::from(y + text_line.measured_size().height),
    ));
    // Each position is a left edge, right-to-left characters included.
    out.set_text_direction(TextDirection::LeftToRight);
    out.set_character_positions(extents.iter().map(|(left, _)| *left).collect::<Vec<_>>());
    out.set_character_widths(extents.iter().map(|(_, width)| *width).collect::<Vec<_>>());
}

/// The byte offset in `node`'s value of `position` on one of its runs, or
/// None when the position names no run of it or lies past the run's end.
pub(super) fn offset(node: &AccessibilityNode, position: TextPosition) -> Option<usize> {
    let runs = lines(node.a11y_id().0, node.a11y_value().unwrap_or_default());
    let run = runs.iter().find(|run| run.id == position.node)?;
    let within = run.lengths.get(..position.character_index)?;
    Some(
        run.start
            + within
                .iter()
                .map(|length| usize::from(*length))
                .sum::<usize>(),
    )
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
    fn run_positions_map_back_to_byte_offsets() {
        use crate::core::ElementId;
        use crate::core::accessibility::AccessibilityRole;
        let value = "ab\n中é";
        let node =
            AccessibilityNode::new(ElementId(1), AccessibilityRole::TextInput).with_value(value);
        let runs = lines(1, value);
        for expected in [0, 2, 3, 6, value.len()] {
            assert_eq!(offset(&node, position(&runs, expected)), Some(expected));
        }
        let past = TextPosition {
            node: runs[0].id,
            character_index: 4,
        };
        assert_eq!(offset(&node, past), None, "past the run's end");
        let elsewhere = TextPosition {
            node: NodeId(7),
            character_index: 0,
        };
        assert_eq!(offset(&node, elsewhere), None, "another node's position");
    }

    #[test]
    fn runs_of_different_inputs_do_not_share_ids() {
        let first: Vec<_> = lines(10, "a\nb").iter().map(|run| run.id).collect();
        let second: Vec<_> = lines(11, "a\nb").iter().map(|run| run.id).collect();
        assert!(first.iter().all(|id| !second.contains(id)));
    }
}
