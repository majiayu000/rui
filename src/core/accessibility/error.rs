use super::node::AccessibilityRole;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessibilityError {
    MissingLabel { role: AccessibilityRole },
    MissingValue { role: AccessibilityRole },
    UnsupportedPlatformFeature { feature: String },
    BridgeFailure { message: String },
}

impl fmt::Display for AccessibilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingLabel { role } => {
                write!(f, "accessibility label is required for {role:?}")
            }
            Self::MissingValue { role } => {
                write!(f, "accessibility value is required for {role:?}")
            }
            Self::UnsupportedPlatformFeature { feature } => {
                write!(f, "accessibility feature is unsupported: {feature}")
            }
            Self::BridgeFailure { message } => {
                write!(f, "accessibility bridge failed: {message}")
            }
        }
    }
}

impl std::error::Error for AccessibilityError {}
