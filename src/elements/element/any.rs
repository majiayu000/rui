//! [`AnyElement`]: the type-erased element the frame keeps, with its stable
//! identity, reconcile, kept layout and paint replay, and action dispatch.

use super::element_box::ElementBox;
use super::*;

/// Type-erased element wrapper
pub struct AnyElement {
    pub(super) inner: ElementBox,
    stable_id: Option<ElementId>,
    /// Layout of a memoizable subtree, which the next retained pass can keep.
    layout_record: Option<LayoutRecord>,
    /// Paint of a memoizable subtree, which the next frame can replay.
    /// Frame and index of the subtree's [`PaintRecord`] in the scene.
    pub(super) paint_record: PaintSlot,
}

/// Frame and index of an element's [`PaintRecord`] in the scene, with a flag
/// that fits in the same word.
#[derive(Debug, Clone, Copy)]
pub(super) struct PaintSlot {
    /// 0 when there is no record; scene frames start at 1.
    frame: u64,
    index: u32,
    /// The subtree holds a view, so a frame that renders views visits it.
    contains_views: bool,
    /// The element has a key to pair it with when its siblings move, so
    /// unkeyed siblings pair without asking.
    pub(super) keyed: bool,
}

impl PaintSlot {
    fn get(&self) -> Option<(u64, u32)> {
        (self.frame != 0).then_some((self.frame, self.index))
    }

    fn set(&mut self, record: Option<(u64, u32)>) {
        (self.frame, self.index) = record.unwrap_or((0, 0));
    }

    fn take(&mut self) -> Option<(u64, u32)> {
        let record = self.get();
        self.set(None);
        record
    }
}

/// Where a memoizable subtree's layout nodes sit in the retained pass. Kept
/// small: every element carries one, and containers hold children inline.
struct LayoutRecord {
    epoch: u64,
    /// Identity the subtree was laid out under.
    id: ElementId,
    /// The subtree's last node, its root's.
    last: NodeId,
    /// The subtree's nodes, in creation order, in [`LayoutNodes`]; its own node
    /// comes last.
    start: u32,
    end: u32,
    text_style: Rc<TextStyle>,
    /// No element in the subtree reacts to pointer moves, releases or scrolls,
    /// so dispatch can pass it by.
    pointer_inert: bool,
}

/// Keep `previous` instead of `next`, built at the same place in the next
/// tree, when it lays out and paints the same: the caller then keeps
/// `previous`. When it does not, the children of `next` that match are swapped
/// for the previous ones, with the layout and paint they already did.
///
/// Only [`Capabilities::pure`] elements can be kept whole. All matching
/// container types pair their structural children through [`reconcile_children`],
/// so an interactive wrapper does not discard an unchanged child view.
pub fn reconcile(next: &mut dyn Element, previous: &mut dyn Element) -> bool {
    let kind = (*next).type_id();
    if kind != (*previous).type_id() {
        return false;
    }
    if let (Some(next), Some(previous)) = (
        (next as &mut dyn Any).downcast_mut::<crate::elements::Input>(),
        (previous as &mut dyn Any).downcast_mut::<crate::elements::Input>(),
    ) {
        next.reconcile_editing_state(previous);
    }
    if let (Some(next), Some(previous)) = (
        (next as &mut dyn Any).downcast_mut::<crate::elements::TextArea>(),
        (previous as &mut dyn Any).downcast_mut::<crate::elements::TextArea>(),
    ) {
        next.reconcile_editing_state(previous);
    }
    // Properties can only justify keeping two pure instances. An interactive
    // container still participates in child reconciliation below.
    let props =
        next.capabilities().pure && previous.capabilities().pure && next.same_props(previous);
    // A view's child is what it rendered, not what was built: the same view
    // keeps it, and a refresh renders it again when it changed.
    if kind == TypeId::of::<crate::elements::ViewElement>() {
        return props;
    }
    if kind == TypeId::of::<crate::elements::VariableList>() {
        let next = (next as &mut dyn std::any::Any)
            .downcast_mut::<crate::elements::VariableList>()
            .unwrap();
        let previous = (previous as &mut dyn std::any::Any)
            .downcast_mut::<crate::elements::VariableList>()
            .unwrap();
        next.take_rows_from(previous);
        return false;
    }
    let (children, old_children) = (next.children_mut(), previous.children_mut());
    // Children are paired whether or not the props match, so the ones that
    // match are kept either way.
    let same = reconcile_children(children, old_children) && props;
    if same {
        // `previous` is kept whole, so it gets its own children back.
        for (child, old) in children.iter_mut().zip(old_children.iter_mut()) {
            std::mem::swap(child, old);
        }
    }
    same
}

