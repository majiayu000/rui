//! Core Element trait and related types

use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAnnouncement, AccessibilityAnnouncementKind, AccessibilityContext,
    AccessibilityError, AccessibilityNode,
};
use crate::core::action::{ActionId, ActionOutcome};
use crate::core::event::{Cursor, KeyEvent, MouseButton, ScrollEvent};
use crate::core::geometry::{Bounds, Point, Size};
use crate::core::style::{Dimension as StyleDimension, Style};
use crate::core::text_editing::{TextInputCommand, TextInputEvent, TextInputSnapshot};
use crate::elements::text::{TextStyle, TextStyleRefinement};
use crate::renderer::text::{RichSpan, RichWrapped, TextMeasureCache, TextRequest, WrappedText};
use crate::renderer::{PaintRecord, Primitive, Scene};
use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use taffy::prelude::*;

/// Source of [`LayoutNodes`] epochs: a new one whenever the kept nodes are
/// started over, so layout recorded against old nodes is never reused.
static NEXT_LAYOUT_EPOCH: AtomicU64 = AtomicU64::new(1);

fn next_layout_epoch() -> u64 {
    NEXT_LAYOUT_EPOCH.fetch_add(1, Ordering::Relaxed)
}

/// Layout context passed during layout phase
pub struct LayoutContext<'a> {
    pub(crate) taffy: &'a mut TaffyTree<ElementId>,
    pub(crate) available_space: Size,
    text_measurer: LayoutTextMeasurer<'a>,
    retained: Option<RetainedPass<'a>>,
    /// Text style inherited by the element being laid out. Shared, so layout
    /// records can keep the one they were laid out with cheaply.
    text_style: Rc<TextStyle>,
    /// Wrapped text in this pass, measured by taffy in [`Self::compute_layout`].
    measured_text: HashMap<NodeId, MeasuredText>,
    /// Wrapped text of the previous retained pass, to dirty nodes whose text
    /// changed while their taffy style did not.
    previous_measured_text: HashMap<NodeId, MeasuredText>,
    /// Identity of the element whose children are being laid out.
    identity_parent: ElementId,
    /// Position the next child laid out under `identity_parent` takes.
    next_child_index: u64,
}

/// Lines of a wrapped text, filled in once layout has given it a width.
pub(crate) type WrappedLines = Rc<RefCell<Option<Arc<WrappedText>>>>;

/// What a wrapped text leaf needs to be measured at any width.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TextWrapRequest {
    pub(crate) content: String,
    pub(crate) font_size: f32,
    pub(crate) font_weight: u16,
    pub(crate) font_family: Option<String>,
    pub(crate) line_height: f32,
    pub(crate) max_lines: Option<usize>,
}

impl TextWrapRequest {
    fn request(&self) -> TextRequest<'_> {
        TextRequest::new(
            &self.content,
            self.font_size,
            self.font_weight,
            self.font_family.as_deref(),
            self.line_height,
        )
    }

    fn wrap(&self, measurer: &mut TextMeasureCache, width: f32) -> Arc<WrappedText> {
        match measurer.wrap_shared(self.request(), width, self.max_lines) {
            Ok(wrapped) => wrapped,
            Err(err) => panic!("text layout failed: {:?}", err),
        }
    }

    fn wrapped_size(&self, measurer: &mut TextMeasureCache, width: f32) -> Size {
        match measurer.wrapped_size(self.request(), width, self.max_lines) {
            Ok(size) => size,
            Err(err) => panic!("text layout failed: {:?}", err),
        }
    }
}

/// Lines of a rich text, filled in once layout has given it a width.
pub(crate) type RichLines = Rc<RefCell<Option<Arc<RichWrapped>>>>;

/// What a rich text leaf needs to be measured at any width.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RichWrapRequest {
    pub(crate) content: String,
    pub(crate) spans: Vec<RichSpan>,
    pub(crate) max_lines: Option<usize>,
}

