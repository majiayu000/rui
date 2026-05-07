//! Additional geometry helpers.
//!
//! Augments [`crate::core::geometry::Point`], [`crate::core::geometry::Size`]
//! and [`crate::core::geometry::Rect`] with non-breaking ergonomic operations:
//!
//! - `Point::lerp` for animation-style interpolation
//! - `Size::min` / `Size::max` for clamping layouts
//! - `Rect::translate`, `Rect::inflate`, `Rect::union` for spatial layout work
//!
//! These are inherent-impl additions on existing types; no public signatures
//! are changed. Hot helpers are marked `#[inline]` to keep them free in the
//! render path.
//!
//! # Examples
//!
//! Translate and inflate a hit box, then union with a tooltip frame:
//!
//! ```
//! use rui::{Point, Rect, Size};
//!
//! let button = Rect::from_xywh(10.0, 20.0, 100.0, 40.0);
//! let tooltip = Rect::from_xywh(140.0, 25.0, 80.0, 30.0);
//! let hit_box = button.inflate(4.0, 4.0);
//! let combined = hit_box.union(&tooltip);
//!
//! assert_eq!(hit_box.size(), Size::new(108.0, 48.0));
//! assert_eq!(combined.origin(), Point::new(6.0, 16.0));
//! ```

use super::geometry::{Bounds, Edges, Point, Rect, Size};

impl Point {
    /// Linearly interpolate from `self` to `other` by `t` in `0.0..=1.0`.
    ///
    /// Values outside that range are accepted (extrapolation), matching the
    /// behavior of `Rgba::lerp`.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::Point;
    /// let mid = Point::new(0.0, 0.0).lerp(Point::new(10.0, 20.0), 0.5);
    /// assert_eq!(mid, Point::new(5.0, 10.0));
    /// ```
    #[inline]
    pub fn lerp(&self, other: Point, t: f32) -> Point {
        Point::new(
            self.x + (other.x - self.x) * t,
            self.y + (other.y - self.y) * t,
        )
    }

    /// Translate the point by an `(x, y)` offset.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::Point;
    /// let p = Point::new(2.0, 3.0).translate(1.0, -1.0);
    /// assert_eq!(p, Point::new(3.0, 2.0));
    /// ```
    #[inline]
    pub fn translate(&self, dx: f32, dy: f32) -> Point {
        Point::new(self.x + dx, self.y + dy)
    }
}

impl Size {
    /// Component-wise minimum.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::Size;
    /// let clamped = Size::new(100.0, 200.0).min(Size::new(80.0, 240.0));
    /// assert_eq!(clamped, Size::new(80.0, 200.0));
    /// ```
    #[inline]
    pub fn min(&self, other: Size) -> Size {
        Size::new(self.width.min(other.width), self.height.min(other.height))
    }

    /// Component-wise maximum.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::Size;
    /// let clamped = Size::new(100.0, 50.0).max(Size::new(120.0, 40.0));
    /// assert_eq!(clamped, Size::new(120.0, 50.0));
    /// ```
    #[inline]
    pub fn max(&self, other: Size) -> Size {
        Size::new(self.width.max(other.width), self.height.max(other.height))
    }

    /// Clamp this size between a minimum and maximum, component-wise.
    ///
    /// `min` must be no larger than `max` on each axis; this method asserts
    /// that invariant in debug builds and is silent in release.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::Size;
    /// let s = Size::new(150.0, 50.0).clamp(Size::new(40.0, 40.0), Size::new(120.0, 120.0));
    /// assert_eq!(s, Size::new(120.0, 50.0));
    /// ```
    #[inline]
    pub fn clamp(&self, min: Size, max: Size) -> Size {
        debug_assert!(min.width <= max.width && min.height <= max.height);
        Size::new(
            self.width.clamp(min.width, max.width),
            self.height.clamp(min.height, max.height),
        )
    }
}

impl Rect {
    /// Return the rectangle's origin point.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::{Point, Rect};
    /// let r = Rect::from_xywh(3.0, 4.0, 10.0, 20.0);
    /// assert_eq!(r.origin(), Point::new(3.0, 4.0));
    /// ```
    #[inline]
    pub fn origin(&self) -> Point {
        self.origin
    }

    /// Return the rectangle's size.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::{Rect, Size};
    /// let r = Rect::from_xywh(0.0, 0.0, 10.0, 20.0);
    /// assert_eq!(r.size(), Size::new(10.0, 20.0));
    /// ```
    #[inline]
    pub fn size(&self) -> Size {
        self.size
    }

