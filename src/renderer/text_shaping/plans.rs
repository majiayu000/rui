//! Shaping plans of a face, one per direction, script, language and feature
//! list shaped.

use super::TextShapingFont;
use crate::renderer::text::{FontFeature, rustybuzz_features};
use rustybuzz::{Direction as HbDirection, Language, Script, ShapePlan, UnicodeBuffer};
use std::sync::{Arc, PoisonError};

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
        if let Some((.., plan)) = plans.iter().find(|(d, s, l, f, _)| {
            *d == direction && *s == script && *l == language && f == features
        }) {
            return Arc::clone(plan);
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
        plans.push(entry);
        plan
    }
}
