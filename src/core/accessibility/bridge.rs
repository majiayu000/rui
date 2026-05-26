use super::{AccessibilityAnnouncement, AccessibilityError, AccessibilityTree};

pub trait AccessibilityBridge {
    fn publish_tree(&mut self, tree: &AccessibilityTree) -> Result<(), AccessibilityError>;
    fn announce(
        &mut self,
        announcement: &AccessibilityAnnouncement,
    ) -> Result<(), AccessibilityError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedAccessibilityBridge {
    feature: String,
}

impl UnsupportedAccessibilityBridge {
    pub fn new(feature: impl Into<String>) -> Self {
        Self {
            feature: feature.into(),
        }
    }

    fn unsupported(&self) -> AccessibilityError {
        AccessibilityError::UnsupportedPlatformFeature {
            feature: self.feature.clone(),
        }
    }
}

impl AccessibilityBridge for UnsupportedAccessibilityBridge {
    fn publish_tree(&mut self, _tree: &AccessibilityTree) -> Result<(), AccessibilityError> {
        Err(self.unsupported())
    }

    fn announce(
        &mut self,
        _announcement: &AccessibilityAnnouncement,
    ) -> Result<(), AccessibilityError> {
        Err(self.unsupported())
    }
}
