//! Accessibility semantics, announcements, and bridge contracts.

mod announcement;
mod bridge;
mod error;
mod node;

pub use announcement::{AccessibilityAnnouncement, AccessibilityAnnouncementKind};
pub use bridge::{AccessibilityBridge, UnsupportedAccessibilityBridge};
pub use error::AccessibilityError;
pub use node::{AccessibilityContext, AccessibilityNode, AccessibilityRole, AccessibilityTree};
