//! Recently used shaping plans of a face.

use super::TextShapingFont;
use crate::renderer::text::{FontFeature, rustybuzz_features};
use rustybuzz::{Direction as HbDirection, Language, Script, ShapePlan, UnicodeBuffer};
use std::sync::{Arc, PoisonError};

// Feature lists are application supplied, so keep a bounded working set per face.
const MAX_CACHED_PLANS: usize = 32;

/// A shaping plan and the buffer properties and features it was built for.
pub(super) type CachedPlan = (
    HbDirection,
    Script,
    Option<Language>,
    Vec<FontFeature>,
    Arc<ShapePlan>,
);

impl TextShapingFont {
    /// The plan for `buffer`'s direction, script and language with `features`,
    /// built on first use.
    pub(super) fn plan_for(
        &self,
        buffer: &UnicodeBuffer,
        features: &[FontFeature],
    ) -> Arc<ShapePlan> {
        let (direction, script, language) =
            (buffer.direction(), buffer.script(), buffer.language());
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
            &self.face,
            direction,
            Some(script),
            language.as_ref(),
            &rustybuzz_features(features),
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
        let mut buffer = UnicodeBuffer::new();
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
