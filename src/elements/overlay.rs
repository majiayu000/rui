//! Floating content laid out against the window instead of its parent.
//!
//! An element that opens an overlay lays the overlay's content out as a
//! separate layout root sized against the window ([`OverlaySlot`]), so the
//! content takes no part in the parent's layout. When painting, it places the
//! content against the window or next to an anchor ([`place_anchored`]) and
//! paints it with [`PaintContext::paint_overlay`], which draws it over the
//! rest of the frame and hits it first. Events reach the content through
//! [`EventContext::for_overlay`] at the same place.
//!
//! Placement follows GPUI's `anchored` and Floating UI's `flip` and `shift`:
//! the side is flipped when the content does not fit on it and fits better on
//! the other, then the content is shifted to stay inside the window.

use crate::core::ElementId;
use crate::core::geometry::{Bounds, Point, Size};
use crate::elements::element::{AnyElement, LayoutContext, PaintContext};
use taffy::prelude::{AvailableSpace, NodeId, TaffyTree};

#[cfg(doc)]
use crate::elements::element::EventContext;

/// The side of the anchor the content opens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
}

impl Side {
    fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    fn vertical(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }
}

/// How the content lines up with the anchor along the side it opens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

/// Where anchored content goes: on `side` of the anchor, lined up by `align`,
/// `gap` away from it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Placement {
    pub side: Side,
    pub align: Align,
    pub gap: f32,
}

impl Placement {
    pub fn new(side: Side, align: Align, gap: f32) -> Self {
        Self { side, align, gap }
    }
}

/// Bounds of content `size` large next to `anchor`, kept inside `window`:
/// flipped to the opposite side when it overflows its side and the opposite
/// one has more room, then shifted onto the window. Whole pixels.
pub fn place_anchored(anchor: Bounds, size: Size, window: Bounds, placement: Placement) -> Bounds {
    let mut side = placement.side;
    if overflow(at(anchor, size, side, placement), window, side) > 0.0 {
        let flipped = side.opposite();
        let flipped_overflow = overflow(at(anchor, size, flipped, placement), window, flipped);
        if flipped_overflow < overflow(at(anchor, size, side, placement), window, side) {
            side = flipped;
        }
    }
    let placed = at(anchor, size, side, placement);
    snap(placed, window)
}

/// Bounds of content `size` large centred in `window`, shifted onto it.
pub fn center_in(window: Bounds, size: Size) -> Bounds {
    let origin = Point::new(
        window.x() + (window.width() - size.width) / 2.0,
        window.y() + (window.height() - size.height) / 2.0,
    );
    snap(Bounds::new(origin, size), window)
}

fn at(anchor: Bounds, size: Size, side: Side, placement: Placement) -> Bounds {
    let align = |start: f32, anchor_len: f32, len: f32| match placement.align {
        Align::Start => start,
        Align::Center => start + (anchor_len - len) / 2.0,
        Align::End => start + anchor_len - len,
    };
    let origin = match side {
        Side::Bottom => Point::new(
            align(anchor.x(), anchor.width(), size.width),
            anchor.max_y() + placement.gap,
        ),
        Side::Top => Point::new(
            align(anchor.x(), anchor.width(), size.width),
            anchor.y() - placement.gap - size.height,
        ),
        Side::Right => Point::new(
            anchor.max_x() + placement.gap,
            align(anchor.y(), anchor.height(), size.height),
        ),
        Side::Left => Point::new(
            anchor.x() - placement.gap - size.width,
            align(anchor.y(), anchor.height(), size.height),
        ),
    };
    Bounds::new(origin, size)
}

/// How far `bounds` reaches out of `window` along `side`'s axis.
fn overflow(bounds: Bounds, window: Bounds, side: Side) -> f32 {
    let (start, end, min, max) = if side.vertical() {
        (bounds.y(), bounds.max_y(), window.y(), window.max_y())
    } else {
        (bounds.x(), bounds.max_x(), window.x(), window.max_x())
    };
    (min - start).max(0.0) + (end - max).max(0.0)
}