#[derive(Debug)]
enum MeasuredText {
    Plain {
        request: TextWrapRequest,
        lines: WrappedLines,
    },
    Rich {
        request: RichWrapRequest,
        lines: RichLines,
    },
}

impl MeasuredText {
    fn size(&self, measurer: &mut TextMeasureCache, width: f32) -> Size {
        match self {
            Self::Plain { request, .. } => request.wrapped_size(measurer, width),
            Self::Rich { request, .. } => match measurer.wrapped_rich_size(
                &request.content,
                &request.spans,
                width,
                request.max_lines,
            ) {
                Ok(size) => size,
                Err(err) => panic!("rich text layout failed: {:?}", err),
            },
        }
    }

    /// Break the text at its final `width` and hand the lines to its element.
    fn break_lines(&self, measurer: &mut TextMeasureCache, width: f32) {
        match self {
            Self::Plain { request, lines } => {
                *lines.borrow_mut() = Some(request.wrap(measurer, width));
            }
            Self::Rich { request, lines } => {
                let wrapped =
                    measurer.wrap_rich(&request.content, &request.spans, width, request.max_lines);
                match wrapped {
                    Ok(wrapped) => *lines.borrow_mut() = Some(wrapped),
                    Err(err) => panic!("rich text layout failed: {:?}", err),
                }
            }
        }
    }

    fn same_request(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Plain { request: a, .. }, Self::Plain { request: b, .. }) => a == b,
            (Self::Rich { request: a, .. }, Self::Rich { request: b, .. }) => a == b,
            _ => false,
        }
    }
}

/// Layout nodes kept from one frame to the next.
///
/// A retained layout pass hands out the node created at the same position in the
/// previous pass, in creation order, and only restyles or rewires it when its style
/// or children changed. Taffy then dirties just those nodes and their ancestors, so
/// unchanged subtrees keep their cached layout. A change of tree shape costs more
/// restyling but stays correct: every node's style and children are still written.
#[derive(Debug)]
pub struct LayoutNodes {
    nodes: Vec<NodeId>,
    /// Changes whenever `nodes` are started over.
    epoch: u64,
    /// Node count of the tree after the last retained pass. Anything else means the
    /// tree was changed outside a retained pass and the kept ids are stale.
    tree_node_count: usize,
    measured_text: HashMap<NodeId, MeasuredText>,
}

impl Default for LayoutNodes {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            epoch: next_layout_epoch(),
            tree_node_count: 0,
            measured_text: HashMap::new(),
        }
    }
}

impl LayoutNodes {
    pub fn new() -> Self {
        Self::default()
    }

    /// Nodes created in the last retained pass.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

struct RetainedPass<'a> {
    kept: &'a mut LayoutNodes,
    cursor: usize,
}

impl<'a> RetainedPass<'a> {
    /// Remove nodes the previous pass had and this one did not reach.
    fn finish(self, taffy: &mut TaffyTree<ElementId>) -> &'a mut LayoutNodes {
        for node in self.kept.nodes.drain(self.cursor..) {
            let _ = taffy.remove(node);
        }
        self.kept.tree_node_count = taffy.total_node_count();
        self.kept
    }
}

/// Where a layout pass gets its text measurement cache from.
///
/// Frame-driving callers hold one cache across frames and pass it in as
/// [`LayoutTextMeasurer::Borrowed`], so measurements survive between frames.
/// One-off layout passes own a throwaway cache instead.
enum LayoutTextMeasurer<'a> {
    Owned(Box<TextMeasureCache>),
    Borrowed(&'a mut TextMeasureCache),
}

impl<'a> LayoutContext<'a> {
    pub fn new(taffy: &'a mut TaffyTree<ElementId>, available_space: Size) -> Self {
        Self::with_measurer(
            taffy,
            available_space,
            LayoutTextMeasurer::Owned(Box::default()),
            None,
        )
    }

