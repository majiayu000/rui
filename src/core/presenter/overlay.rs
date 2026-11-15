//! Overlays at the window level: a press outside a light-dismiss overlay
//! closes it, and focus moves around modal overlays as the WAI-ARIA modal
//! dialog pattern has it: opening one moves focus into it, Tab stays inside
//! it (see [`crate::core::action`]), and closing it gives focus back to the
//! element that had it when the modal opened.

use super::Presenter;
use crate::core::ElementId;
use crate::core::action::{focus_order, scope_contains, sync_focus};
use crate::core::geometry::Bounds;
use crate::elements::Element;
use crate::elements::element::{PointerEvent, PointerEventKind};
use crate::renderer::Overlay;
use std::rc::Rc;

impl<E> Presenter<E> {
    /// The window the last layout filled, which overlays are placed against.
    pub(super) fn window_bounds(&self) -> Bounds {
        let size = self.laid_out_viewport;
        Bounds::from_xywh(0.0, 0.0, size.width, size.height)
    }
}

impl<E> Presenter<E>
where
    E: Element,
{
    /// Before a press is dispatched, let the owners of light-dismiss overlays
    /// it lands outside of see it as a press outside, so they close even when
    /// the control it lands on takes it. Focus is left as it was. Returns
    /// whether that asked for a redraw.
    pub(super) fn dismiss_light_overlays(
        &mut self,
        event: &PointerEvent,
        overlays: &Option<Rc<[Overlay]>>,
        target: Option<ElementId>,
    ) -> bool {
        let light = overlays
            .as_deref()
            .is_some_and(|overlays| overlays.iter().any(|overlay| overlay.light_dismiss));
        // Without a target every element sees the press anyway.
        if !light || target.is_none() || event.kind != PointerEventKind::Down {
            return false;
        }
        let overlays = overlays.clone();
        let (_, redraw) = self.with_event_context(|root, event_cx| {
            let focused = *event_cx.focused;
            event_cx.set_overlays(overlays);
            event_cx.set_hit_target(target);
            event_cx.set_dismiss_pass(true);
            root.dispatch_pointer_event(event_cx, event);
            *event_cx.focused = focused;
        });
        redraw
    }

    /// Move focus for the modals the frame just painted opened or closed.
    /// Returns whether focus moved, which the next frame shows.
    pub(crate) fn sync_modal_focus(&mut self) -> bool {
        let modals: Vec<ElementId> = self
            .scene
            .overlays()
            .iter()
            .filter(|overlay| overlay.modal)
            .map(|overlay| overlay.owner)
            .collect();
        if modals.is_empty() && self.modal_openers.is_empty() {
            return false;
        }

        let mut next = self.focused_element;
        // Closed modals hand focus back to whoever had it before the first of them.
        let mut opener = None;
        while let Some(&(modal, had_focus)) = self.modal_openers.last() {
            if modals.contains(&modal) {
                break;
            }
            self.modal_openers.pop();
            opener = Some(had_focus);
        }
        if let Some(opener) = opener
            && next.is_none_or(|focused| !self.root.contains_id(focused))
        {
            next = opener.filter(|opener| self.root.contains_id(*opener));
        }

        // A modal that opened takes focus, unless it already has it.
        let mut opened = false;
        for modal in &modals {
            if !self.modal_openers.iter().any(|(open, _)| open == modal) {
                self.modal_openers.push((*modal, next));
                opened = true;
            }
        }
        if opened
            && let Some(&top) = modals.last()
            && next.is_none_or(|focused| !scope_contains(&self.root, top, focused))
        {
            let first =
                focus_order(&self.root, next, Some(top)).and_then(|order| order.first().copied());
            next = Some(first.unwrap_or(top));
        }

        if next == self.focused_element {
            return false;
        }
        // Blur first, so the old element's on_blur runs before the new one's on_focus.
        if self.focused_element.is_some() {
            self.focused_element = None;
            self.with_event_context(|root, event_cx| sync_focus(root, event_cx));
        }
        self.focused_element = next;
        if next.is_some() {
            self.with_event_context(|root, event_cx| sync_focus(root, event_cx));
        }
        self.scene_dirty = true;
        true
    }
}
