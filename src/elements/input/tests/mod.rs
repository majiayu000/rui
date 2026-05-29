mod basics;
mod builders;
mod callbacks;
mod presentation;
mod scenarios;
mod state_and_edges;

mod support {
    pub(super) use super::super::*;
    pub(super) use crate::core::event::{KeyCode, KeyEvent, Modifiers};
    pub(super) use crate::core::geometry::Size;
    pub(super) use crate::core::text_editing::{
        MemoryClipboard, TextEditError, TextInputEvent, TextRange,
    };
    pub(super) use crate::renderer::{Primitive, Scene};
    pub(super) use std::cell::RefCell;
    pub(super) use std::rc::Rc;
    pub(super) use taffy::prelude::{AvailableSpace, TaffyTree};

    pub(super) fn focused_context<'a>(
        taffy: &'a TaffyTree<ElementId>,
        focused: &'a mut Option<ElementId>,
    ) -> EventContext<'a> {
        EventContext::new(Bounds::from_xywh(0.0, 0.0, 200.0, 40.0), taffy, focused)
    }

    pub(super) fn typed_char_event(ch: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Unknown(ch as u32), Modifiers::none()).with_char(ch)
    }

    pub(super) fn key_event(key: KeyCode) -> KeyEvent {
        KeyEvent::new(key, Modifiers::none())
    }

    pub(super) fn shifted_key_event(key: KeyCode) -> KeyEvent {
        KeyEvent::new(key, Modifiers::shift())
    }

    pub(super) fn range(start: usize, end: usize) -> TextRange {
        match TextRange::new(start, end) {
            Ok(range) => range,
            Err(err) => panic!("range construction failed: {err}"),
        }
    }

    pub(super) fn painted_primitives(mut input: Input) -> Vec<Primitive> {
        let viewport = Size::new(240.0, 56.0);
        let mut taffy = TaffyTree::new();
        let mut layout_cx = LayoutContext::new(&mut taffy, viewport);
        let node = input.layout(&mut layout_cx);
        if let Err(err) = taffy.compute_layout(
            node,
            taffy::Size {
                width: AvailableSpace::Definite(viewport.width),
                height: AvailableSpace::Definite(viewport.height),
            },
        ) {
            panic!("input test layout failed: {err}");
        }
        let layout = match taffy.layout(node) {
            Ok(layout) => layout,
            Err(err) => panic!("input test layout lookup failed: {err}"),
        };
        let bounds = Bounds::from_xywh(
            layout.location.x,
            layout.location.y,
            layout.size.width,
            layout.size.height,
        );
        let mut scene = Scene::new();
        let mut paint_cx = PaintContext::new(&mut scene, bounds, &taffy);
        input.paint(&mut paint_cx);
        scene.primitives().to_vec()
    }
}