    fn with_measurer(
        taffy: &'a mut TaffyTree<ElementId>,
        available_space: Size,
        text_measurer: LayoutTextMeasurer<'a>,
        retained: Option<RetainedPass<'a>>,
    ) -> Self {
        Self {
            taffy,
            available_space,
            text_measurer,
            retained,
            text_style: Rc::default(),
            measured_text: HashMap::new(),
            previous_measured_text: HashMap::new(),
            identity_parent: ElementId::ROOT,
            next_child_index: 0,
        }
    }

    /// A pass that reuses the nodes `kept` from the previous pass over `taffy`.
    /// Call [`Self::finish`] once the root is laid out.
    pub fn retained(
        taffy: &'a mut TaffyTree<ElementId>,
        available_space: Size,
        text_measurer: &'a mut TextMeasureCache,
        kept: &'a mut LayoutNodes,
    ) -> Self {
        if kept.tree_node_count != taffy.total_node_count() {
            taffy.clear();
            kept.nodes.clear();
            kept.measured_text.clear();
            kept.epoch = next_layout_epoch();
        }
        let previous_measured_text = std::mem::take(&mut kept.measured_text);
        let mut cx = Self::with_measurer(
            taffy,
            available_space,
            LayoutTextMeasurer::Borrowed(text_measurer),
            Some(RetainedPass { kept, cursor: 0 }),
        );
        cx.previous_measured_text = previous_measured_text;
        cx
    }

    /// End a retained pass, dropping nodes this pass did not reuse.
    ///
    /// Wrapped text is only measured by [`Self::compute_layout`], which also
    /// finishes the pass.
    pub fn finish(mut self) {
        if let Some(pass) = self.retained.take() {
            pass.finish(self.taffy).measured_text = std::mem::take(&mut self.measured_text);
        }
    }

    /// Finish the pass and lay out the tree under `root`, measuring wrapped text
    /// against the width its parent offers it.
    pub fn compute_layout(
        mut self,
        root: NodeId,
        available_space: taffy::Size<AvailableSpace>,
    ) -> taffy::TaffyResult<()> {
        let kept = self.retained.take().map(|pass| pass.finish(self.taffy));
        let measured = &self.measured_text;
        if measured.is_empty() {
            self.taffy.compute_layout(root, available_space)?;
        } else {
            let measurer = match &mut self.text_measurer {
                LayoutTextMeasurer::Owned(cache) => cache,
                LayoutTextMeasurer::Borrowed(cache) => &mut **cache,
            };
            self.taffy.compute_layout_with_measure(
                root,
                available_space,
                |known, available, node, _, _| {
                    let Some(text) = measured.get(&node) else {
                        return taffy::Size::ZERO;
                    };
                    let width = known.width.unwrap_or(match available.width {
                        AvailableSpace::Definite(width) => width,
                        AvailableSpace::MinContent => 0.0,
                        AvailableSpace::MaxContent => f32::INFINITY,
                    });
                    let wrapped = text.size(measurer, width);
                    taffy::Size {
                        width: known.width.unwrap_or(wrapped.width),
                        height: known.height.unwrap_or(wrapped.height),
                    }
                },
            )?;
            // Break each text at its final width; the unrounded one is what
            // the measure above saw.
            for (node, text) in measured {
                let width = self.taffy.unrounded_layout(*node).size.width;
                text.break_lines(measurer, width);
            }
        }
        if let Some(kept) = kept {
            kept.measured_text = std::mem::take(&mut self.measured_text);
        }
        Ok(())
    }

    /// Text style the element being laid out inherits.
    pub fn text_style(&self) -> &TextStyle {
        &self.text_style
    }

    /// Run `f` with `refinement` applied over the inherited text style.
    pub fn with_text_style<R>(
        &mut self,
        refinement: &TextStyleRefinement,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        if refinement.is_empty() {
            return f(self);
        }
        let inherited = Rc::new(self.text_style.refined(refinement));
        let parent = std::mem::replace(&mut self.text_style, inherited);
        let result = f(self);
        self.text_style = parent;
        result
    }