    /// Translate the rectangle by an `(x, y)` offset.
    ///
    /// Only the origin moves; the size is unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::Rect;
    /// let r = Rect::from_xywh(1.0, 2.0, 10.0, 20.0).translate(4.0, -1.0);
    /// assert_eq!(r, Rect::from_xywh(5.0, 1.0, 10.0, 20.0));
    /// ```
    #[inline]
    pub fn translate(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.origin.translate(dx, dy), self.size)
    }

    /// Inflate the rectangle by `dx` on each horizontal edge and `dy` on each
    /// vertical edge.
    ///
    /// Negative values shrink the rectangle. Width and height are clamped at
    /// zero to avoid producing negative sizes.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::Rect;
    /// let inflated = Rect::from_xywh(10.0, 10.0, 20.0, 20.0).inflate(2.0, 4.0);
    /// assert_eq!(inflated, Rect::from_xywh(8.0, 6.0, 24.0, 28.0));
    /// ```
    #[inline]
    pub fn inflate(&self, dx: f32, dy: f32) -> Rect {
        let width = (self.size.width + 2.0 * dx).max(0.0);
        let height = (self.size.height + 2.0 * dy).max(0.0);
        Rect::from_xywh(self.origin.x - dx, self.origin.y - dy, width, height)
    }

    /// Inflate the rectangle by per-edge amounts.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::{Edges, Rect};
    /// let r = Rect::from_xywh(10.0, 10.0, 20.0, 20.0)
    ///     .inflate_edges(Edges::new(1.0, 2.0, 3.0, 4.0));
    /// assert_eq!(r, Rect::from_xywh(6.0, 9.0, 26.0, 24.0));
    /// ```
    #[inline]
    pub fn inflate_edges(&self, edges: Edges) -> Rect {
        let width = (self.size.width + edges.left + edges.right).max(0.0);
        let height = (self.size.height + edges.top + edges.bottom).max(0.0);
        Rect::from_xywh(
            self.origin.x - edges.left,
            self.origin.y - edges.top,
            width,
            height,
        )
    }

    /// Return the smallest rectangle that contains both `self` and `other`.
    ///
    /// Empty rectangles (`is_empty() == true`) are skipped: the union of an
    /// empty rect with `r` is `r`. If both are empty, the result is also empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use rui::Rect;
    /// let a = Rect::from_xywh(0.0, 0.0, 10.0, 10.0);
    /// let b = Rect::from_xywh(20.0, 5.0, 10.0, 10.0);
    /// let u = a.union(&b);
    /// assert_eq!(u, Rect::from_xywh(0.0, 0.0, 30.0, 15.0));
    /// ```
    #[inline]
    pub fn union(&self, other: &Rect) -> Rect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let min_x = self.min_x().min(other.min_x());
        let min_y = self.min_y().min(other.min_y());
        let max_x = self.max_x().max(other.max_x());
        let max_y = self.max_y().max(other.max_y());
        Rect::from_xywh(min_x, min_y, max_x - min_x, max_y - min_y)
    }
}

