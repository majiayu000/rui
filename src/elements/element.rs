//! Core Element trait and related types

use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAnnouncement, AccessibilityAnnouncementKind, AccessibilityContext,
    AccessibilityError, AccessibilityNode,
};
use crate::core::action::{ActionId, ActionOutcome};
use crate::core::app::SharedClipboard;
use crate::core::drag::{ActiveDrag, SharedDrag};
use crate::core::event::{Cursor, KeyEvent, MouseButton, ScrollEvent};
use crate::core::geometry::{Bounds, Point, Size};
use crate::core::style::{Dimension as StyleDimension, Style};
use crate::core::text_editing::{Clipboard, TextInputCommand, TextInputEvent, TextInputSnapshot};
use crate::elements::text::{TextStyle, TextStyleRefinement};
use crate::renderer::text::{RichSpan, RichWrapped, TextMeasureCache, TextRequest, WrappedText};
use crate::renderer::{PaintRecord, Primitive, Scene};
use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use taffy::prelude::*;

mod any;
mod event;
mod layout;
mod paint;
mod reconcile;
pub(crate) use reconcile::reconcile_children;

pub use any::*;
pub use event::*;
pub use layout::*;
pub use paint::*;

/// What the retained frame may assume about an element, declared once instead
/// of through hooks. See [`Element::capabilities`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// Layout and paint depend only on what [`Element::same_props`] compares,
    /// the inherited text style, the children and the size layout gives the
    /// element: never on event, timer or shared state. Such elements, when all
    /// their children are too, keep their layout nodes and replay their paint
    /// from the previous frame while nothing of that changed, and a rebuild
    /// keeps the previous instance when it has the same props and children.
    pub pure: bool,
    /// Pointer moves, releases and scrolls change nothing this element shows
    /// or calls, so dispatch can skip it when its children are inert too. Only
    /// read for [`Capabilities::pure`] elements.
    pub pointer_inert: bool,
}

impl Capabilities {
    /// Pure, and inert to pointer moves, releases and scrolls.
    pub const PURE_INERT: Self = Self {
        pure: true,
        pointer_inert: true,
    };
}

/// The id of an element that needs one for focus, hit regions or
/// accessibility: one the application set, or else the element's stable
/// identity, which layout gives it so the id is the same across rebuilds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StableId {
    id: ElementId,
    explicit: bool,
}

impl StableId {
    /// An ephemeral id, replaced by the stable identity in layout.
    pub fn new() -> Self {
        Self {
            id: ElementId::new(),
            explicit: false,
        }
    }

    /// An id set by the application, which layout keeps.
    pub fn explicit(id: ElementId) -> Self {
        Self { id, explicit: true }
    }

    pub fn get(&self) -> ElementId {
        self.id
    }

    pub fn is_explicit(&self) -> bool {
        self.explicit
    }

    pub(crate) fn adopt(&mut self, id: ElementId) {
        if !self.explicit {
            self.id = id;
        }
    }
}

impl Default for StableId {
    fn default() -> Self {
        Self::new()
    }
}

/// The core Element trait - all UI components implement this
pub trait Element: Any {
    /// Unique identifier for this element (optional)
    fn id(&self) -> Option<ElementId> {
        None
    }

    /// What the retained frame may assume about this element.
    fn capabilities(&self) -> Capabilities {
        Capabilities::default()
    }

    /// For [`Capabilities::pure`] elements: whether `self`, built at the same
    /// place in the next tree as `previous`, lays out and paints exactly as
    /// `previous` did, children aside. The frame pairs and compares the
    /// children itself, and when they match too it keeps `previous`, with the
    /// layout and paint it already did. Elements that are not pure leave it
    /// false.
    fn same_props(&self, _previous: &dyn Element) -> bool {
        false
    }

    /// Where an element that made its own id keeps it. Layout gives it the
    /// element's stable identity unless the application set the id, so focus,
    /// hit regions and accessibility keep pointing at it across rebuilds.
    fn stable_id_mut(&mut self) -> Option<&mut StableId> {
        None
    }

    /// The element's Rust type, for the inspector.
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
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

    /// Structural children in ownership order, including temporarily hidden ones.
    /// Return the same nodes in the same order from `children_mut`. The frame
    /// uses this structure for view refresh, retention and reconciliation.
    /// Containers filter visibility separately in layout, events and accessibility.
    /// Leaf elements may keep the empty default; wrappers must expose their child.
    fn children(&self) -> &[AnyElement] {
        &[]
    }

    /// The children of [`Self::children`], to change. Elements that list
    /// children must return the same ones here, so the frame can reconcile
    /// them and render the views placed among them.
    fn children_mut(&mut self) -> &mut [AnyElement] {
        &mut []
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
