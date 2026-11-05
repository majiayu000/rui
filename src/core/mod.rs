//! Core types and abstractions

pub mod accessibility;
pub mod action;
pub mod animation;
pub mod app;
pub mod app_handle;
pub mod color;
pub mod color_palette;
pub mod effects;
pub mod entity;
pub mod event;
pub mod executor;
pub mod foreground;
pub mod frame_pipeline;
pub mod geometry;
pub mod geometry_ops;
pub mod presenter;
pub mod style;
pub mod text_editing;
pub mod view;
pub mod window;
mod windows;

pub use app::{App, AppContext, RedrawSource, RedrawSourceCounts};
pub use app_handle::{AppClosed, AppHandle};
pub use color::{Color, Hsla, Rgba};
pub use effects::Subscription;
pub use entity::EntityId;
pub use executor::{AsyncApp, BackgroundExecutor, BackgroundTask, Task};
pub use foreground::TaskPriority;
pub use frame_pipeline::{FramePipeline, FramePipelineError};
pub use geometry::{Bounds, Edges, Point, Rect, Size};
pub use presenter::{PointerDispatch, Presenter, PresenterFrame};
pub use style::{Background, BorderStyle, Corners, Dimension, DimensionConstraints, Style};
pub use view::{View, ViewContext, ViewNotifier};
pub use window::{Window, WindowId, WindowOptions};

use std::any::TypeId;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

/// Generate unique element IDs
static ELEMENT_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Set on ids the framework makes: [`ElementId::new`] and identities derived
/// in layout. Ids without it are the application's own.
const GENERATED_BIT: u64 = 1 << 63;
/// Set, together with [`GENERATED_BIT`], on identities derived in layout.
const DERIVED_BIT: u64 = 1 << 62;

/// Identifier for elements in the UI tree.
///
/// Ids from [`ElementId::new`] are unique but change every time the tree is
/// rebuilt. Layout gives each element a stable identity instead: its explicit
/// id when it has one, otherwise one derived from its parent's identity, its
/// position among its siblings and its type (see [`ElementId::derived`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementId(pub u64);

impl ElementId {
    /// Identity of the root element; layout derives everything below it.
    pub const ROOT: Self = Self(GENERATED_BIT | DERIVED_BIT);

    pub fn new() -> Self {
        let count = ELEMENT_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        Self(GENERATED_BIT | (count & !(GENERATED_BIT | DERIVED_BIT)))
    }

    /// Identity of the child at `index` under `parent` whose type is `kind`.
    /// The same inputs give the same id in every rebuild.
    pub fn derived(parent: ElementId, index: u64, kind: TypeId) -> Self {
        let mut hasher = IdHasher(parent.0 ^ index.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        kind.hash(&mut hasher);
        Self(GENERATED_BIT | DERIVED_BIT | (hasher.finish() & !(GENERATED_BIT | DERIVED_BIT)))
    }

    /// Whether this id came from [`ElementId::new`], so it does not survive a
    /// rebuild and cannot key an element's identity.
    pub fn is_ephemeral(self) -> bool {
        self.0 & (GENERATED_BIT | DERIVED_BIT) == GENERATED_BIT
    }
}

/// Mixes the words a `TypeId` hashes into the seed. Not a general-purpose
/// hasher: it only has to spread `derived` inputs over 62 bits.
struct IdHasher(u64);

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        // splitmix64 finalizer.
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_le_bytes(word));
        }
    }

    fn write_u64(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(23) ^ word).wrapping_mul(0x2127_599b_f432_5c37);
    }
}

impl Default for ElementId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<u64> for ElementId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

/// Pixels unit type for type-safe pixel measurements
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Pixels(pub f32);

impl Pixels {
    pub const ZERO: Self = Self(0.0);

    pub fn new(value: f32) -> Self {
        Self(value)
    }
}

impl From<f32> for Pixels {
    fn from(value: f32) -> Self {
        Self(value)
    }
}

impl From<Pixels> for f32 {
    fn from(value: Pixels) -> Self {
        value.0
    }
}

impl std::ops::Add for Pixels {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Pixels {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::Mul<f32> for Pixels {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl std::ops::Div<f32> for Pixels {
    type Output = Self;
    fn div(self, rhs: f32) -> Self::Output {
        Self(self.0 / rhs)
    }
}
