//! Shaping a run with HarfRust, and the recently used shaping plans of a face.

use super::{ClusterDraft, TextShapingFont};
use crate::renderer::text::{FontFeature, harfrust_features};
use crate::renderer::text_fallback::RunScript;
use harfrust::{
    Buffer, ClusterLevel, Direction, Language, Script, ShapeOptions, ShapePlan, ShaperFont,
};
use std::sync::{Arc, PoisonError};

// Feature lists are application supplied, so keep a bounded working set per face.
const MAX_CACHED_PLANS: usize = 32;

/// A shaping plan and the buffer properties and features it was built for.
pub(super) type CachedPlan = (
    Direction,
    Option<Script>,
    Option<Language>,
    Vec<FontFeature>,
    Arc<ShapePlan>,
);

impl TextShapingFont {
    /// `clusters` shaped with `features`: each glyph's id, cluster (byte
    /// offset), advance and offsets, in font units.
    pub(super) fn shape(
        &self,
        features: &[FontFeature],
        clusters: &[ClusterDraft],
        script: RunScript,
        rtl: bool,
    ) -> Vec<(u32, u32, i32, i32, i32)> {
        let mut buffer = Buffer::new();
        buffer.set_cluster_level(ClusterLevel::MonotoneGraphemes);
        buffer.set_direction(if rtl {
            Direction::RightToLeft
        } else {
            Direction::LeftToRight
        });
        if let Some(script) = script.script() {
            buffer.set_script(Some(script));
        }
        for cluster in clusters {
            for ch in cluster.text.chars().filter(|ch| !ch.is_control()) {
                buffer.push(ch as u32, cluster.byte_start as u32);
            }
        }
        buffer.guess_segment_properties();
        let plan = self.plan_for(&buffer, features);
        let features = harfrust_features(features);
        // A plan made for the buffer's own direction and script always applies.
        let _ = harfrust::shape(
            &ShaperFont::new(self.font()),
            &mut buffer,
            ShapeOptions::new().plan(Some(&plan)).features(&features),
        );
        buffer
            .glyph_infos()
            .iter()
            .zip(buffer.glyph_positions())
            .map(|(info, position)| {
                (
                    info.glyph_id,
                    info.cluster,
                    position.x_advance,
                    position.x_offset,
                    position.y_offset,
                )
            })
            .collect()
    }

    /// The plan for `buffer`'s direction, script and language with `features`,
    /// built on first use.
    fn plan_for(&self, buffer: &Buffer, features: &[FontFeature]) -> Arc<ShapePlan> {
        let (direction, script, language) = (
            buffer.direction(),
            buffer.script(),
            buffer.language().cloned(),
        );
        let mut plans = self.plans.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(index) = plans.iter().rposition(|(d, s, l, f, _)| {
            *d == direction && *s == script && *l == language && f == features
        }) {
            let entry = plans
                .remove(index)
                .expect("the matching plan is in the cache");
            let plan = Arc::clone(&entry.4);
            plans.push_back(entry);
            return plan;
        }
        let plan = Arc::new(ShapePlan::new(
            self.font(),
            direction,
            script,
            language.as_ref(),
            &harfrust_features(features),
        ));
        let entry = (
            direction,
            script,
            language,
            features.to_vec(),
            Arc::clone(&plan),
        );
        if plans.len() == MAX_CACHED_PLANS {
            plans.pop_front();
        }
        plans.push_back(entry);
        plan
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::text_shaping::FontBytes;

    #[test]
    fn many_feature_lists_keep_the_most_recent_plans_only() {
        let bytes = include_bytes!("../../../tests/fonts/JetBrainsMono-subset.ttf");
        let font = TextShapingFont::new("JetBrains Mono", 0, Arc::new(FontBytes::Static(bytes)))
            .expect("fixture face");
        let mut buffer = Buffer::new();
        buffer.push_str("->");
        buffer.guess_segment_properties();
        let default = font.plan_for(&buffer, &[]);
        let settings = |value| [FontFeature::new(*b"calt", value)];
        let oldest = font.plan_for(&buffer, &settings(0));

        for value in 1..=MAX_CACHED_PLANS as u32 {
            font.plan_for(&buffer, &settings(value));
            assert!(Arc::ptr_eq(&font.plan_for(&buffer, &[]), &default));
        }

        assert_eq!(font.plans.lock().expect("cache").len(), MAX_CACHED_PLANS);
        assert!(!Arc::ptr_eq(&font.plan_for(&buffer, &settings(0)), &oldest));
        assert!(Arc::ptr_eq(&font.plan_for(&buffer, &[]), &default));
    }
}