    /// Lay out `child` as the child at `index` of the element being laid out,
    /// instead of at the next position. For elements that build only some of
    /// their children, such as the rows in view, so each keeps its identity.
    pub fn layout_child_at(&mut self, index: u64, child: &mut AnyElement) -> NodeId {
        child.layout_at(self, index)
    }

    /// A layout node without children.
    pub fn new_leaf(&mut self, style: taffy::Style) -> taffy::TaffyResult<NodeId> {
        self.new_with_children(style, &[])
    }

    /// A leaf measured by taffy as text wrapped to the width it is offered.
    /// `lines` receives the final lines in [`Self::compute_layout`].
    pub(crate) fn new_wrapped_text_leaf(
        &mut self,
        style: taffy::Style,
        request: TextWrapRequest,
        lines: WrappedLines,
    ) -> taffy::TaffyResult<NodeId> {
        *lines.borrow_mut() = None;
        self.new_measured_text_leaf(style, MeasuredText::Plain { request, lines })
    }

    /// A leaf measured by taffy as rich text wrapped to the width it is
    /// offered. `lines` receives the final lines in [`Self::compute_layout`].
    pub(crate) fn new_rich_text_leaf(
        &mut self,
        style: taffy::Style,
        request: RichWrapRequest,
        lines: RichLines,
    ) -> taffy::TaffyResult<NodeId> {
        *lines.borrow_mut() = None;
        self.new_measured_text_leaf(style, MeasuredText::Rich { request, lines })
    }

    fn new_measured_text_leaf(
        &mut self,
        style: taffy::Style,
        text: MeasuredText,
    ) -> taffy::TaffyResult<NodeId> {
        let node = self.reuse_or_create(style, &[])?;
        let previous = self.previous_measured_text.remove(&node);
        if previous.is_none_or(|previous| !previous.same_request(&text)) {
            self.taffy.mark_dirty(node)?;
        }
        self.measured_text.insert(node, text);
        Ok(node)
    }

    /// A layout node with `children`, reused from the previous pass when retained.
    pub fn new_with_children(
        &mut self,
        style: taffy::Style,
        children: &[NodeId],
    ) -> taffy::TaffyResult<NodeId> {
        let node = self.reuse_or_create(style, children)?;
        // A node that was wrapped text last pass keeps its measured size cached.
        if !self.previous_measured_text.is_empty()
            && self.previous_measured_text.remove(&node).is_some()
        {
            self.taffy.mark_dirty(node)?;
        }
        Ok(node)
    }

    fn reuse_or_create(
        &mut self,
        style: taffy::Style,
        children: &[NodeId],
    ) -> taffy::TaffyResult<NodeId> {
        let Some(pass) = self.retained.as_mut() else {
            return self.taffy.new_with_children(style, children);
        };
        let Some(&node) = pass.kept.nodes.get(pass.cursor) else {
            // `set_children`, unlike `new_with_children`, detaches each child from
            // the parent it had last pass, which that parent may still list.
            let node = self.taffy.new_leaf(style)?;
            if !children.is_empty() {
                self.taffy.set_children(node, children)?;
            }
            pass.kept.nodes.push(node);
            pass.cursor += 1;
            return Ok(node);
        };
        pass.cursor += 1;
        if *self.taffy.style(node)? != style {
            self.taffy.set_style(node, style)?;
        }
        let same_children = self.taffy.child_count(node) == children.len()
            && children
                .iter()
                .enumerate()
                .all(|(index, child)| self.taffy.child_at_index(node, index).ok() == Some(*child));
        if !same_children {
            self.taffy.set_children(node, children)?;
        }
        Ok(node)
    }

    /// Layout against a text measurement cache owned by the caller, so cached
    /// metrics outlive this single layout pass.
    pub fn with_text_measurer(
        taffy: &'a mut TaffyTree<ElementId>,
        available_space: Size,
        text_measurer: &'a mut TextMeasureCache,
    ) -> Self {
        Self::with_measurer(
            taffy,
            available_space,
            LayoutTextMeasurer::Borrowed(text_measurer),
            None,
        )
    }

