//! A view placed in another tree, rendered only when it changes.

use crate::core::ElementId;
use crate::core::action::{ActionId, ActionOutcome};
use crate::core::app::AppContext;
use crate::core::entity::{Entity, EntityId};
use crate::core::event::{KeyEvent, ScrollEvent};
use crate::core::geometry::Bounds;
use crate::core::style::Style;
use crate::core::text_editing::{TextInputCommand, TextInputEvent, TextInputSnapshot};
use crate::core::view::{View, ViewPlacement};
use crate::elements::element::{
    AnyElement, Capabilities, Element, EventContext, LayoutContext, PaintContext, PointerEvent,
};
use crate::renderer::text::TextMeasureCache;
use std::any::Any;
use std::collections::HashSet;
use std::rc::Rc;
use taffy::prelude::{NodeId, TaffyTree};

/// Where a view made with [`AppContext::new_view`] shows. Placing it renders
/// nothing: the view renders when it is first laid out somewhere, and again only
/// when it or an entity its last render read is notified, without rebuilding
/// the tree it sits in.
///
/// A view must sit where the frame can reach it before layout: in a div or
/// another element that lists its children. Inside a virtual list's rows, which
/// are built during layout, it fails to lay out.
///
/// Siblings that move keep their views by entity: reordering views in a div
/// renders none of them. Once no window's tree places a view, it is released.
pub struct ViewElement {
    view: EntityId,
    style: Style,
    /// What the view last rendered; `None` until it renders here.
    content: Option<AnyElement>,
    /// Keeps the view alive while this element shows it.
    placement: Option<Rc<ViewPlacement>>,
}

/// Place the view `entity` here.
pub fn view<V: View>(entity: Entity<V>) -> ViewElement {
    ViewElement {
        view: entity.id(),
        style: Style::default(),
        content: None,
        placement: None,
    }
}

/// The view `element` shows, when it is a [`ViewElement`].
pub(crate) fn view_of(element: &dyn Element) -> Option<EntityId> {
    (element as &dyn Any)
        .downcast_ref::<ViewElement>()
        .map(|element| element.view)
}

impl ViewElement {
    fn content(&mut self) -> &mut AnyElement {
        match &mut self.content {
            Some(content) => content,
            None => panic!(
                "view {:?} was laid out before it rendered: views cannot sit where \
                 the frame only builds elements during layout, such as virtual list rows",
                self.view
            ),
        }
    }

    /// Render the view if it is in `dirty` or has not rendered here yet, then
    /// the views inside it. See [`crate::elements::element::refresh_views`].
    pub(crate) fn refresh(&mut self, app: &mut AppContext, dirty: &HashSet<EntityId>) -> bool {
        let mut changed = false;
        if self.content.is_none() || dirty.contains(&self.view) {
            if self.placement.is_none() {
                self.placement = Some(app.place_view(self.view));
            }
            let mut next = app.render_child_view(self.view);
            if let Some(previous) = &mut self.content {
                // Keeps the parts that did not change, with their layout and paint.
                next.reconcile(previous);
            }
            self.content = Some(next);
            changed = true;
        }
        if let Some(content) = &mut self.content {
            changed |= content.refresh_views(app, dirty);
        }
        changed
    }
}

impl Element for ViewElement {
    fn style(&self) -> &Style {
        self.content
            .as_ref()
            .map_or(&self.style, |content| content.style())
    }

    fn capabilities(&self) -> Capabilities {
        // Only a wrapper: what it shows records its own layout and paint.
        Capabilities::PURE_INERT
    }

    fn same_props(&self, previous: &dyn Element) -> bool {
        // The same view keeps what it rendered; a refresh renders it again
        // when it changed.
        (previous as &dyn Any)
            .downcast_ref::<ViewElement>()
            .is_some_and(|previous| previous.view == self.view && previous.content.is_some())
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.content().layout(cx)
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        self.content().paint(cx);
    }

    fn unseen_reach(
        &self,
        taffy: &TaffyTree<ElementId>,
        bounds: Bounds,
        visible: &Bounds,
    ) -> Option<Bounds> {
        self.content.as_ref()?.unseen_reach(taffy, bounds, visible)
    }

    fn refresh_text_geometry(&mut self, text_measurer: &mut TextMeasureCache) {
        if let Some(content) = &mut self.content {
            content.refresh_text_geometry(text_measurer);
        }
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        self.content().handle_pointer_event(cx, event)
    }

    fn handle_scroll_event(&mut self, cx: &mut EventContext, event: &ScrollEvent) -> bool {
        self.content().handle_scroll_event(cx, event)
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        self.content().handle_key_event(cx, event)
    }

    fn dispatch_action(&mut self, cx: &mut EventContext, action: &ActionId) -> ActionOutcome {
        self.content().dispatch_action(cx, action)
    }

    fn handle_text_input_event(&mut self, cx: &mut EventContext, event: &TextInputEvent) -> bool {
        self.content().handle_text_input_event(cx, event)
    }

    fn handle_text_input_command(
        &mut self,
        cx: &mut EventContext,
        command: &TextInputCommand,
    ) -> bool {
        self.content().handle_text_input_command(cx, command)
    }

    fn text_input_snapshot(&self, focused: ElementId) -> Option<TextInputSnapshot> {
        self.content
            .as_ref()
            .and_then(|content| content.text_input_snapshot(focused))
    }

    fn handle_window_event(&mut self, event: &crate::core::event::Event) -> bool {
        self.content
            .as_mut()
            .is_some_and(|content| content.handle_window_event(event))
    }

    fn children(&self) -> &[AnyElement] {
        self.content.as_slice()
    }

    fn children_mut(&mut self) -> &mut [AnyElement] {
        self.content.as_mut_slice()
    }
}
