use crate::advanced_ui::tokens::{control_border_color, ControlSize, CONTROL_RADIUS};
use crate::core::color::Color;
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Corners, Style};
use crate::core::ElementId;
use crate::elements::element::{style_to_taffy, Element, LayoutContext, PaintContext};
use crate::renderer::Primitive;
use taffy::prelude::*;

pub struct ProgressBar {
    id: ElementId,
    value: f32,
    size: ControlSize,
    width: f32,
    show_label: bool,
    color: Color,
    background: Color,
    style: Style,
}

impl ProgressBar {
    pub fn new(value: f32) -> Self {
        validate_progress_value(value);

        let mut style = Style::new();
        style.border.radius = Corners::all(CONTROL_RADIUS);

        Self {
            id: ElementId::new(),
            value: value.clamp(0.0, 1.0),
            size: ControlSize::default(),
            width: 220.0,
            show_label: true,
            color: Color::hex(0x2563eb),
            background: Color::hex(0xe5e7eb),
            style,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = id;
        self
    }

    pub fn value(mut self, value: f32) -> Self {
        validate_progress_value(value);
        self.value = value.clamp(0.0, 1.0);
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        if !width.is_finite() || width < 0.0 {
            panic!("progress bar width must be a finite non-negative value");
        }
        self.width = width;
        self
    }

    pub fn show_label(mut self, show_label: bool) -> Self {
        self.show_label = show_label;
        self
    }

    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = color.into();
        self
    }

    pub fn background(mut self, color: impl Into<Color>) -> Self {
        self.background = color.into();
        self
    }

    pub fn get_value(&self) -> f32 {
        self.value
    }

    pub fn is_label_visible(&self) -> bool {
        self.show_label
    }
}

impl Element for ProgressBar {
    fn id(&self) -> Option<ElementId> {
        Some(self.id)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let mut style = style_to_taffy(&self.style);
        style.size.width = Dimension::Length(self.width);
        style.size.height = Dimension::Length(self.size.control_height() / 2.0);

        match cx.taffy.new_leaf(style) {
            Ok(node) => node,
            Err(err) => panic!("failed to create advanced progress bar layout node: {}", err),
        }
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        let radius = self.style.border.radius;

        cx.paint(Primitive::Quad {
            bounds,
            background: self.background.to_rgba(),
            border_color: control_border_color().to_rgba(),
            border_widths: Edges::all(1.0),
            corner_radii: radius,
        });

        if self.value > 0.0 {
            let filled_width = bounds.width() * self.value;
            cx.paint(Primitive::Quad {
                bounds: Bounds::from_xywh(bounds.x(), bounds.y(), filled_width, bounds.height()),
                background: self.color.to_rgba(),
                border_color: Color::TRANSPARENT.to_rgba(),
                border_widths: Edges::ZERO,
                corner_radii: radius,
            });
        }

        if self.show_label {
            cx.paint(Primitive::Text {
                bounds,
                content: format!("{}%", (self.value * 100.0).round() as u32),
                color: Color::WHITE.to_rgba(),
                font_size: (bounds.height() * 0.62).max(10.0),
                font_weight: 600,
                font_family: None,
                line_height: 1.0,
                align: crate::elements::text::TextAlign::Center,
            });
        }
    }
}

fn validate_progress_value(value: f32) {
    if !value.is_finite() {
        panic!("progress bar value must be finite");
    }
}

pub fn progress_bar(value: f32) -> ProgressBar {
    ProgressBar::new(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::geometry::Size;
    use crate::renderer::Scene;
    use taffy::TaffyTree;

    #[test]
    fn advanced_ui_progress_bar_clamps_value() {
        assert_eq!(ProgressBar::new(-0.25).get_value(), 0.0);
        assert_eq!(ProgressBar::new(1.25).get_value(), 1.0);
    }

    #[test]
    #[should_panic(expected = "progress bar value must be finite")]
    fn advanced_ui_progress_bar_rejects_nan() {
        let _ = ProgressBar::new(f32::NAN);
    }

    #[test]
    fn advanced_ui_progress_bar_paints_track_fill_and_label() {
        let mut bar = ProgressBar::new(0.5);
        let mut taffy = TaffyTree::<ElementId>::new();
        let mut layout_cx = LayoutContext::new(&mut taffy, Size::new(200.0, 40.0));
        let node = bar.layout(&mut layout_cx);
        if let Err(err) = taffy.compute_layout(
            node,
            taffy::Size {
                width: taffy::prelude::AvailableSpace::Definite(200.0),
                height: taffy::prelude::AvailableSpace::Definite(40.0),
            },
        ) {
            panic!("layout should compute: {}", err);
        }

        let mut scene = Scene::new();
        let mut paint_cx = PaintContext::new(
            &mut scene,
            Bounds::from_xywh(0.0, 0.0, 200.0, 18.0),
            &taffy,
        );
        bar.paint(&mut paint_cx);

        assert_eq!(scene.primitives().len(), 3);
    }
}