    pub fn text_measurer(&mut self) -> &mut TextMeasureCache {
        match &mut self.text_measurer {
            LayoutTextMeasurer::Owned(cache) => cache,
            LayoutTextMeasurer::Borrowed(cache) => cache,
        }
    }
}

/// Paint context passed during paint phase
pub struct PaintContext<'a> {
    pub(crate) scene: &'a mut Scene,
    pub(crate) bounds: Bounds,
    pub(crate) taffy: &'a TaffyTree<ElementId>,
    /// Replay unchanged subtrees from the previous frame and record this one.
    memo: bool,
    /// Unrounded layout of the element being painted, with its origin summed
    /// from the root as taffy sums it when rounding. `None` when the bounds did
    /// not come from [`Self::child_bounds`].
    unrounded: Option<Unrounded>,
    /// The last [`Self::child_bounds`] result and its unrounded layout, which
    /// [`Self::with_bounds`] passes on when it is given those bounds.
    last_child: Cell<Option<(Bounds, Unrounded)>>,
    /// The part of the window that is seen. Unchanged subtrees entirely outside
    /// it and the active clip are not painted. `None` paints everything.
    viewport: Option<Bounds>,
}

/// A node's layout before taffy rounds it.
///
/// Taffy rounds each size against the node's unrounded position from the root,
/// so a subtree's rounded layout stays the same only when that position moves
/// by whole pixels and the unrounded layout inside does not change.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Unrounded {
    origin: Point,
    size: Size,
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
        }
    }
}

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
        }
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
        }
    }
}

/// The core Element trait - all UI components implement this
pub trait Element: Any {
    /// Unique identifier for this element (optional)
    fn id(&self) -> Option<ElementId> {
        None
    }

    /// Called in layout with the element's stable identity. An element that
    /// made its own id with [`ElementId::new`] takes this one instead, so focus,
    /// hit regions and accessibility keep pointing at it across rebuilds. Ids
    /// set by the application are kept.
    fn adopt_stable_id(&mut self, _id: ElementId) {}

    /// Whether layout and paint depend only on what [`Element::reconcile`]
    /// compares, the inherited text style, the children and the size layout
    /// gives the element: never on event, timer or shared state. Such elements,
    /// when all their children are too, keep their layout nodes and replay their
    /// paint from the previous frame while nothing of that changed.
    fn memoizable(&self) -> bool {
        false
    }

    /// Whether pointer moves, releases and scrolls change nothing this element
    /// shows or calls, so dispatch can skip it when its children do too. Only
    /// consulted for [`Element::memoizable`] elements.
    fn ignores_pointer(&self) -> bool {
        false
    }

    /// Called on a freshly built element with the one built at the same place
    /// in the previous tree. Returns true when `self` would lay out and paint
    /// exactly as `previous` did; the caller then keeps `previous`, with the
    /// layout and paint it already did, instead of `self`.
    ///
    /// An element that returns false may still reconcile its children, so the
    /// unchanged ones are kept. One that returns true must leave its children as
    /// they were.
    fn reconcile(&mut self, _previous: &mut dyn Element) -> bool {
        false
    }

    /// Get the element's style
    fn style(&self) -> &Style;

    /// Request layout from Taffy
    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId;

    /// Paint the element to the scene
    fn paint(&mut self, cx: &mut PaintContext);

    /// Refresh text geometry after event dispatch and before painting.
    fn refresh_text_geometry(&mut self, _text_measurer: &mut TextMeasureCache) {}

    /// Handle pointer events (mouse/touch)
    fn handle_pointer_event(&mut self, _cx: &mut EventContext, _event: &PointerEvent) -> bool {
        false
    }

    fn dispatch_pointer_event(
        &mut self,
        cx: &mut EventContext,
        event: &PointerEvent,
    ) -> EventResult {
        EventResult::from_handled(self.handle_pointer_event(cx, event))
    }

    /// Handle scroll wheel events
    fn handle_scroll_event(&mut self, _cx: &mut EventContext, _event: &ScrollEvent) -> bool {
        false
    }

