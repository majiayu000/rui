use crate::core::color::Color;
use crate::core::geometry::Size;
use crate::core::style::Style;
use crate::core::ElementId;
use crate::elements::element::{
    AnyElement, Element, EventContext, LayoutContext, PaintContext, PointerEvent,
};
use crate::elements::{ScrollDirection, ScrollView};
use taffy::prelude::NodeId;

pub struct Scrollable {
    inner: ScrollView,
}

impl Scrollable {
    pub fn new(child: impl Into<AnyElement>) -> Self {
        Self {
            inner: ScrollView::new().child(child),
        }
    }

    pub fn empty() -> Self {
        Self {
            inner: ScrollView::new(),
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.inner = self.inner.id(id);
        self
    }

    pub fn direction(mut self, direction: ScrollDirection) -> Self {
        self.inner = self.inner.direction(direction);
        self
    }

    pub fn vertical(self) -> Self {
        self.direction(ScrollDirection::Vertical)
    }

    pub fn horizontal(self) -> Self {
        self.direction(ScrollDirection::Horizontal)
    }

    pub fn both(self) -> Self {
        self.direction(ScrollDirection::Both)
    }

    pub fn w(mut self, width: f32) -> Self {
        self.inner = self.inner.w(width);
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        self.inner = self.inner.h(height);
        self
    }

    pub fn size(mut self, size: impl Into<Size>) -> Self {
        self.inner = self.inner.size(size);
        self
    }

    pub fn background(mut self, color: impl Into<Color>) -> Self {
        self.inner = self.inner.bg(color);
        self
    }

    pub fn child(mut self, child: impl Into<AnyElement>) -> Self {
        self.inner = self.inner.child(child);
        self
    }

    pub fn children<I, E>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<AnyElement>,
    {
        self.inner = self.inner.children(children);
        self
    }

    pub fn scrollbar_always(mut self) -> Self {
        self.inner = self.inner.scrollbar_always();
        self
    }

    pub fn scrollbar_never(mut self) -> Self {
        self.inner = self.inner.scrollbar_never();
        self
    }

    pub fn on_scroll(mut self, handler: impl Fn(f32, f32) + 'static) -> Self {
        self.inner = self.inner.on_scroll(handler);
        self
    }

    pub fn into_scroll_view(self) -> ScrollView {
        self.inner
    }
}

impl Element for Scrollable {
    fn id(&self) -> Option<ElementId> {
        Element::id(&self.inner)
    }

    fn style(&self) -> &Style {
        Element::style(&self.inner)
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.inner.layout(cx)
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        self.inner.paint(cx);
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        self.inner.handle_pointer_event(cx, event)
    }

    fn handle_scroll_event(
        &mut self,
        cx: &mut EventContext,
        event: &crate::core::event::ScrollEvent,
    ) -> bool {
        self.inner.handle_scroll_event(cx, event)
    }

    fn handle_key_event(
        &mut self,
        cx: &mut EventContext,
        event: &crate::core::event::KeyEvent,
    ) -> bool {
        self.inner.handle_key_event(cx, event)
    }

    fn handle_window_event(&mut self, event: &crate::core::event::Event) -> bool {
        self.inner.handle_window_event(event)
    }

    fn children(&self) -> &[AnyElement] {
        Element::children(&self.inner)
    }
}

pub fn scrollable(child: impl Into<AnyElement>) -> Scrollable {
    Scrollable::new(child)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::advanced_ui::container;
    use crate::core::event::{Modifiers, ScrollEvent};
    use crate::core::geometry::{Bounds, Point};
    use std::cell::Cell;
    use std::rc::Rc;
    use taffy::TaffyTree;

    #[test]
    fn advanced_ui_scrollable_forwards_scroll_events() {
        let did_scroll = Rc::new(Cell::new(false));
        let did_scroll_ref = Rc::clone(&did_scroll);
        let mut scrollable = Scrollable::new(container().w(100.0).h(300.0))
            .h(100.0)
            .on_scroll(move |_, _| did_scroll_ref.set(true));
        let mut taffy = TaffyTree::<ElementId>::new();
        let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(100.0, 100.0));
        let node = scrollable.layout(&mut layout_cx);
        if let Err(err) = taffy.compute_layout(
            node,
            taffy::Size {
                width: taffy::prelude::AvailableSpace::Definite(100.0),
                height: taffy::prelude::AvailableSpace::Definite(100.0),
            },
        ) {
            panic!("layout should compute: {}", err);
        }

        let mut focused = None;
        let mut event_cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 100.0, 100.0),
            &taffy,
            &mut focused,
        );
        assert!(scrollable.handle_scroll_event(
            &mut event_cx,
            &ScrollEvent {
                position: Point::new(4.0, 4.0),
                delta_x: 0.0,
                delta_y: 24.0,
                modifiers: Modifiers::default(),
            },
        ));
        assert!(did_scroll.get());
    }
}
