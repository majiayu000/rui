//! Events: pointer events and the context elements handle events in.

use super::*;
use crate::renderer::Overlay;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerEventKind {
    Move,
    Down,
    Up,
}

#[derive(Debug, Clone, Copy)]
pub struct PointerEvent {
    pub kind: PointerEventKind,
    pub position: Point,
    pub button: Option<MouseButton>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventResult {
    #[default]
    Propagate,
    Stop,
}

impl EventResult {
    pub fn from_handled(handled: bool) -> Self {
        if handled { Self::Stop } else { Self::Propagate }
    }

    pub fn is_stopped(self) -> bool {
        matches!(self, Self::Stop)
    }
}

pub struct EventContext<'a> {
    pub(crate) bounds: Bounds,
    /// Optional absolute-space clip for pointer hit-testing.
    ///
    /// Layout coordinates (`bounds` / `child_bounds`) stay unclipped so nested
    /// origins remain correct under scroll; hit checks also require this clip.
    hit_clip: Option<Bounds>,
    /// When set, [`Self::bounds`] returns a never-hit sentinel so legacy
    /// `bounds().contains` checks cannot activate clipped-away content.
    /// Real geometry remains available via [`Self::layout_bounds`].
    ///
    /// Clip-derived suppression is re-evaluated against each child bounds in
    /// [`Self::with_bounds`] / [`Self::with_bounds_and_hit_clip`] (and again by
    /// child [`Self::for_pointer_dispatch`]) using [`Self::hit_test_point`].
    /// See [`Self::force_hit_suppressed`] for blur suppression that must also
    /// survive concrete `with_bounds` paths.
    hit_suppressed: bool,
    /// Forced outside-hit for focus blur through an unrelated overlay.
    ///
    /// Unlike clip-derived [`Self::hit_suppressed`], this must propagate through
    /// [`Self::with_bounds`] / [`Self::with_bounds_and_hit_clip`] so concrete
    /// children (e.g. `TabsRoot` → `TabList`) keep treating the event as outside.
    force_hit_suppressed: bool,
    /// Pointer position from the active [`Self::for_pointer_dispatch`] call.
    ///
    /// Concrete composites that call [`Self::with_bounds`] without re-entering
    /// `AnyElement` recompute clip-derived [`Self::hit_suppressed`] against this
    /// point so legacy `bounds().contains` cannot activate outside the clip.
    hit_test_point: Option<Point>,
    pub(crate) taffy: &'a TaffyTree<ElementId>,
    pub(crate) focused: &'a mut Option<ElementId>,
    hit_target: Option<ElementId>,
    previous_hit_target: Option<ElementId>,
    cursor: Rc<Cell<Option<Cursor>>>,
    redraw_requested: Rc<Cell<bool>>,
    accessibility_announcements: Rc<RefCell<Vec<AccessibilityAnnouncement>>>,
    /// The app clipboard, attached while key actions run.
    clipboard: Option<SharedClipboard>,
    /// The window's drag, shared with its presenter.
    drag: Option<SharedDrag>,
    /// The window, which overlays are placed against.
    window: Bounds,
    /// Overlays of the frame the event lands on, topmost last.
    overlays: Option<Rc<[Overlay]>>,
    /// The press goes only to owners of light-dismiss overlays it is outside.
    dismiss_pass: bool,
}

impl<'a> EventContext<'a> {
    pub fn new(
        bounds: Bounds,
        taffy: &'a TaffyTree<ElementId>,
        focused: &'a mut Option<ElementId>,
    ) -> Self {
        Self {
            bounds,
            hit_clip: None,
            hit_suppressed: false,
            force_hit_suppressed: false,
            hit_test_point: None,
            taffy,
            focused,
            hit_target: None,
            previous_hit_target: None,
            cursor: Rc::new(Cell::new(None)),
            redraw_requested: Rc::new(Cell::new(false)),
            accessibility_announcements: Rc::new(RefCell::new(Vec::new())),
            clipboard: None,
            drag: None,
            window: bounds,
            overlays: None,
            dismiss_pass: false,
        }
    }

    fn hits_suppressed(&self) -> bool {
        self.force_hit_suppressed || self.hit_suppressed
    }

