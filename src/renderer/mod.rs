//! Rendering subsystem

mod scene;
mod primitives;

#[cfg(target_os = "macos")]
pub mod metal;

pub use scene::Scene;
pub use primitives::Primitive;
