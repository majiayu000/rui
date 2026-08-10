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
use crate::core::text_editing::{TextInputEvent, TextInputSnapshot};
use crate::renderer::text::TextMeasureCache;
use crate::renderer::{Primitive, Scene};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use taffy::prelude::*;

/// Layout context passed during layout phase
pub struct LayoutContext<'a> {
    pub(crate) taffy: &'a mut TaffyTree<ElementId>,
    pub(crate) available_space: Size,
    text_measurer: LayoutTextMeasurer<'a>,
}

/// Where a layout pass gets its text measurement cache from.
///
/// Frame-driving callers hold one cache across frames and pass it in as
/// [`LayoutTextMeasurer::Borrowed`], so measurements survive between frames.
/// One-off layout passes own a throwaway cache instead.
enum LayoutTextMeasurer<'a> {
    Owned(TextMeasureCache),
    Borrowed(&'a mut TextMeasureCache),
}

impl<'a> LayoutContext<'a> {
    pub fn new(taffy: &'a mut TaffyTree<ElementId>, available_space: Size) -> Self {
        Self {
            taffy,
            available_space,
            text_measurer: LayoutTextMeasurer::Owned(TextMeasureCache::new()),
        }
    }

    /// Layout against a text measurement cache owned by the caller, so cached
    /// metrics outlive this single layout pass.
    pub fn with_text_measurer(
        taffy: &'a mut TaffyTree<ElementId>,
        available_space: Size,
        text_measurer: &'a mut TextMeasureCache,
    ) -> Self {
        Self {
            taffy,
            available_space,
            text_measurer: LayoutTextMeasurer::Borrowed(text_measurer),
        }
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
}

impl<'a> PaintContext<'a> {
    pub fn new(scene: &'a mut Scene, bounds: Bounds, taffy: &'a TaffyTree<ElementId>) -> Self {
        Self {
            scene,
            bounds,
            taffy,
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
        Some(Bounds::from_xywh(
            self.bounds.x() + layout.location.x,
            self.bounds.y() + layout.location.y,
            layout.size.width,
            layout.size.height,
        ))
    }

    pub fn register_hit_region(&mut self, id: ElementId, bounds: Bounds) -> bool {
        self.scene.register_hit_region(id, bounds)
    }

    pub fn register_accessibility_region(&mut self, id: ElementId, bounds: Bounds) {
        self.scene.register_accessibility_region(id, bounds);
    }

    /// Create a child paint context with new bounds
    pub fn with_bounds(&mut self, bounds: Bounds) -> PaintContext<'_> {
        PaintContext {
            scene: self.scene,
            bounds,
            taffy: self.taffy,
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
            taffy,
            focused,
            hit_target: None,
            previous_hit_target: None,
            cursor: Rc::new(Cell::new(None)),
            redraw_requested: Rc::new(Cell::new(false)),
            accessibility_announcements: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn bounds(&self) -> Bounds {
        self.bounds
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
pub trait Element: 'static {
    /// Unique identifier for this element (optional)
    fn id(&self) -> Option<ElementId> {
        None
    }

    /// Get the element's style
    fn style(&self) -> &Style;

    /// Request layout from Taffy
    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId;

    /// Paint the element to the scene
    fn paint(&mut self, cx: &mut PaintContext);

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
}

impl AnyElement {
    pub fn new<E: Element>(element: E) -> Self {
        Self {
            inner: Box::new(element),
        }
    }

    pub fn style(&self) -> &Style {
        self.inner.style()
    }

    pub fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.inner.layout(cx)
    }

    pub fn paint(&mut self, cx: &mut PaintContext) {
        self.inner.paint(cx)
    }

    pub fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let matches_current = cx
            .hit_target()
            .map(|target| self.contains_id(target))
            .unwrap_or(false);
        let matches_previous = cx
            .previous_hit_target()
            .map(|target| self.contains_id(target))
            .unwrap_or(false);

        if cx.has_hit_filter() && !matches_current && !matches_previous {
            return false;
        }

        self.inner.dispatch_pointer_event(cx, event).is_stopped()
    }

    pub fn handle_scroll_event(&mut self, cx: &mut EventContext, event: &ScrollEvent) -> bool {
        self.inner.handle_scroll_event(cx, event)
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
