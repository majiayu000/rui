//! Primitives recorded once and drawn again unchanged.

use super::{Primitive, Reach};
use crate::core::geometry::Bounds;
use std::sync::Arc;

/// Primitives recorded once to be drawn again as they are, what a
/// [`Canvas`](crate::elements::Canvas) with a cache key and an
/// [`svg`](crate::elements::svg) icon paint. Cloning one shares its primitives.
///
/// Drawn with [`Primitive::Picture`]. The renderer keeps what it planned for a
/// picture, and while the picture is drawn again at the same offset, scale and
/// clip it copies that instead of planning the primitives again.
#[derive(Clone, Debug)]
pub struct Picture(Arc<PictureData>);

#[derive(Debug)]
pub(crate) struct PictureData {
    primitives: Box<[Primitive]>,
    reach: Reach,
}

impl Picture {
    /// A picture of `primitives`, drawn in order.
    ///
    /// # Panics
    ///
    /// When its clips are not balanced, every `PushClip` popped by a later
    /// `PopClip`, or when one of `primitives` is a picture itself.
    pub fn new(primitives: impl Into<Vec<Primitive>>) -> Self {
        let primitives: Box<[Primitive]> = primitives.into().into_boxed_slice();
        let mut depth = 0usize;
        let mut reach = Reach::Nothing;
        for primitive in &primitives {
            match primitive {
                Primitive::PushClip { .. } => depth += 1,
                Primitive::PopClip => {
                    depth = depth
                        .checked_sub(1)
                        .expect("a picture pops a clip it did not push");
                }
                Primitive::Picture { .. } => panic!("a picture cannot hold another picture"),
                // A clip only cuts what the picture draws.
                primitive => reach = union(reach, primitive.reach()),
            }
        }
        assert_eq!(depth, 0, "a picture leaves a clip pushed");
        Self(Arc::new(PictureData { primitives, reach }))
    }

    pub fn primitives(&self) -> &[Primitive] {
        &self.0.primitives
    }

    /// What the primitives can draw on, at offset zero.
    pub fn reach(&self) -> Reach {
        self.0.reach
    }

    /// Whether `other` shares this picture's primitives.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn shared(&self) -> &Arc<PictureData> {
        &self.0
    }
}

fn union(a: Reach, b: Reach) -> Reach {
    match (a, b) {
        (Reach::Unbounded, _) | (_, Reach::Unbounded) => Reach::Unbounded,
        (Reach::Nothing, other) | (other, Reach::Nothing) => other,
        (Reach::Within(a), Reach::Within(b)) => Reach::Within(a.union(&b)),
    }
}

/// `reach` moved by `(dx, dy)`.
pub(crate) fn moved(reach: Reach, dx: f32, dy: f32) -> Reach {
    match reach {
        Reach::Within(bounds) => Reach::Within(Bounds::from_xywh(
            bounds.x() + dx,
            bounds.y() + dy,
            bounds.width(),
            bounds.height(),
        )),
        other => other,
    }
}
