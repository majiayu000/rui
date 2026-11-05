use crate::renderer::text_shaping::{ClusterDraft, GlyphDraft};

pub(crate) fn apply_exact_cluster_positions(
    clusters: &mut [ClusterDraft],
    glyphs: &[GlyphDraft],
) -> bool {
    if clusters.is_empty() {
        return true;
    }

    // Each glyph's byte start and horizontal extent, ordered by byte start so
    // the glyphs of a cluster are one binary-searched range.
    let mut extents: Vec<(usize, f32, f32)> = glyphs
        .iter()
        .map(|glyph| {
            (
                glyph.byte_start,
                glyph.x_offset,
                glyph.x_offset + glyph.advance_width,
            )
        })
        .collect();
    extents.sort_by_key(|&(byte_start, ..)| byte_start);

    let mut positions = Vec::with_capacity(clusters.len());
    for (index, cluster) in clusters.iter().enumerate() {
        let first = extents.partition_point(|&(start, ..)| start < cluster.byte_start);
        let last = extents.partition_point(|&(start, ..)| start < cluster.byte_end);
        let matching = &extents[first..last];
        if matching.is_empty() {
            return false;
        }

        let start = matching
            .iter()
            .map(|&(_, start, _)| start)
            .fold(f32::INFINITY, f32::min);
        let end = matching
            .iter()
            .map(|&(.., end)| end)
            .fold(f32::NEG_INFINITY, f32::max);
        positions.push(ClusterPosition { index, start, end });
    }

    let mut visual_order = positions.iter().collect::<Vec<_>>();
    visual_order.sort_by(|left, right| left.start.total_cmp(&right.start));

    let mut resolved = vec![(0.0, 0.0); clusters.len()];
    for (visual_index, position) in visual_order.iter().enumerate() {
        let end = visual_order
            .get(visual_index + 1)
            .map(|next| next.start)
            .unwrap_or(position.end);
        resolved[position.index] = (position.start, (end - position.start).max(0.0));
    }

    for (cluster, (x_offset, advance_width)) in clusters.iter_mut().zip(resolved) {
        cluster.x_offset = x_offset;
        cluster.advance_width = advance_width;
    }

    true
}

struct ClusterPosition {
    index: usize,
    start: f32,
    end: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::text_shaping::{TextDirection, TextScript};

    #[test]
    fn exact_cluster_positions_use_neighbor_starts_for_kerned_glyphs() {
        let mut clusters = vec![cluster(0, 1, "A"), cluster(1, 2, "V")];
        let glyphs = vec![glyph(0, 1, 0.0, 10.0), glyph(1, 2, 8.0, 9.0)];

        assert!(apply_exact_cluster_positions(&mut clusters, &glyphs));

        assert_eq!(clusters[0].x_offset, 0.0);
        assert_eq!(clusters[0].advance_width, 8.0);
        assert_eq!(clusters[1].x_offset, 8.0);
        assert_eq!(clusters[1].advance_width, 9.0);
    }

    fn cluster(byte_start: usize, byte_end: usize, text: &str) -> ClusterDraft<'_> {
        ClusterDraft {
            byte_start,
            byte_end,
            text,
            script: TextScript::Latin,
            direction: TextDirection::LeftToRight,
            font_index: 0,
            x_offset: 0.0,
            advance_width: 0.0,
        }
    }

    fn glyph(byte_start: usize, byte_end: usize, x_offset: f32, advance_width: f32) -> GlyphDraft {
        GlyphDraft {
            byte_start,
            byte_end,
            glyph_id: byte_start as u16,
            x_offset,
            y_offset: 0.0,
            advance_width,
            font_index: 0,
        }
    }
}