/// Render the views placed in `element`'s subtree that are in `dirty` or have
/// not rendered yet. Returns whether anything below changed, so what was laid
/// out and painted for it is not reused.
pub(crate) fn refresh_views(
    element: &mut dyn Element,
    app: &mut crate::core::app::AppContext,
    dirty: &std::collections::HashSet<crate::core::entity::EntityId>,
) -> bool {
    if let Some(view) = (element as &mut dyn Any).downcast_mut::<crate::elements::ViewElement>() {
        return view.refresh(app, dirty);
    }
    let mut changed = false;
    for child in element.children_mut() {
        changed |= child.refresh_views(app, dirty);
    }
    changed
}

impl AnyElement {
    // Composite builders keep one owned child tree while accessing typed props.
    pub(crate) fn downcast_ref<E: Element>(&self) -> Option<&E> {
        (&*self.inner as &dyn Any).downcast_ref()
    }

    pub(crate) fn downcast_mut<E: Element>(&mut self) -> Option<&mut E> {
        (&mut *self.inner as &mut dyn Any).downcast_mut()
    }

    pub fn new<E: Element>(element: E) -> Self {
        let is_view = TypeId::of::<E>() == TypeId::of::<crate::elements::ViewElement>();
        let mut inner = ElementBox::new(element);
        // Read from the box: reading the element before boxing copies it.
        let keyed = is_view || inner.id().is_some_and(|id| !id.is_ephemeral());
        debug_assert_eq!(
            inner.children().len(),
            inner.children_mut().len(),
            "{} lists children that children_mut leaves out",
            std::any::type_name::<E>()
        );
        // `children_mut` lists hidden children too, which may show later.
        let contains_views = is_view
            || inner
                .children_mut()
                .iter()
                .any(|child| child.paint_record.contains_views);
        Self {
            inner,
            stable_id: None,
            layout_record: None,
            paint_record: PaintSlot {
                frame: 0,
                index: 0,
                contains_views,
                keyed,
            },
        }
    }

    /// Keep `previous` instead of `self` when it lays out and paints the same.
    /// See [`reconcile`].
    pub fn reconcile(&mut self, previous: &mut AnyElement) -> bool {
        if !reconcile(&mut *self.inner, &mut *previous.inner) {
            return false;
        }
        std::mem::swap(self, previous);
        true
    }

    pub fn style(&self) -> &Style {
        self.inner.style()
    }

