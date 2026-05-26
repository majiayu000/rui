//! Rendering subsystem

mod primitives;
mod scene;
pub mod text;

#[cfg(target_os = "macos")]
pub mod metal;

pub use primitives::Primitive;
pub use scene::{HitRegion, Layer, LayerId, Scene, ZIndex};
