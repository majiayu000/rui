//! Where each character of a line sits, for platforms that ask a text field
//! for character bounds (AccessKit text runs).

use super::TextEditLayout;

impl TextEditLayout {
    /// The left edge and width of each character of line `line_index`, from
    /// the line's left edge. The characters are consecutive UTF-8 `lengths`
    /// from the line's start; any past the line's end (its newline) sit at the
    /// line's right edge with no width. A shaped cluster holding several
    /// characters (a ligature) is shared between them by byte length. Empty
    /// for a line the layout does not have.
    pub(crate) fn character_extents(&self, line_index: usize, lengths: &[u8]) -> Vec<(f32, f32)> {
        let Some(line) = self.lines.get(line_index) else {
            return Vec::new();
        };
        let mut clusters: Vec<_> = self
            .clusters
            .iter()
            .filter(|cluster| cluster.line_index == line_index)
            .collect();
        clusters.sort_by_key(|cluster| cluster.byte_start);
        let (line_start, line_end) = (line.range.start(), line.range.end());
        let mut extents = Vec::with_capacity(lengths.len());
        let mut start = line_start;
        let mut previous_end = 0.0;
        for length in lengths {
            let end = start + usize::from(*length);
            let cluster = clusters
                .iter()
                .find(|cluster| cluster.byte_start <= start && start < cluster.byte_end);
            let extent = match cluster {
                _ if start >= line_end => (line.size.width, 0.0),
                Some(cluster) => {
                    let bytes = (cluster.byte_end - cluster.byte_start) as f32;
                    let from = (start - cluster.byte_start) as f32 / bytes;
                    let to = (end.min(cluster.byte_end) - cluster.byte_start) as f32 / bytes;
                    let left = cluster
                        .x_offset
                        .min(cluster.x_offset + cluster.advance_width);
                    let width = cluster.advance_width.abs();
                    // Laid out right to left, the cluster's first byte is on the right.
                    let offset = if cluster.rtl { 1.0 - to } else { from };
                    (left + width * offset, width * (to - from))
                }
                None => (previous_end, 0.0),
            };
            previous_end = extent.0 + extent.1;
            extents.push(extent);
            start = end;
        }
        extents
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_character_of_a_line_has_its_left_edge_and_width() {
        let layout = TextEditLayout::new("ab\ncd", 10.0, 20.0);
        assert_eq!(
            layout.character_extents(0, &[1, 1, 1]),
            [(0.0, 10.0), (10.0, 10.0), (20.0, 0.0)],
            "the newline sits at the line's end"
        );
        assert_eq!(
            layout.character_extents(1, &[1, 1]),
            [(0.0, 10.0), (10.0, 10.0)]
        );
        assert!(layout.character_extents(2, &[1]).is_empty());
    }

    #[test]
    fn multibyte_characters_follow_their_clusters() {
        let layout = TextEditLayout::new("中é", 12.0, 20.0);
        assert_eq!(
            layout.character_extents(0, &[3, 2]),
            [(0.0, 12.0), (12.0, 12.0)]
        );
    }
}