    pub fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let index = cx.next_child_index;
        self.layout_at(cx, index)
    }

    pub(super) fn layout_at(&mut self, cx: &mut LayoutContext, index: u64) -> NodeId {
        cx.next_child_index = index + 1;
        let id = match self.inner.id() {
            Some(id) if !id.is_ephemeral() => id,
            _ => ElementId::derived(cx.identity_parent, index, self.kind()),
        };
        self.stable_id = Some(id);
        if let Some(node) = self.keep_layout(cx) {
            return node;
        }
        // Laid out again, so what it paints may have changed.
        self.paint_record.set(None);
        if let Some(slot) = self.inner.stable_id_mut() {
            slot.adopt(id);
        }
        let parent = std::mem::replace(&mut cx.identity_parent, id);
        let next_child_index = std::mem::replace(&mut cx.next_child_index, 0);
        let start = cx.retained.as_ref().map(|pass| pass.cursor());
        let node = self.inner.layout(cx);
        cx.identity_parent = parent;
        cx.next_child_index = next_child_index;
        self.layout_record = match (start, &cx.retained) {
            (Some(start), Some(pass)) => {
                let capabilities = self.inner.capabilities();
                let children = self.children();
                (capabilities.pure && children.iter().all(|child| child.layout_record.is_some()))
                    .then(|| LayoutRecord {
                        epoch: pass.kept.epoch,
                        id,
                        last: node,
                        start: start as u32,
                        end: pass.cursor() as u32,
                        text_style: Rc::clone(&cx.text_style),
                        pointer_inert: capabilities.pointer_inert
                            && children.iter().all(|child| {
                                child
                                    .layout_record
                                    .as_ref()
                                    .is_some_and(|record| record.pointer_inert)
                            }),
                    })
            }
            _ => None,
        };
        node
    }

    /// The node of a memoizable subtree laid out unchanged in the previous
    /// retained pass, handed back without visiting the subtree. Its nodes must
    /// be the ones this pass reaches next, so the pass keeps them.
    fn keep_layout(&mut self, cx: &mut LayoutContext) -> Option<NodeId> {
        let record = self.layout_record.as_mut()?;
        let pass = cx.retained.as_mut()?;
        let (start, end) = (record.start as usize, record.end as usize);
        let kept_start = pass.cursor();
        let node = (pass.kept.epoch == record.epoch
            && Some(record.id) == self.stable_id
            && (Rc::ptr_eq(&cx.text_style, &record.text_style)
                || *cx.text_style == *record.text_style))
            .then(|| pass.keep_range(start, end, record.last))
            .flatten();
        let Some(node) = node else {
            self.layout_record = None;
            return None;
        };
        // The subtree's nodes now sit where this pass put them. Its descendants'
        // records keep the old places, which only holds while it does not move.
        record.start = kept_start as u32;
        record.end = pass.cursor() as u32;
        if !cx.previous_measured_text.is_empty() {
            for (_, node) in &pass.kept.nodes[kept_start..] {
                if let Some(text) = cx.previous_measured_text.remove(node) {
                    cx.measured_text.insert(*node, text);
                }
            }
        }
        Some(node)
    }

    fn kind(&self) -> TypeId {
        // Deref first: the box's own type id is not the element's.
        (*self.inner).type_id()
    }

    /// See [`refresh_views`]. Subtrees without views are passed by.
    pub(crate) fn refresh_views(
        &mut self,
        app: &mut crate::core::app::AppContext,
        dirty: &std::collections::HashSet<crate::core::entity::EntityId>,
    ) -> bool {
        if !self.paint_record.contains_views {
            return false;
        }
        let changed = refresh_views(&mut *self.inner, app, dirty);
        if changed {
            // Something inside was rendered again: lay out and paint it anew.
            self.layout_record = None;
            self.paint_record.set(None);
        }
        changed
    }

    /// Identity given to this element by the last layout: the same for the
    /// same element in every rebuild. `None` before the first layout.
    pub fn stable_id(&self) -> Option<ElementId> {
        self.stable_id
    }

    pub fn paint(&mut self, cx: &mut PaintContext) {
        if !cx.memo {
            if let Some(inspection) = cx.inspection {
                inspection.enter(self, cx.bounds, cx.scene.visible_bounds_for(cx.bounds));
                self.inner.paint(cx);
                inspection.exit();
                return;
            }
            self.inner.paint(cx);
            return;
        }
        let origin = cx.bounds.origin;
        let Some(unrounded) = cx.unrounded else {
            self.paint_record.set(None);
            self.inner.paint(cx);
            return;
        };
        let previous = self
            .paint_record
            .take()
            .filter(|(frame, _)| *frame == cx.scene.previous_frame())
            .and_then(|(frame, index)| cx.scene.paint_record(frame, index));
        if let Some(record) = previous
            && record.unrounded_size == unrounded.size
            && let (dx, dy) = (origin.x - record.origin.x, origin.y - record.origin.y)
            // Rounding inside the subtree repeats only after a whole-pixel move.
            && unrounded.origin.x - record.unrounded_origin.x == dx
            && unrounded.origin.y - record.unrounded_origin.y == dy
        {
            // It would draw what it drew, moved: nothing of that is seen.
            let mut reach = record.reach;
            reach.origin.x += dx;
            reach.origin.y += dy;
            let seen = cx.viewport.is_none_or(|viewport| {
                let visible = match cx.scene.current_clip() {
                    Some(clip) => clip.intersection(&viewport),
                    None => Some(viewport),
                };
                visible.is_some_and(|visible| reach.intersects(&visible))
            });
            let replayed = if !seen {
                Some(0..0)
            } else if record.painted {
                cx.scene
                    .replay(record.start as usize..record.end as usize, dx, dy)
            } else {
                None
            };
            if let Some(primitives) = replayed {
                let index = cx.scene.record(PaintRecord {
                    // A subtree left out has no range to replay next time.
                    painted: seen,
                    replayed: true,
                    start: primitives.start as u32,
                    end: primitives.end as u32,
                    origin,
                    unrounded_origin: unrounded.origin,
                    unrounded_size: unrounded.size,
                    reach,
                });
                self.paint_record.set(Some((cx.scene.frame(), index)));
                return;
            }
        }
        if let Some(reach) = cx.unseen_reach(&*self.inner) {
            // Left out as a replay leaves out what is not seen, so the next
            // frame can pass it by the same way.
            if self.layout_record.is_some() {
                let start = cx.scene.len() as u32;
                let index = cx.scene.record(PaintRecord {
                    painted: false,
                    replayed: true,
                    start,
                    end: start,
                    origin,
                    unrounded_origin: unrounded.origin,
                    unrounded_size: unrounded.size,
                    reach,
                });
                self.paint_record.set(Some((cx.scene.frame(), index)));
            }
            return;
        }
        let start = cx.scene.len();
        let regions = cx.scene.region_count();
        let layers = cx.scene.layers().len();
        self.inner.paint(cx);
        let frame = cx.scene.frame();
        let scene = &*cx.scene;
        let replayable = self.layout_record.is_some()
            && scene.region_count() == regions
            && scene.layers().len() == layers
            && self.children().iter().all(|child| {
                child.paint_record.get().is_some_and(|(at, index)| {
                    at == frame
                        && scene
                            .paint_record(at, index)
                            .is_some_and(|record| record.painted)
                })
            });
        if replayable {
            let index = cx.scene.record(PaintRecord {
                painted: true,
                replayed: false,
                start: start as u32,
                end: cx.scene.len() as u32,
                origin,
                unrounded_origin: unrounded.origin,
                unrounded_size: unrounded.size,
                reach: self.painted_reach(cx.scene, start),
            });
            self.paint_record.set(Some((frame, index)));
        }
    }

    /// What the primitives this element painted from `start` reach: each
    /// child's recorded reach, and the primitives between children, so a
    /// primitive is measured once however deep it is painted. Children must
    /// all have painted records of this frame.
    fn painted_reach(&self, scene: &Scene, start: usize) -> Bounds {
        let (mut reach, mut at) = (Bounds::default(), start);
        for child in self.children() {
            let record = child
                .paint_record
                .get()
                .and_then(|(frame, index)| scene.paint_record(frame, index));
            let Some(record) = record.filter(|record| record.start as usize >= at) else {
                return scene.reach_of(start..scene.len());
            };
            reach = reach
                .union(&scene.reach_of(at..record.start as usize))
                .union(&record.reach);
            at = record.end as usize;
        }
        reach.union(&scene.reach_of(at..scene.len()))
    }

    /// Frame and index of the [`PaintRecord`] this element last left.
    pub(crate) fn paint_record_slot(&self) -> Option<(u64, u32)> {
        self.paint_record.get()
    }

    pub(crate) fn type_name(&self) -> &'static str {
        self.inner.type_name()
    }

    pub fn refresh_text_geometry(&mut self, text_measurer: &mut TextMeasureCache) {
        self.inner.refresh_text_geometry(text_measurer)
    }

    pub fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        // A press can clear focus in any div it misses, so presses always go in.
        if !matches!(event.kind, PointerEventKind::Down) && self.pointer_inert() {
            return false;
        }
        let matches_current = cx
            .hit_target()
            .map(|target| self.contains_id(target))
            .unwrap_or(false);
        let matches_previous = cx
            .previous_hit_target()
            .map(|target| self.contains_id(target))
            .unwrap_or(false);

        // Outside the viewport clip: never start new presses, but still forward
        // Move/Up (and focused Downs for blur) so unregistered pressed/hovered
        // controls can clear state even when Presenter filters to an unrelated
        // scene hit_target. In-viewport Move/Up filtered to an unrelated
        // registered target also need suppressed cleanup — otherwise a press on
        // an unregistered control sticks when released over a sibling hit
        // region. Always deliver those cleanups with forced outside
        // containment so background Div::on_click / hover cannot activate under
        // a registered overlay. Focus delivery is Down-only blur: never treat
        // focused Move/Up/Down as activatable hits through an overlay.
        let delivers_for_focus = cx
            .focused_id()
            .map(|id| self.contains_id(id))
            .unwrap_or(false);
        let outside_hit_region = cx.pointer_outside_hit_region(event.position);
        let focus_blur_delivery =
            delivers_for_focus && matches!(event.kind, PointerEventKind::Down);
        let filtered_pointer_cleanup =
            matches!(event.kind, PointerEventKind::Move | PointerEventKind::Up)
                && cx.has_hit_filter()
                && !matches_current
                && !matches_previous;
        let is_cleanup = (matches!(event.kind, PointerEventKind::Move | PointerEventKind::Up)
            && outside_hit_region)
            || filtered_pointer_cleanup
            || focus_blur_delivery;

        if cx.has_hit_filter() && !matches_current && !matches_previous && !is_cleanup {
            return false;
        }

        if outside_hit_region && !is_cleanup {
            return false;
        }

        // Force outside containment for overlay focus blur and for filtered
        // Move/Up cleanup so in-viewport releases over an unrelated registered
        // hit clear pressed/hover without activating. Do not suppress normal
        // focused Downs that are not hit-filter bypasses.
        let force_outside = filtered_pointer_cleanup
            || (focus_blur_delivery
                && cx.has_hit_filter()
                && !matches_current
                && !matches_previous);
        let mut dispatch_cx = if force_outside {
            cx.for_blur_dispatch()
        } else {
            cx.for_pointer_dispatch(event.position)
        };
        self.inner
            .dispatch_pointer_event(&mut dispatch_cx, event)
            .is_stopped()
    }

    pub fn handle_scroll_event(&mut self, cx: &mut EventContext, event: &ScrollEvent) -> bool {
        if self.pointer_inert() {
            return false;
        }
        self.inner.handle_scroll_event(cx, event)
    }

    /// Whether the last layout found nothing in the subtree that reacts to
    /// pointer moves, releases or scrolls.
    pub(crate) fn pointer_inert(&self) -> bool {
        self.layout_record
            .as_ref()
            .is_some_and(|record| record.pointer_inert)
    }

    pub fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        self.inner.handle_key_event(cx, event)
    }

    pub fn dispatch_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        self.inner.dispatch_action(cx, action)
    }

    pub fn handle_text_input_event(
        &mut self,
        cx: &mut EventContext,
        event: &TextInputEvent,
    ) -> bool {
        self.inner.handle_text_input_event(cx, event)
    }

    pub fn handle_text_input_command(
        &mut self,
        cx: &mut EventContext,
        command: &TextInputCommand,
    ) -> bool {
        self.inner.handle_text_input_command(cx, command)
    }

    pub fn text_input_snapshot(&self, focused: ElementId) -> Option<TextInputSnapshot> {
        self.inner.text_input_snapshot(focused)
    }

    pub fn handle_window_event(&mut self, event: &crate::core::event::Event) -> bool {
        self.inner.handle_window_event(event)
    }

    pub fn id(&self) -> Option<ElementId> {
        self.inner.id()
    }

    pub fn contains_id(&self, id: ElementId) -> bool {
        self.inner.contains_id(id)
    }

    pub fn children(&self) -> &[AnyElement] {
        self.inner.children()
    }

    pub(crate) fn accessibility_nodes(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Vec<AccessibilityNode>, AccessibilityError> {
        self.inner.accessibility_nodes(cx)
    }
}

pub(crate) fn dispatch_action_to_children(
    children: &mut [AnyElement],
    cx: &mut EventContext,
    action: &ActionId,
) -> ActionOutcome {
    let focused_index = cx.focused_id().and_then(|focused| {
        children
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, child)| child.contains_id(focused).then_some(index))
    });

    if let Some(index) = focused_index {
        let outcome = children[index].dispatch_action(cx, action);
        if outcome.is_handled() {
            return outcome;
        }
    }

    for (index, child) in children.iter_mut().enumerate().rev() {
        if Some(index) == focused_index {
            continue;
        }
        let outcome = child.dispatch_action(cx, action);
        if outcome.is_handled() {
            return outcome;
        }
    }

    ActionOutcome::Ignored
}

impl<E: Element> From<E> for AnyElement {
    fn from(element: E) -> Self {
        AnyElement::new(element)
    }
}