    /// Handle key events
    fn handle_key_event(&mut self, _cx: &mut EventContext, _event: &KeyEvent) -> bool {
        false
    }

    /// Handle typed actions produced by keymaps or platform semantics.
    fn handle_action(&mut self, _cx: &mut EventContext, _action: &ActionId) -> ActionOutcome {
        ActionOutcome::Ignored
    }

    fn dispatch_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        self.handle_action(cx, action)
    }

    /// Handle text input and IME composition events.
    fn handle_text_input_event(&mut self, _cx: &mut EventContext, _event: &TextInputEvent) -> bool {
        false
    }

    #[doc(hidden)]
    fn handle_text_input_command(
        &mut self,
        cx: &mut EventContext,
        command: &TextInputCommand,
    ) -> bool {
        command
            .clone()
            .into_legacy_event()
            .is_some_and(|event| self.handle_text_input_event(cx, &event))
    }

    fn text_input_snapshot(&self, focused: ElementId) -> Option<TextInputSnapshot> {
        self.children()
            .iter()
            .find_map(|child| child.text_input_snapshot(focused))
    }

    /// Handle window events
    fn handle_window_event(&mut self, _event: &crate::core::event::Event) -> bool {
        false
    }

    /// Get child elements
    fn children(&self) -> &[AnyElement] {
        &[]
    }

    /// The children of [`Self::children`], to change. Elements that list
    /// children return them here too, so views placed among them render.
    fn children_mut(&mut self) -> &mut [AnyElement] {
        &mut []
    }

    /// Render the views placed in this subtree that are in `dirty` or have not
    /// rendered yet. Returns whether anything below changed, so what was laid
    /// out and painted for it is not reused.
    fn refresh_views(
        &mut self,
        app: &mut crate::core::app::AppContext,
        dirty: &std::collections::HashSet<crate::core::entity::EntityId>,
    ) -> bool {
        let mut changed = false;
        for child in self.children_mut() {
            changed |= child.refresh_views(app, dirty);
        }
        changed
    }

    fn accessibility(
        &self,
        _cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        Ok(None)
    }

    fn accessibility_nodes(
        &self,
        cx: &AccessibilityContext,
    ) -> Result<Vec<AccessibilityNode>, AccessibilityError> {
        let mut child_nodes = Vec::new();
        for child in self.children() {
            child_nodes.extend(child.accessibility_nodes(cx)?);
        }

        if let Some(node) = self.accessibility(cx)? {
            Ok(vec![node.with_children(child_nodes)])
        } else {
            Ok(child_nodes)
        }
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id() == Some(id) || self.children().iter().any(|child| child.contains_id(id))
    }
}

/// Type-erased element wrapper
pub struct AnyElement {
    inner: Box<dyn Element>,
    stable_id: Option<ElementId>,
    /// Layout of a memoizable subtree, which the next retained pass can keep.
    layout_record: Option<LayoutRecord>,
    /// Paint of a memoizable subtree, which the next frame can replay.
    /// Frame and index of the subtree's [`PaintRecord`] in the scene.
    paint_record: PaintSlot,
}

/// Frame and index of an element's [`PaintRecord`] in the scene, with a flag
/// that fits in the same word.
#[derive(Debug, Clone, Copy)]
struct PaintSlot {
    /// 0 when there is no record; scene frames start at 1.
    frame: u64,
    index: u32,
    /// The subtree holds a view, so a frame that renders views visits it.
    contains_views: bool,
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
    /// The subtree's nodes, in creation order, in [`LayoutNodes`]; its own node
    /// comes last.
    start: u32,
    end: u32,
    text_style: Rc<TextStyle>,
    /// No element in the subtree reacts to pointer moves, releases or scrolls,
    /// so dispatch can pass it by.
    pointer_inert: bool,
}

