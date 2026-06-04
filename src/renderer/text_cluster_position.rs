use crate::renderer::text_shaping::{ClusterDraft, ShapedGlyph};

pub(crate) fn apply_exact_cluster_positions(
    clusters: &mut [ClusterDraft],
    glyphs: &[ShapedGlyph],
) -> bool {
    if clusters.is_empty() {
        return true;
    }

    let mut positions = Vec::with_capacity(clusters.len());
    for cluster in clusters.iter() {
        let matching = glyphs
            .iter()
            .filter(|glyph| {
                glyph.byte_start >= cluster.byte_start && glyph.byte_start < cluster.byte_end
            })
            .collect::<Vec<_>>();
        if matching.is_empty() {
            return false;
        }

        let start = matching
            .iter()
            .map(|glyph| glyph.x_offset)
            .fold(f32::INFINITY, f32::min);
        let end = matching
            .iter()
            .map(|glyph| glyph.x_offset + glyph.advance_width)
            .fold(f32::NEG_INFINITY, f32::max);
        positions.push((start, (end - start).max(0.0)));
    }

    for (cluster, (x_offset, advance_width)) in clusters.iter_mut().zip(positions) {
        cluster.x_offset = x_offset;
        cluster.advance_width = advance_width;
    }

    true
}