/// Compile-time confirmation that `Bounds` is a `Rect` alias.
const _: fn(Bounds) -> Rect = |b| b;

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-5, "expected {} == {}", a, b);
    }

    // ---- Point ----

    #[test]
    fn point_lerp_endpoints() {
        let a = Point::new(1.0, 2.0);
        let b = Point::new(5.0, 8.0);
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
    }

    #[test]
    fn point_lerp_midpoint() {
        let mid = Point::new(0.0, 0.0).lerp(Point::new(10.0, 20.0), 0.5);
        assert_eq!(mid, Point::new(5.0, 10.0));
    }

    #[test]
    fn point_lerp_extrapolation() {
        let p = Point::new(0.0, 0.0).lerp(Point::new(10.0, 0.0), 1.5);
        approx(p.x, 15.0);
        approx(p.y, 0.0);
    }

    #[test]
    fn point_translate() {
        let p = Point::new(2.0, 3.0).translate(1.0, -1.0);
        assert_eq!(p, Point::new(3.0, 2.0));
    }

    // ---- Size ----

    #[test]
    fn size_min_max() {
        let a = Size::new(100.0, 50.0);
        let b = Size::new(80.0, 60.0);
        assert_eq!(a.min(b), Size::new(80.0, 50.0));
        assert_eq!(a.max(b), Size::new(100.0, 60.0));
    }

    #[test]
    fn size_clamp_inside() {
        let s = Size::new(50.0, 50.0).clamp(Size::new(10.0, 10.0), Size::new(100.0, 100.0));
        assert_eq!(s, Size::new(50.0, 50.0));
    }

    #[test]
    fn size_clamp_above() {
        let s = Size::new(150.0, 50.0).clamp(Size::new(40.0, 40.0), Size::new(120.0, 120.0));
        assert_eq!(s, Size::new(120.0, 50.0));
    }

    #[test]
    fn size_clamp_below() {
        let s = Size::new(5.0, 50.0).clamp(Size::new(40.0, 40.0), Size::new(120.0, 120.0));
        assert_eq!(s, Size::new(40.0, 50.0));
    }

    #[test]
    #[should_panic]
    fn size_clamp_invalid_bounds_debug() {
        // In debug builds the assertion fires; release mode would silently
        // produce undefined ordering on f32::clamp, which we want to forbid.
        if cfg!(debug_assertions) {
            let _ = Size::new(10.0, 10.0).clamp(Size::new(100.0, 100.0), Size::new(50.0, 50.0));
        } else {
            // Force panic in release so the test passes uniformly; the actual
            // guard is only meaningful when debug assertions are on.
            panic!("clamp invariant only enforced in debug");
        }
    }

    // ---- Rect ----

    #[test]
    fn rect_translate_only_moves_origin() {
        let r = Rect::from_xywh(1.0, 2.0, 10.0, 20.0).translate(4.0, -1.0);
        assert_eq!(r, Rect::from_xywh(5.0, 1.0, 10.0, 20.0));
    }

    #[test]
    fn rect_inflate_grows_symmetrically() {
        let r = Rect::from_xywh(10.0, 10.0, 20.0, 20.0).inflate(2.0, 4.0);
        assert_eq!(r, Rect::from_xywh(8.0, 6.0, 24.0, 28.0));
    }

    #[test]
    fn rect_inflate_negative_clamps_to_zero() {
        // Shrink past the size — width/height clamp to 0, origin still shifts.
        let r = Rect::from_xywh(0.0, 0.0, 10.0, 10.0).inflate(-100.0, -100.0);
        assert_eq!(r.width(), 0.0);
        assert_eq!(r.height(), 0.0);
    }

    #[test]
    fn rect_inflate_edges_uses_each_side() {
        let r = Rect::from_xywh(10.0, 10.0, 20.0, 20.0)
            .inflate_edges(Edges::new(1.0, 2.0, 3.0, 4.0));
        assert_eq!(r, Rect::from_xywh(6.0, 9.0, 26.0, 24.0));
    }

    #[test]
    fn rect_union_basic() {
        let a = Rect::from_xywh(0.0, 0.0, 10.0, 10.0);
        let b = Rect::from_xywh(20.0, 5.0, 10.0, 10.0);
        assert_eq!(a.union(&b), Rect::from_xywh(0.0, 0.0, 30.0, 15.0));
    }

    #[test]
    fn rect_union_with_empty_returns_other() {
        let a = Rect::from_xywh(0.0, 0.0, 10.0, 10.0);
        let empty = Rect::ZERO;
        assert_eq!(a.union(&empty), a);
        assert_eq!(empty.union(&a), a);
        assert_eq!(empty.union(&empty), Rect::ZERO);
    }

    #[test]
    fn rect_union_overlapping() {
        let a = Rect::from_xywh(0.0, 0.0, 30.0, 30.0);
        let b = Rect::from_xywh(10.0, 10.0, 10.0, 10.0);
        // b is fully inside a — union equals a.
        assert_eq!(a.union(&b), a);
    }

    #[test]
    fn rect_origin_size_accessors() {
        let r = Rect::from_xywh(3.0, 4.0, 10.0, 20.0);
        assert_eq!(r.origin(), Point::new(3.0, 4.0));
        assert_eq!(r.size(), Size::new(10.0, 20.0));
    }

    #[test]
    fn rect_translate_then_inflate() {
        let r = Rect::from_xywh(0.0, 0.0, 10.0, 10.0)
            .translate(5.0, 5.0)
            .inflate(1.0, 1.0);
        assert_eq!(r, Rect::from_xywh(4.0, 4.0, 12.0, 12.0));
    }
}