    /// Whether `bounds` should suppress legacy `bounds().contains` for the
    /// active pointer against `hit_clip`.
    fn clip_hit_suppressed_for(
        bounds: Bounds,
        hit_clip: Option<Bounds>,
        point: Option<Point>,
    ) -> bool {
        match (hit_clip, point) {
            (Some(clip), Some(point)) => bounds
                .intersection(&clip)
                .map(|visible| !visible.contains(point))
                .unwrap_or(true),
            _ => false,
        }
    }

    /// Hit-test / legacy containment geometry for this element.
    ///
    /// When pointer dispatch suppresses clipped-away hits, this returns a
    /// never-hit sentinel so `bounds().contains` cannot activate invisible
    /// content. Use [`Self::layout_bounds`] for stable local coordinates.
    pub fn bounds(&self) -> Bounds {
        if self.hits_suppressed() {
            // Never-hit sentinel (negative size); edges are not containable.
            Bounds::from_xywh(0.0, 0.0, -1.0, -1.0)
        } else {
            self.bounds
        }
    }

    /// Stable layout/coordinate geometry for this element.
    ///
    /// Always returns the real bounds so captured Move/Up cleanup and overflow
    /// forwarding can derive local coordinates even when [`Self::bounds`] is
    /// suppressed for legacy hit-testing.
    pub fn layout_bounds(&self) -> Bounds {
        self.bounds
    }

    /// Whether `point` is inside this context's visible pointer region.
    ///
    /// When hit testing is suppressed (clipped cleanup / focus blur through an
    /// unrelated overlay), returns false so modern `contains_pointer` callers
    /// match legacy `bounds().contains` suppression. When a hit clip is
    /// present, requires a positive-area intersection of layout bounds and
    /// clip before accepting the point, so edge-only contacts (inclusive on
    /// both rectangles, zero-area overlap) are rejected.
    pub fn contains_pointer(&self, point: Point) -> bool {
        if self.hits_suppressed() {
            return false;
        }
        match self.hit_clip {
            Some(clip) => match self.bounds.intersection(&clip) {
                Some(visible) => visible.contains(point),
                None => false,
            },
            None => self.bounds.contains(point),
        }
    }

    /// True when a hit clip is present and `point` lies outside that clip.
    ///
    /// This does **not** treat a fully clipped ancestor as proof that all
    /// descendants are invisible: overflow children may still paint inside the
    /// viewport. Per-element activation continues to use
    /// [`Self::contains_pointer`] / [`Self::for_pointer_dispatch`].
    pub fn pointer_outside_hit_region(&self, point: Point) -> bool {
        match self.hit_clip {
            Some(clip) => !clip.contains(point),
            None => false,
        }
    }

    pub fn focused_id(&self) -> Option<ElementId> {
        *self.focused
    }

    pub fn is_focused(&self, id: Option<ElementId>) -> bool {
        id.is_some() && *self.focused == id
    }

    pub fn request_focus(&mut self, id: Option<ElementId>) {
        if let Some(id) = id
            && *self.focused != Some(id)
        {
            self.announce_accessibility(AccessibilityAnnouncement::new(
                id,
                AccessibilityAnnouncementKind::FocusChanged,
                "focus changed",
            ));
        }
        *self.focused = id;
    }

    pub fn clear_focus(&mut self) {
        *self.focused = None;
    }

    pub fn hit_target(&self) -> Option<ElementId> {
        self.hit_target
    }

    pub fn set_hit_target(&mut self, id: Option<ElementId>) {
        self.hit_target = id;
    }

    pub fn previous_hit_target(&self) -> Option<ElementId> {
        self.previous_hit_target
    }

    pub fn set_previous_hit_target(&mut self, id: Option<ElementId>) {
        self.previous_hit_target = id;
    }

    pub fn has_hit_filter(&self) -> bool {
        self.hit_target.is_some() || self.previous_hit_target.is_some()
    }

    pub fn request_redraw(&self) {
        self.redraw_requested.set(true);
    }

    pub fn redraw_requested(&self) -> bool {
        self.redraw_requested.get()
    }

    pub fn set_cursor(&self, cursor: Cursor) {
        self.cursor.set(Some(cursor));
    }

    pub fn cursor(&self) -> Option<Cursor> {
        self.cursor.get()
    }