/// Shift `bounds` onto `window`, keeping its start edges in when it is too
/// large, and round its origin to whole pixels.
fn snap(bounds: Bounds, window: Bounds) -> Bounds {
    let shift = |start: f32, len: f32, min: f32, max: f32| {
        let start = if start + len > max { max - len } else { start };
        start.max(min).round()
    };
    Bounds::new(
        Point::new(
            shift(bounds.x(), bounds.width(), window.x(), window.max_x()),
            shift(bounds.y(), bounds.height(), window.y(), window.max_y()),
        ),
        bounds.size,
    )
}

/// The layout root of an overlay's content, laid out against the window and
/// apart from the tree of the element that opens it.
#[derive(Debug, Default, Clone, Copy)]
pub struct OverlaySlot {
    node: Option<NodeId>,
}

impl OverlaySlot {
    /// Lay `content` out as its own root, as wide and high as it wants up to
    /// the window.
    pub fn layout(&mut self, cx: &mut LayoutContext, content: &mut AnyElement) {
        self.layout_with(cx, |cx| content.layout(cx));
    }

    /// [`Self::layout`] for content laid out by `layout`.
    pub fn layout_with(
        &mut self,
        cx: &mut LayoutContext,
        layout: impl FnOnce(&mut LayoutContext) -> NodeId,
    ) {
        let node = layout(cx);
        let window = cx.available_space;
        let space = taffy::Size {
            width: AvailableSpace::Definite(window.width),
            height: AvailableSpace::Definite(window.height),
        };
        if let Err(err) = cx.compute_subtree(node, space) {
            panic!("failed to lay out overlay content: {err}");
        }
        self.node = Some(node);
    }

    /// Forget the content, as when the overlay closes.
    pub fn clear(&mut self) {
        self.node = None;
    }

    /// The size the content was laid out at.
    pub fn size(&self, taffy: &TaffyTree<ElementId>) -> Option<Size> {
        let layout = taffy.layout(self.node?).ok()?;
        Some(Size::new(layout.size.width, layout.size.height))
    }

    /// [`Self::size`] in a paint context, zero before layout.
    pub fn size_in(&self, cx: &PaintContext) -> Size {
        self.size(cx.taffy).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: Bounds = Bounds::new(Point::new(0.0, 0.0), Size::new(400.0, 300.0));

    fn below() -> Placement {
        Placement::new(Side::Bottom, Align::Start, 4.0)
    }

    #[test]
    fn content_opens_below_the_anchor_where_it_fits() {
        let anchor = Bounds::from_xywh(20.0, 20.0, 100.0, 30.0);
        let placed = place_anchored(anchor, Size::new(120.0, 80.0), WINDOW, below());
        assert_eq!(placed, Bounds::from_xywh(20.0, 54.0, 120.0, 80.0));
    }

    #[test]
    fn content_flips_above_an_anchor_near_the_bottom_edge() {
        let anchor = Bounds::from_xywh(20.0, 250.0, 100.0, 30.0);
        let placed = place_anchored(anchor, Size::new(120.0, 80.0), WINDOW, below());
        assert_eq!(placed, Bounds::from_xywh(20.0, 166.0, 120.0, 80.0));
    }

    #[test]
    fn content_shifts_left_of_an_anchor_near_the_right_edge() {
        let anchor = Bounds::from_xywh(350.0, 20.0, 40.0, 30.0);
        let placed = place_anchored(anchor, Size::new(120.0, 80.0), WINDOW, below());
        assert_eq!(placed.max_x(), WINDOW.max_x());
        assert_eq!(placed.y(), 54.0);
    }

    #[test]
    fn content_with_no_room_on_either_side_stays_inside() {
        let anchor = Bounds::from_xywh(20.0, 140.0, 100.0, 20.0);
        let placed = place_anchored(anchor, Size::new(120.0, 200.0), WINDOW, below());
        assert!(placed.y() >= 0.0 && placed.max_y() <= WINDOW.max_y());
    }

    #[test]
    fn centred_content_is_in_the_middle_of_the_window() {
        let placed = center_in(WINDOW, Size::new(100.0, 50.0));
        assert_eq!(placed, Bounds::from_xywh(150.0, 125.0, 100.0, 50.0));
    }
}
