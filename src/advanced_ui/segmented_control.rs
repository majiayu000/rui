use crate::advanced_ui::tokens::{
    control_border_color, control_colors, ControlSize, ControlState, ControlVariant, CONTROL_RADIUS,
};
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Corners, Style};
use crate::core::ElementId;
use crate::elements::element::{
    style_to_taffy, Element, EventContext, LayoutContext, PaintContext, PointerEvent,
    PointerEventKind,
};
use crate::renderer::Primitive;
use taffy::prelude::*;

type SegmentChangeHandler = Box<dyn Fn(&str)>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentedOption {
    value: String,
    label: String,
}

impl SegmentedOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

impl From<(&str, &str)> for SegmentedOption {
    fn from((value, label): (&str, &str)) -> Self {
        Self::new(value, label)
    }
}

pub struct SegmentedControl {
    id: ElementId,
    options: Vec<SegmentedOption>,
    selected: String,
    size: ControlSize,
    state: ControlState,
    hovered_index: Option<usize>,
    pressed_index: Option<usize>,
    style: Style,
    on_change: Option<SegmentChangeHandler>,
}

impl SegmentedControl {
    pub fn new<I, O>(options: I, selected: impl Into<String>) -> Self
    where
        I: IntoIterator<Item = O>,
        O: Into<SegmentedOption>,
    {
        let options: Vec<SegmentedOption> = options.into_iter().map(Into::into).collect();
        let selected = selected.into();
        validate_options(&options, &selected);

        let mut style = Style::new();
        style.border.radius = Corners::all(CONTROL_RADIUS);

        Self {
            id: ElementId::new(),
            options,
            selected,
            size: ControlSize::default(),
            state: ControlState::default(),
            hovered_index: None,
            pressed_index: None,
            style,
            on_change: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self
    }

    pub fn selected(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        validate_selected(&self.options, &value);
        self.selected = value;
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.state.disabled = disabled;
        self
    }

    pub fn on_change(mut self, handler: impl Fn(&str) + 'static) -> Self {
        self.on_change = Some(Box::new(handler));
        self
    }

    pub fn selected_value(&self) -> &str {
        &self.selected
    }

    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    fn option_width(&self) -> f32 {
        self.options
            .iter()
            .map(|option| {
                option.label.chars().count() as f32 * self.size.text_size() * 0.56
                    + self.size.horizontal_padding() * 2.0
            })
            .fold(72.0, f32::max)
    }

    fn index_at(&self, bounds: Bounds, position: crate::core::geometry::Point) -> Option<usize> {
        if !bounds.contains(position) {
            return None;
        }
        let width = bounds.width() / self.options.len() as f32;
        let index = ((position.x - bounds.x()) / width).floor() as usize;
        (index < self.options.len()).then_some(index)
    }
}

impl Element for SegmentedControl {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let mut style = style_to_taffy(&self.style);
        style.size.width = Dimension::Length(self.option_width() * self.options.len() as f32);
        style.size.height = Dimension::Length(self.size.control_height());

        match cx.taffy.new_leaf(style) {
            Ok(node) => node,
            Err(err) => panic!("failed to create advanced segmented control layout node: {}", err),
        }
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        cx.register_hit_region(self.id, bounds);

        cx.paint(Primitive::Quad {
            bounds,
            background: crate::advanced_ui::tokens::surface_color().to_rgba(),
            border_color: control_border_color().to_rgba(),
            border_widths: Edges::all(1.0),
            corner_radii: self.style.border.radius,
        });

        let segment_width = bounds.width() / self.options.len() as f32;
        for (index, option) in self.options.iter().enumerate() {
            let segment_bounds = Bounds::from_xywh(
                bounds.x() + index as f32 * segment_width,
                bounds.y(),
                segment_width,
                bounds.height(),
            );
            let selected = option.value == self.selected;
            let colors = control_colors(
                ControlVariant::Primary,
                ControlState {
                    selected,
                    hovered: self.hovered_index == Some(index),
                    pressed: self.pressed_index == Some(index),
                    disabled: self.state.disabled,
                },
            );

            if selected || self.hovered_index == Some(index) {
                cx.paint(Primitive::Quad {
                    bounds: segment_bounds,
                    background: if selected {
                        colors.background
                    } else {
                        crate::advanced_ui::tokens::disabled_surface_color()
                    }
                    .to_rgba(),
                    border_color: Color::TRANSPARENT.to_rgba(),
                    border_widths: Edges::ZERO,
                    corner_radii: segment_corners(index, self.options.len()),
                });
            }

            cx.paint(Primitive::Text {
                bounds: segment_bounds,
                content: option.label.clone(),
                color: if selected {
                    colors.foreground
                } else {
                    crate::advanced_ui::tokens::text_color()
                }
                .to_rgba(),
                font_size: self.size.text_size(),
                font_weight: if selected { 700 } else { 500 },
                font_family: None,
                line_height: 1.2,
                align: crate::elements::text::TextAlign::Center,
            });
        }
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        if self.state.disabled {
            self.hovered_index = None;
            self.pressed_index = None;
            return false;
        }