    pub(crate) fn set_clipboard(&mut self, clipboard: SharedClipboard) {
        self.clipboard = Some(clipboard);
    }

    /// The app clipboard, present while a key action is dispatched.
    pub fn clipboard(&self) -> Option<RefMut<'_, dyn Clipboard + 'static>> {
        self.clipboard.as_deref().map(RefCell::borrow_mut)
    }

    pub(crate) fn set_drag(&mut self, drag: SharedDrag) {
        self.drag = Some(drag);
    }

    /// The window's drag state, present when a presenter dispatches the event.
    pub(crate) fn drag(&self) -> Option<&SharedDrag> {
        self.drag.as_ref()
    }

    /// Whether a drag is past its threshold.
    pub(crate) fn is_dragging(&self) -> bool {
        self.drag
            .as_ref()
            .is_some_and(|drag| drag.borrow().as_ref().is_some_and(ActiveDrag::is_dragging))
    }

    pub fn announce_accessibility(&self, announcement: AccessibilityAnnouncement) {
        self.accessibility_announcements
            .borrow_mut()
            .push(announcement);
    }

    pub fn announce_accessibility_action(&self, id: ElementId, message: impl Into<String>) {
        self.announce_accessibility(AccessibilityAnnouncement::new(
            id,
            AccessibilityAnnouncementKind::ActionFeedback,
            message,
        ));
    }

    pub fn accessibility_announcements(&self) -> Vec<AccessibilityAnnouncement> {
        self.accessibility_announcements.borrow().clone()
    }

    pub fn take_accessibility_announcements(&self) -> Vec<AccessibilityAnnouncement> {
        std::mem::take(&mut *self.accessibility_announcements.borrow_mut())
    }

    pub fn child_bounds(&self, node: NodeId) -> Option<Bounds> {
        let layout = self.taffy.layout(node).ok()?;
        Some(Bounds::from_xywh(
            self.bounds.x() + layout.location.x,
            self.bounds.y() + layout.location.y,
            layout.size.width,
            layout.size.height,
        ))
    }