impl AnyElement {
    pub fn new<E: Element>(element: E) -> Self {
        let mut inner: Box<dyn Element> = Box::new(element);
        // `children_mut` lists hidden children too, which may show later.
        let contains_views = TypeId::of::<E>() == TypeId::of::<crate::elements::ViewElement>()
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
            },
        }
    }

    /// Keep `previous` instead of `self` when it lays out and paints the same.
    /// See [`Element::reconcile`].
    pub fn reconcile(&mut self, previous: &mut AnyElement) -> bool {
        if self.kind() != previous.kind() || !self.inner.reconcile(&mut *previous.inner) {
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

    fn layout_at(&mut self, cx: &mut LayoutContext, index: u64) -> NodeId {
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
        self.inner.adopt_stable_id(id);
        let parent = std::mem::replace(&mut cx.identity_parent, id);
        let next_child_index = std::mem::replace(&mut cx.next_child_index, 0);
        let start = cx.retained.as_ref().map(|pass| pass.cursor);
        let node = self.inner.layout(cx);
        cx.identity_parent = parent;
        cx.next_child_index = next_child_index;
        self.layout_record = match (start, &cx.retained) {
            (Some(start), Some(pass))
                if self.inner.memoizable()
                    && self
                        .children()
                        .iter()
                        .all(|child| child.layout_record.is_some()) =>
            {
                Some(LayoutRecord {
                    epoch: pass.kept.epoch,
                    start: start as u32,
                    end: pass.cursor as u32,
                    text_style: Rc::clone(&cx.text_style),
                    pointer_inert: self.inner.ignores_pointer()
                        && self.children().iter().all(|child| {
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
        let record = self.layout_record.as_ref()?;
        let pass = cx.retained.as_mut()?;
        let (start, end) = (record.start as usize, record.end as usize);
        if pass.kept.epoch != record.epoch
            || pass.cursor != start
            || end > pass.kept.nodes.len()
            || start == end
            || !(Rc::ptr_eq(&cx.text_style, &record.text_style)
                || *cx.text_style == *record.text_style)
        {
            self.layout_record = None;
            return None;
        }
        pass.cursor = end;
        let nodes = &pass.kept.nodes[start..end];
        if !cx.previous_measured_text.is_empty() {
            for node in nodes {
                if let Some(text) = cx.previous_measured_text.remove(node) {
                    cx.measured_text.insert(*node, text);
                }
            }
        }
        nodes.last().copied()
    }

    fn kind(&self) -> TypeId {
        // Deref first: the box's own type id is not the element's.
        (*self.inner).type_id()
    }

    /// See [`Element::refresh_views`]. Subtrees without views are passed by.
    pub fn refresh_views(
        &mut self,
        app: &mut crate::core::app::AppContext,
        dirty: &std::collections::HashSet<crate::core::entity::EntityId>,
    ) -> bool {
        if !self.paint_record.contains_views {
            return false;
        }
        let changed = self.inner.refresh_views(app, dirty);
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
                start: start as u32,
                end: cx.scene.len() as u32,
                origin,
                unrounded_origin: unrounded.origin,
                unrounded_size: unrounded.size,
                reach: cx.scene.reach_of(start..cx.scene.len()),
            });
            self.paint_record.set(Some((frame, index)));
        }
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

/// Trait for types that can be converted into elements
pub trait IntoElement {
    type Element: Element;

    fn into_element(self) -> Self::Element;

    fn into_any_element(self) -> AnyElement
    where
        Self: Sized,
    {
        AnyElement::new(self.into_element())
    }
}

impl<E: Element> IntoElement for E {
    type Element = E;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Trait for renderable components (views)
pub trait Render {
    fn render(&mut self) -> impl Element;
}

fn dimension_to_taffy(dimension: StyleDimension) -> Dimension {
    match dimension {
        StyleDimension::Px(value) => Dimension::Length(value),
        StyleDimension::Percent(value) => Dimension::Percent(value / 100.0),
        StyleDimension::Auto => Dimension::Auto,
        StyleDimension::Fill => Dimension::Percent(1.0),
    }
}

fn dimension_or_px(
    dimension: Option<StyleDimension>,
    pixels: Option<f32>,
    fallback: Dimension,
) -> Dimension {
    dimension
        .map(dimension_to_taffy)
        .or_else(|| pixels.map(Dimension::Length))
        .unwrap_or(fallback)
}

pub(crate) fn style_dimension_or(
    dimension: Option<StyleDimension>,
    pixels: Option<f32>,
    fallback: Dimension,
) -> Dimension {
    dimension_or_px(dimension, pixels, fallback)
}

/// Convert Taffy style to our style
pub fn style_to_taffy(style: &Style) -> taffy::Style {
    taffy::Style {
        display: match style.display {
            crate::core::style::Display::Flex => taffy::Display::Flex,
            crate::core::style::Display::Block => taffy::Display::Block,
            crate::core::style::Display::None => taffy::Display::None,
        },
        position: match style.position {
            crate::core::style::Position::Relative => taffy::Position::Relative,
            crate::core::style::Position::Absolute => taffy::Position::Absolute,
        },
        flex_direction: match style.flex_direction {
            crate::core::style::FlexDirection::Row => taffy::FlexDirection::Row,
            crate::core::style::FlexDirection::Column => taffy::FlexDirection::Column,
            crate::core::style::FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
            crate::core::style::FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
        },
        justify_content: Some(match style.justify_content {
            crate::core::style::JustifyContent::FlexStart => taffy::JustifyContent::FlexStart,
            crate::core::style::JustifyContent::FlexEnd => taffy::JustifyContent::FlexEnd,
            crate::core::style::JustifyContent::Center => taffy::JustifyContent::Center,
            crate::core::style::JustifyContent::SpaceBetween => taffy::JustifyContent::SpaceBetween,
            crate::core::style::JustifyContent::SpaceAround => taffy::JustifyContent::SpaceAround,
            crate::core::style::JustifyContent::SpaceEvenly => taffy::JustifyContent::SpaceEvenly,
        }),
        align_items: Some(match style.align_items {
            crate::core::style::AlignItems::FlexStart => taffy::AlignItems::FlexStart,
            crate::core::style::AlignItems::FlexEnd => taffy::AlignItems::FlexEnd,
            crate::core::style::AlignItems::Center => taffy::AlignItems::Center,
            crate::core::style::AlignItems::Stretch => taffy::AlignItems::Stretch,
            crate::core::style::AlignItems::Baseline => taffy::AlignItems::Baseline,
        }),
        flex_grow: style.flex_grow,
        flex_shrink: style.flex_shrink,
        gap: taffy::Size {
            width: LengthPercentage::Length(style.gap),
            height: LengthPercentage::Length(style.gap),
        },
        size: taffy::Size {
            width: dimension_or_px(style.dimensions.width, style.width, Dimension::Auto),
            height: dimension_or_px(style.dimensions.height, style.height, Dimension::Auto),
        },
        min_size: taffy::Size {
            width: dimension_or_px(style.dimensions.min_width, style.min_width, Dimension::Auto),
            height: dimension_or_px(
                style.dimensions.min_height,
                style.min_height,
                Dimension::Auto,
            ),
        },
        max_size: taffy::Size {
            width: dimension_or_px(style.dimensions.max_width, style.max_width, Dimension::Auto),
            height: dimension_or_px(
                style.dimensions.max_height,
                style.max_height,
                Dimension::Auto,
            ),
        },
        margin: taffy::Rect {
            top: LengthPercentageAuto::Length(style.margin.top),
            right: LengthPercentageAuto::Length(style.margin.right),
            bottom: LengthPercentageAuto::Length(style.margin.bottom),
            left: LengthPercentageAuto::Length(style.margin.left),
        },
        padding: taffy::Rect {
            top: LengthPercentage::Length(style.padding.top),
            right: LengthPercentage::Length(style.padding.right),
            bottom: LengthPercentage::Length(style.padding.bottom),
            left: LengthPercentage::Length(style.padding.left),
        },
        ..Default::default()
    }
}
