use crate::core::accessibility::{
    AccessibilityAnnouncement, AccessibilityBridge, AccessibilityError, AccessibilityTree,
};

#[derive(Debug, Default)]
pub struct MacAccessibilityBridge {
    native_attached: bool,
}

impl MacAccessibilityBridge {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn native_attached(&self) -> bool {
        self.native_attached
    }

    fn missing_native_bridge() -> AccessibilityError {
        AccessibilityError::BridgeFailure {
            message: "macOS accessibility bridge is not attached to a native AppKit host"
                .to_string(),
        }
    }
}

impl AccessibilityBridge for MacAccessibilityBridge {
    fn publish_tree(&mut self, _tree: &AccessibilityTree) -> Result<(), AccessibilityError> {
        Err(Self::missing_native_bridge())
    }

    fn announce(
        &mut self,
        _announcement: &AccessibilityAnnouncement,
    ) -> Result<(), AccessibilityError> {
        Err(Self::missing_native_bridge())
    }
}
