use crate::core::ElementId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccessibilityAnnouncementKind {
    FocusChanged,
    ActionFeedback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibilityAnnouncement {
    node_id: ElementId,
    kind: AccessibilityAnnouncementKind,
    message: String,
}

impl AccessibilityAnnouncement {
    pub fn new(
        node_id: ElementId,
        kind: AccessibilityAnnouncementKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            node_id,
            kind,
            message: message.into(),
        }
    }

    pub fn node_id(&self) -> ElementId {
        self.node_id
    }

    pub fn kind(&self) -> AccessibilityAnnouncementKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}