    pub fn with_bounds(&mut self, bounds: Bounds) -> EventContext<'_> {
        EventContext {
            bounds,
            hit_clip: self.hit_clip,
            // Recompute clip-derived suppression for the child's bounds so
            // concrete TabsRoot/ScrollView-style with_bounds paths suppress
            // legacy bounds().contains without needing AnyElement.
            hit_suppressed: Self::clip_hit_suppressed_for(
                bounds,
                self.hit_clip,
                self.hit_test_point,
            ),
            // Forced blur suppression must survive concrete with_bounds paths
            // (TabsRoot → TabList) that never re-enter AnyElement.
            force_hit_suppressed: self.force_hit_suppressed,
            hit_test_point: self.hit_test_point,
            taffy: self.taffy,
            focused: self.focused,
            hit_target: self.hit_target,
            previous_hit_target: self.previous_hit_target,
            cursor: Rc::clone(&self.cursor),
            redraw_requested: Rc::clone(&self.redraw_requested),
            accessibility_announcements: Rc::clone(&self.accessibility_announcements),
            clipboard: self.clipboard.clone(),
            drag: self.drag.clone(),
            window: self.window,
            overlays: self.overlays.clone(),
            dismiss_pass: self.dismiss_pass,
        }
    }

    /// Keep `bounds` as the layout/coordinate origin and intersect `clip` into
    /// the pointer hit-test region. An empty clip intersection yields a
    /// never-hit region (not `Bounds::ZERO` at the origin).
    pub fn with_bounds_and_hit_clip(&mut self, bounds: Bounds, clip: Bounds) -> EventContext<'_> {
        let hit_clip = match self.hit_clip {
            Some(existing) => existing
                .intersection(&clip)
                .unwrap_or_else(|| Bounds::from_xywh(0.0, 0.0, -1.0, -1.0)),
            None => clip,
        };
        EventContext {
            bounds,
            hit_clip: Some(hit_clip),
            hit_suppressed: Self::clip_hit_suppressed_for(
                bounds,
                Some(hit_clip),
                self.hit_test_point,
            ),
            force_hit_suppressed: self.force_hit_suppressed,
            hit_test_point: self.hit_test_point,
            taffy: self.taffy,
            focused: self.focused,
            hit_target: self.hit_target,
            previous_hit_target: self.previous_hit_target,
            cursor: Rc::clone(&self.cursor),
            redraw_requested: Rc::clone(&self.redraw_requested),
            accessibility_announcements: Rc::clone(&self.accessibility_announcements),
            clipboard: self.clipboard.clone(),
            drag: self.drag.clone(),
            window: self.window,
            overlays: self.overlays.clone(),
            dismiss_pass: self.dismiss_pass,
        }
    }

    /// A context for overlay content at `bounds`: outside every ancestor
    /// clip, as [`PaintContext::paint_overlay`] paints it.
    pub fn for_overlay(&mut self, bounds: Bounds) -> EventContext<'_> {
        let mut cx =
            self.pointer_dispatch_context(false, self.force_hit_suppressed, self.hit_test_point);
        cx.bounds = bounds;
        cx.hit_clip = None;
        cx
    }

    /// The window the tree is laid out in.
    pub fn window(&self) -> Bounds {
        self.window
    }

    pub(crate) fn set_window(&mut self, window: Bounds) {
        self.window = window;
    }

    pub(crate) fn set_dismiss_pass(&mut self, dismiss_pass: bool) {
        self.dismiss_pass = dismiss_pass;
    }

    pub(crate) fn is_dismiss_pass(&self) -> bool {
        self.dismiss_pass
    }

    pub(crate) fn set_overlays(&mut self, overlays: Option<Rc<[Overlay]>>) {
        self.overlays = overlays;
    }

    /// Overlays of the frame the event lands on, topmost last.
    pub fn overlays(&self) -> &[Overlay] {
        self.overlays.as_deref().unwrap_or(&[])
    }

    /// Whether Escape closes the overlay `owner` opened now: it is the topmost
    /// one Escape closes, or no overlays are known, as outside a window.
    pub fn is_escape_target(&self, owner: ElementId) -> bool {
        self.overlays()
            .iter()
            .rev()
            .find(|overlay| overlay.escape)
            .is_none_or(|overlay| overlay.owner == owner)
    }

    /// The topmost modal overlay's owner, which keeps focus inside it.
    pub fn modal_scope(&self) -> Option<ElementId> {
        self.overlays()
            .iter()
            .rev()
            .find(|overlay| overlay.modal)
            .map(|overlay| overlay.owner)
    }

    /// Build a dispatch context that suppresses legacy `bounds().contains` hits
    /// outside the positive-area visible intersection while keeping
    /// [`Self::layout_bounds`] stable for local coordinates.
    pub fn for_pointer_dispatch(&mut self, point: Point) -> EventContext<'_> {
        let hit_suppressed = Self::clip_hit_suppressed_for(self.bounds, self.hit_clip, Some(point));
        self.pointer_dispatch_context(hit_suppressed, self.force_hit_suppressed, Some(point))
    }

    /// Dispatch context that always reports outside for hit testing while
    /// keeping [`Self::layout_bounds`] stable.
    ///
    /// Used for focus blur through an overlay and for Move/Up cleanup when an
    /// unrelated scene hit filter would otherwise drop the event (including
    /// in-viewport releases over a registered sibling).
    pub fn for_blur_dispatch(&mut self) -> EventContext<'_> {
        self.pointer_dispatch_context(true, true, self.hit_test_point)
    }

    fn pointer_dispatch_context(
        &mut self,
        hit_suppressed: bool,
        force_hit_suppressed: bool,
        hit_test_point: Option<Point>,
    ) -> EventContext<'_> {
        EventContext {
            bounds: self.bounds,
            hit_clip: self.hit_clip,
            hit_suppressed,
            force_hit_suppressed,
            hit_test_point,
            taffy: self.taffy,
            focused: self.focused,
            hit_target: self.hit_target,
            previous_hit_target: self.previous_hit_target,
            cursor: Rc::clone(&self.cursor),
            redraw_requested: Rc::clone(&self.redraw_requested),
            accessibility_announcements: Rc::clone(&self.accessibility_announcements),
            clipboard: self.clipboard.clone(),
            drag: self.drag.clone(),
            window: self.window,
            overlays: self.overlays.clone(),
            dismiss_pass: self.dismiss_pass,
        }
    }
}
