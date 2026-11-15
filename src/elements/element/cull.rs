//! Leaving out subtrees that nothing in the window can show, before painting
//! them. See [`Element::unseen_reach`].

use super::*;

/// Bounds of the child laid out at `node` in a parent at `parent`, as
/// [`PaintContext::child_bounds`] gives them.
pub fn child_bounds_in(
    taffy: &TaffyTree<ElementId>,
    parent: Bounds,
    node: NodeId,
) -> Option<Bounds> {
    let layout = taffy.layout(node).ok()?;
    Some(Bounds::from_xywh(
        parent.x() + layout.location.x,
        parent.y() + layout.location.y,
        layout.size.width,
        layout.size.height,
    ))
}

/// The reach of nothing drawn, which no bounds intersect.
pub fn no_reach() -> Bounds {
    Bounds::from_xywh(0.0, 0.0, 0.0, 0.0)
}

/// `reach` and `more` together; [`no_reach`] adds nothing.
pub fn add_reach(reach: Bounds, more: Bounds) -> Bounds {
    if reach.is_empty() {
        more
    } else if more.is_empty() {
        reach
    } else {
        reach.union(&more)
    }
}

impl AnyElement {
    /// See [`Element::unseen_reach`].
    pub fn unseen_reach(
        &self,
        taffy: &TaffyTree<ElementId>,
        bounds: Bounds,
        visible: &Bounds,
    ) -> Option<Bounds> {
        self.inner.unseen_reach(taffy, bounds, visible)
    }
}

impl PaintContext<'_> {
    /// Where `element`, painted at this context's bounds, would draw, when
    /// nothing of it can be seen in the window. `None` without a viewport, and
    /// when clipped out entirely: rare, and painting it is never wrong.
    pub(super) fn unseen_reach(&self, element: &dyn Element) -> Option<Bounds> {
        let viewport = self.viewport?;
        let visible = match self.scene.current_clip() {
            Some(clip) => clip.intersection(&viewport)?,
            None => viewport,
        };
        element.unseen_reach(self.taffy, self.bounds, &visible)
    }
}