        let index = self.index_at(cx.bounds(), event.position);
        match event.kind {
            PointerEventKind::Move => {
                if self.hovered_index != index {
                    self.hovered_index = index;
                    cx.request_redraw();
                }
                if index.is_some() {
                    cx.set_cursor(crate::core::event::Cursor::Pointer);
                }
                false
            }
            PointerEventKind::Down => {
                self.pressed_index = index;
                if index.is_some() {
                    cx.request_redraw();
                }
                index.is_some()
            }
            PointerEventKind::Up => {
                let pressed = self.pressed_index;
                self.pressed_index = None;
                if let (Some(pressed), Some(released)) = (pressed, index)
                    && pressed == released
                {
                    let value = self.options[released].value.clone();
                    if value != self.selected {
                        self.selected = value;
                        if let Some(handler) = &self.on_change {
                            handler(&self.selected);
                        }
                    }
                    cx.request_redraw();
                    return true;
                }
                false
            }
        }
    }
}

fn validate_options(options: &[SegmentedOption], selected: &str) {
    if options.is_empty() {
        panic!("segmented control requires at least one option");
    }
    validate_selected(options, selected);
}

fn validate_selected(options: &[SegmentedOption], selected: &str) {
    if !options.iter().any(|option| option.value == selected) {
        panic!("segmented control selected value must match an option");
    }
}

fn segment_corners(index: usize, len: usize) -> Corners {
    match (index == 0, index + 1 == len) {
        (true, true) => Corners::all(CONTROL_RADIUS),
        (true, false) => Corners::left(CONTROL_RADIUS),
        (false, true) => Corners::right(CONTROL_RADIUS),
        (false, false) => Corners::ZERO,
    }
}

pub fn segmented_control<I, O>(options: I, selected: impl Into<String>) -> SegmentedControl
where
    I: IntoIterator<Item = O>,
    O: Into<SegmentedOption>,
{
    SegmentedControl::new(options, selected)
}

use crate::core::color::Color;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::MouseButton;
    use crate::core::geometry::Point;
    use std::cell::RefCell;
    use std::rc::Rc;
    use taffy::TaffyTree;

    fn pointer(kind: PointerEventKind, x: f32) -> PointerEvent {
        PointerEvent {
            kind,
            position: Point::new(x, 4.0),
            button: Some(MouseButton::Left),
        }
    }

    fn pointer_at(kind: PointerEventKind, x: f32, y: f32) -> PointerEvent {
        PointerEvent {
            kind,
            position: Point::new(x, y),
            button: Some(MouseButton::Left),
        }
    }

    #[test]
    fn advanced_ui_segmented_control_changes_selected_value() {
        let changes = Rc::new(RefCell::new(Vec::<String>::new()));
        let changes_ref = Rc::clone(&changes);
        let mut control = SegmentedControl::new([("list", "List"), ("grid", "Grid")], "list")
            .on_change(move |value| changes_ref.borrow_mut().push(value.to_string()));
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 160.0, 36.0),
            &taffy,
            &mut focused,
        );

        assert!(control.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Down, 120.0)));
        assert!(control.handle_pointer_event(&mut cx, &pointer(PointerEventKind::Up, 120.0)));
        assert_eq!(control.selected_value(), "grid");
        assert_eq!(&*changes.borrow(), &["grid".to_string()]);
    }

    #[test]
    fn advanced_ui_segmented_control_ignores_vertical_miss() {
        let mut control = SegmentedControl::new([("list", "List"), ("grid", "Grid")], "list");
        let taffy = TaffyTree::<ElementId>::new();
        let mut focused = None;
        let mut cx = EventContext::new(
            Bounds::from_xywh(0.0, 0.0, 160.0, 36.0),
            &taffy,
            &mut focused,
        );

        assert!(!control.handle_pointer_event(
            &mut cx,
            &pointer_at(PointerEventKind::Down, 120.0, 80.0),
        ));
        assert_eq!(control.selected_value(), "list");
    }

    #[test]
    #[should_panic(expected = "segmented control selected value must match an option")]
    fn advanced_ui_segmented_control_rejects_missing_selection() {
        let _ = SegmentedControl::new([("list", "List")], "grid");
    }
}
