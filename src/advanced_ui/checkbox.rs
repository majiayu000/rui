use crate::advanced_ui::tokens::{
    control_border_color, control_colors, text_color, ControlSize, ControlState, ControlVariant,
    CONTROL_GAP, CONTROL_RADIUS,
};
use crate::core::event::Cursor;
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Corners, Style};
use crate::core::ElementId;
use crate::elements::element::{
    style_to_taffy, Element, EventContext, LayoutContext, PaintContext, PointerEvent,
    PointerEventKind,
};
use crate::renderer::Primitive;
use taffy::prelude::*;

pub struct Checkbox {
    id: ElementId,
    label: String,
    checked: bool,
    size: ControlSize,
    state: ControlState,
    style: Style,
    on_change: Option<Box<dyn Fn(bool)>>,
}

impl Checkbox {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: ElementId::new(),
            label: label.into(),
            checked: false,
            size: ControlSize::default(),
            state: ControlState::default(),
            style: Style::new(),
            on_change: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn on_change(mut self, handler: impl Fn(bool) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    pub fn is_checked(&self) -> bool {
        self.checked
    }

    pub fn state(&self) -> ControlState {
        self.state
    }

    fn preferred_width(&self) -> f32 {
        self.size.indicator_extent()
            + CONTROL_GAP
            + self.label.chars().count() as f32 * self.size.text_size() * 0.55
    }
}

impl Element for Checkbox {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let mut style = style_to_taffy(&self.style);
        style.size.width = Dimension::Length(self.preferred_width());
        style.size.height = Dimension::Length(self.size.control_height());

        match cx.taffy.new_leaf(style) {
            Ok(node) => node,
            Err(err) => panic!("failed to create advanced checkbox layout node: {}", err),
        }
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        cx.register_hit_region(self.id, bounds);

        let indicator = self.size.indicator_extent();
        let indicator_y = bounds.y() + (bounds.height() - indicator) / 2.0;
        let box_bounds = Bounds::from_xywh(bounds.x(), indicator_y, indicator, indicator);
        let colors = control_colors(
            ControlVariant::Primary,
            ControlState {
                selected: self.checked,
                disabled: self.state.disabled,
                hovered: self.state.hovered,
                pressed: self.state.pressed,
            },
        );

        let box_background = if self.checked {
            colors.background
        } else if self.state.hovered {
            crate::advanced_ui::tokens::disabled_surface_color()
        } else {
            crate::advanced_ui::tokens::surface_color()
        };

        cx.paint(Primitive::Quad {
            bounds: box_bounds,
            background: box_background.to_rgba(),
            border_color: if self.checked { colors.background } else { control_border_color() }.to_rgba(),
            border_widths: Edges::all(1.0),
            corner_radii: Corners::all(CONTROL_RADIUS / 2.0),
        });

        if self.checked {
            cx.paint(Primitive::Text {
                bounds: box_bounds,
                content: "x".to_string(),
                color: colors.foreground.to_rgba(),
                font_size: self.size.text_size(),
                font_weight: 700,
                font_family: None,
                line_height: 1.0,
                align: crate::elements::text::TextAlign::Center,
            });
        }

        let label_bounds = Bounds::from_xywh(
            bounds.x() + indicator + CONTROL_GAP,
            bounds.y(),
            (bounds.width() - indicator - CONTROL_GAP).max(0.0),
            bounds.height(),
        );
        cx.paint(Primitive::Text {
            bounds: label_bounds,
            content: self.label.clone(),
            color: text_color().to_rgba(),
            font_size: self.size.text_size(),
            font_weight: 500,
            font_family: None,
            line_height: 1.2,
            align: crate::elements::text::TextAlign::Left,
        });
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        if self.state.disabled {
            self.state.hovered = false;
            self.state.pressed = false;
            return false;
        }

        let inside = cx.bounds().contains(event.position);
        match event.kind {
            PointerEventKind::Move => {
                if self.state.hovered != inside {
                    self.state.hovered = inside;
                    cx.request_redraw();
                }
                if inside {
                    cx.set_cursor(Cursor::Pointer);
                }
                false
            }
            PointerEventKind::Down => {
                if inside {
                    self.state.pressed = true;
                    cx.request_redraw();
                    true
                } else {
                    false
                }
            }
            PointerEventKind::Up => {
                let was_pressed = self.state.pressed;
                self.state.pressed = false;
                if inside && was_pressed {
                    self.checked = !self.checked;
                    if let Some(handler) = &self.on_change {
                        handler(self.checked);
                    }
                    cx.request_redraw();
                    true
                } else {
                    if was_pressed {
                        cx.request_redraw();
                    }
                    false
                }
            }
        }
    }
}

pub fn checkbox(label: impl Into<String>) -> Checkbox {
    Checkbox::new(label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::MouseButton;
    use crate::core::geometry::Point;
    use std::cell::Cell;
    use std::rc::Rc;
    use taffy::TaffyTree;

    fn pointer(kind: PointerEventKind) -> PointerEvent {
        PointerEvent {
            kind,
            position: Point::new(4.0, 4.0),
            button: Some(MouseButton::Left),
        }
    }

    #[test]
    fn advanced_ui_checkbox_toggles_and_reports_change() {
        let latest = Rc::new(Cell::new(false));
        let latest_ref = Rc::clone(&latest);
        let mut checkbox = Checkbox::new("Enable").on_change(move |checked| latest_ref.set(checked));
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 120.0, 36.0),
            &taffy,
            &mut focused,
        );

        assert!(checkbox.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down)));
        assert!(checkbox.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Up)));
        assert!(checkbox.is_checked());
        assert!(latest.get());
    }

    #[test]
    fn advanced_ui_checkbox_disabled_does_not_toggle() {
        let mut checkbox = Checkbox::new("Enable").disabled(true);
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 120.0, 36.0),
            &taffy,
            &mut focused,
        );

        assert!(!checkbox.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down)));
        assert!(!checkbox.is_checked());
    }
}
