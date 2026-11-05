//! Paint: the context elements paint into.

use super::*;

/// Paint context passed during paint phase
pub struct PaintContext<'a> {
    pub(crate) scene: &'a mut Scene,
    pub(crate) bounds: Bounds,
    pub(crate) taffy: &'a TaffyTree<ElementId>,
    /// Replay unchanged subtrees from the previous frame and record this one.
    pub(super) memo: bool,
    /// Unrounded layout of the element being painted, with its origin summed
    /// from the root as taffy sums it when rounding. `None` when the bounds did
    /// not come from [`Self::child_bounds`].
    pub(super) unrounded: Option<Unrounded>,
    /// The last [`Self::child_bounds`] result and its unrounded layout, which
    /// [`Self::with_bounds`] passes on when it is given those bounds.
    last_child: Cell<Option<(Bounds, Unrounded)>>,
    /// The part of the window that is seen. Unchanged subtrees entirely outside
    /// it and the active clip are not painted. `None` paints everything.
    pub(super) viewport: Option<Bounds>,
    focused: Option<ElementId>,
}

/// A node's layout before taffy rounds it.
///
/// Taffy rounds each size against the node's unrounded position from the root,
/// so a subtree's rounded layout stays the same only when that position moves
/// by whole pixels and the unrounded layout inside does not change.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Unrounded {
    pub(super) origin: Point,
    pub(super) size: Size,
}

impl<'a> PaintContext<'a> {
    pub fn new(scene: &'a mut Scene, bounds: Bounds, taffy: &'a TaffyTree<ElementId>) -> Self {
        Self {
            scene,
            bounds,
            taffy,
            memo: true,
            unrounded: None,
            last_child: Cell::new(None),
            viewport: None,
            focused: None,
        }
    }

    /// A context for the root of the tree laid out in `taffy`, which taffy
    /// always places at the origin.
    pub(crate) fn for_root(
        scene: &'a mut Scene,
        bounds: Bounds,
        taffy: &'a TaffyTree<ElementId>,
        viewport: Option<Bounds>,
    ) -> Self {
        Self {
            viewport,
            unrounded: Some(Unrounded {
                origin: Point::new(0.0, 0.0),
                size: bounds.size,
            }),
            ..Self::new(scene, bounds, taffy)
        }
    }

    /// A context that paints every element, replaying and recording nothing,
    /// to check replayed frames against.
    pub fn without_replay(
        scene: &'a mut Scene,
        bounds: Bounds,
        taffy: &'a TaffyTree<ElementId>,
    ) -> Self {
        Self {
            memo: false,
            ..Self::new(scene, bounds, taffy)
        }
    }

    pub fn with_focus(mut self, focused: Option<ElementId>) -> Self {
        self.focused = focused;
        self
    }

    pub fn is_focused(&self, id: ElementId) -> bool {
        self.focused == Some(id)
    }

    /// Add a primitive to the scene
    pub fn paint(&mut self, primitive: Primitive) {
        self.scene.insert(primitive);
    }

    /// Get the current bounds
    pub fn bounds(&self) -> Bounds {
        self.bounds
    }

    pub fn child_bounds(&self, node: NodeId) -> Option<Bounds> {
        let layout = self.taffy.layout(node).ok()?;
        let bounds = Bounds::from_xywh(
            self.bounds.x() + layout.location.x,
            self.bounds.y() + layout.location.y,
            layout.size.width,
            layout.size.height,
        );
        if let Some(parent) = self.unrounded {
            let unrounded = self.taffy.unrounded_layout(node);
            self.last_child.set(Some((
                bounds,
                Unrounded {
                    origin: Point::new(
                        parent.origin.x + unrounded.location.x,
                        parent.origin.y + unrounded.location.y,
                    ),
                    size: Size::new(unrounded.size.width, unrounded.size.height),
                },
            )));
        }
        Some(bounds)
    }

    pub fn register_hit_region(&mut self, id: ElementId, bounds: Bounds) -> bool {
        self.scene.register_hit_region(id, bounds)
    }

    pub fn register_accessibility_region(&mut self, id: ElementId, bounds: Bounds) {
        self.scene.register_accessibility_region(id, bounds);
    }

    /// Create a child paint context with new bounds
    pub fn with_bounds(&mut self, bounds: Bounds) -> PaintContext<'_> {
        let unrounded = match self.last_child.take() {
            Some((child, unrounded)) if child == bounds => Some(unrounded),
            _ if bounds == self.bounds => self.unrounded,
            _ => None,
        };
        PaintContext {
            scene: self.scene,
            bounds,
            taffy: self.taffy,
            memo: self.memo,
            unrounded,
            last_child: Cell::new(None),
            viewport: self.viewport,
            focused: self.focused,
        }
    }
}
