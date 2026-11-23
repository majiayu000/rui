//! The shaper behind `shape_run_glyphs`: rustybuzz, or HarfRust with the
//! `fontations` feature (issue #369 spike).

use super::{ClusterDraft, TextShapingFont};
use crate::renderer::text::FontFeature;
use crate::renderer::text_fallback::RunScript;
use rustybuzz::{BufferClusterLevel, Direction as HbDirection, UnicodeBuffer};

#[cfg(feature = "fontations")]
pub(super) use harfrust_glyphs as shape_glyphs;
#[cfg(not(feature = "fontations"))]
pub(super) use rustybuzz_glyphs as shape_glyphs;

/// `(glyph, cluster, x_advance, x_offset, y_offset)` in font units.
#[cfg_attr(feature = "fontations", allow(dead_code))]
pub(crate) fn rustybuzz_glyphs(
    font: &TextShapingFont,
    features: &[FontFeature],
    clusters: &[ClusterDraft],
    script: RunScript,
    rtl: bool,
) -> Vec<(u32, u32, i32, i32, i32)> {
    let mut buffer = UnicodeBuffer::new();
    buffer.set_cluster_level(BufferClusterLevel::MonotoneGraphemes);
    buffer.set_direction(if rtl {
        HbDirection::RightToLeft
    } else {
        HbDirection::LeftToRight
    });
    if let Some(script) = script.script() {
        buffer.set_script(script);
    }
    for cluster in clusters {
        for ch in cluster.text.chars().filter(|ch| !ch.is_control()) {
            buffer.add(ch, cluster.byte_start as u32);
        }
    }

    buffer.guess_segment_properties();
    let plan = font.plan_for(&buffer, features);
    let shaped = rustybuzz::shape_with_plan(font.face(), &plan, buffer);
    shaped
        .glyph_infos()
        .iter()
        .zip(shaped.glyph_positions())
        .map(|(info, pos)| {
            (
                info.glyph_id,
                info.cluster,
                pos.x_advance,
                pos.x_offset,
                pos.y_offset,
            )
        })
        .collect()
}

#[cfg(feature = "fontations")]
pub(crate) fn harfrust_glyphs(
    font: &TextShapingFont,
    features: &[FontFeature],
    clusters: &[ClusterDraft],
    script: RunScript,
    rtl: bool,
) -> Vec<(u32, u32, i32, i32, i32)> {
    let chars = clusters.iter().flat_map(|cluster| {
        cluster
            .text
            .chars()
            .filter(|ch| !ch.is_control())
            .map(move |ch| (ch, cluster.byte_start as u32))
    });
    let shaped = font
        .fontations
        .shape(chars, script.script().map(|_| script.tag()), rtl, features);
    #[cfg(test)]
    {
        let reference = rustybuzz_glyphs(font, features, clusters, script, rtl);
        if reference != shaped {
            let text: String = clusters.iter().map(|cluster| cluster.text).collect();
            eprintln!(
                "SHAPE DIFF {} {text:?}\n  rustybuzz {reference:?}\n  harfrust  {shaped:?}",
                font.family
            );
            SHAPE_DIFFS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    shaped
}

#[cfg(all(test, feature = "fontations"))]
pub(crate) static SHAPE_DIFFS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
