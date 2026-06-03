use crate::advanced_ui::state::{InteractionState, require_non_empty};
use crate::advanced_ui::tokens::{ControlSize, Theme};
use crate::core::ElementId;
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Corners, Style};
use crate::elements::element::{
    AnyElement, Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind,
};
use crate::renderer::Primitive;
use taffy::prelude::NodeId;

pub struct Tooltip {
    id: ElementId,
    child: AnyElement,
    content: String,
    theme: Theme,
    state: InteractionState,
}

impl Tooltip {
    pub fn new(child: impl Into<AnyElement>, content: impl Into<String>) -> Self {
        let content = content.into();
        require_non_empty(&content, "tooltip content must not be empty");

        Self {
            id: ElementId::new(),
            child: child.into(),
            content,
            theme: Theme::default(),
            state: InteractionState::default(),
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self
    }

    pub fn is_visible(&self) -> bool {
        self.state.hovered()
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.set_disabled(disabled);
        self
    }

    pub fn read_only(mut self, read_only: bool) -> Self {
        self.state.set_read_only(read_only);
        self
    }

    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    pub fn interaction_state(&self) -> InteractionState {
        self.state
    }
}

impl Element for Tooltip {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        self.child.style()
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.child.layout(cx)
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        cx.register_hit_region(self.id, bounds);
        self.child.paint(cx);

        if self.state.hovered() {
            let font_size = self.theme.text_size(ControlSize::Small);
            let padding = self.theme.control_gap();
            let width = self.content.chars().count() as f32 * font_size * 0.58 + padding * 2.0;
            let height = (font_size * self.theme.typography.line_height + padding).max(20.0);
            let tooltip_bounds = Bounds::from_xywh(
                bounds.x(),
                (bounds.y() - height - padding).max(0.0),
                width,
                height,
            );

            cx.paint(Primitive::Quad {
                bounds: tooltip_bounds,
                background: self.theme.colors.tooltip_background.to_rgba(),
                border_color: crate::core::color::Color::TRANSPARENT.to_rgba(),
                border_widths: Edges::ZERO,
                corner_radii: Corners::all(self.theme.control_radius()),
            });
            cx.paint(Primitive::Text {
                bounds: tooltip_bounds,
                content: self.content.clone(),
                color: self.theme.colors.tooltip_text.to_rgba(),
                font_size,
                font_weight: self.theme.typography.label_weight,
                font_family: None,
                line_height: self.theme.typography.line_height,
                align: crate::elements::text::TextAlign::Center,
            });
        }
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        if matches!(event.kind, PointerEventKind::Move) {
            self.state.update_hover(cx.bounds(), event.position, cx);
        }

        if !self.state.can_activate() {
            return false;
        }

        let hit_target = cx.hit_target();
        let previous_hit_target = cx.previous_hit_target();
        cx.set_hit_target(None);
        cx.set_previous_hit_target(None);
        let handled = self.child.handle_pointer_event(cx, event);
        cx.set_hit_target(hit_target);
        cx.set_previous_hit_target(previous_hit_target);
        handled
    }

    fn handle_scroll_event(
        &mut self,
        cx: &mut EventContext,
        event: &crate::core::event::ScrollEvent,
    ) -> bool {
        self.child.handle_scroll_event(cx, event)
    }

    fn handle_key_event(
        &mut self,
        cx: &mut EventContext,
        event: &crate::core::event::KeyEvent,
    ) -> bool {
        self.child.handle_key_event(cx, event)
    }

    fn dispatch_action(
        &mut self,
        cx: &mut EventContext,
        action: &crate::core::action::ActionId,
    ) -> crate::core::action::ActionOutcome {
        self.child.dispatch_action(cx, action)
    }

    fn handle_window_event(&mut self, event: &crate::core::event::Event) -> bool {
        self.child.handle_window_event(event)
    }

    fn contains_id(&self, id: ElementId) -> bool {
        self.id == id || self.child.contains_id(id)
    }
}

pub fn tooltip(child: impl Into<AnyElement>, content: impl Into<String>) -> Tooltip {
    Tooltip::new(child, content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advanced_ui::{button, container};
    use crate::core::event::MouseButton;
    use crate::core::geometry::Point;
    use crate::elements::element::PointerEvent;
    use std::cell::Cell;
    use std::rc::Rc;
    use taffy::TaffyTree;

    #[test]
    fn advanced_ui_tooltip_tracks_hover_state() {
        let mut tooltip = Tooltip::new(container().w(40.0).h(20.0), "Details");
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 40.0, 20.0),
            &taffy,
            &mut focused,
        );

        tooltip.handle_pointer_event(
            &mut cx,
            &PointerEvent {
                kind: PointerEventKind::Move,
                position: Point::new(4.0, 4.0),
                button: None,
            },
        );
        assert!(tooltip.is_visible());
        assert!(tooltip.interaction_state().hovered());
        assert!(cx.redraw_requested());
    }

    #[test]
    fn advanced_ui_tooltip_disabled_does_not_dispatch_child_activation() {
        let clicked = Rc::new(Cell::new(false));
        let clicked_ref = Rc::clone(&clicked);
        let mut tooltip = Tooltip::new(
            button("Open").on_click(move || clicked_ref.set(true)),
            "Open details",
        )
        .disabled(true);
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 80.0, 32.0),
            &taffy,
            &mut focused,
        );

        assert!(!tooltip.handle_pointer_event(
            &mut cx,
            &PointerEvent {
                kind: PointerEventKind::Down,
                position: Point::new(4.0, 4.0),
                button: Some(MouseButton::Left),
            },
        ));
        assert!(!tooltip.handle_pointer_event(
            &mut cx,
            &PointerEvent {
                kind: PointerEventKind::Up,
                position: Point::new(4.0, 4.0),
                button: Some(MouseButton::Left),
            },
        ));
        assert!(!clicked.get());
    }

    #[test]
    #[should_panic(expected = "tooltip content must not be empty")]
    fn advanced_ui_tooltip_rejects_empty_content() {
        drop(Tooltip::new(container(), " "));
    }
}
