use crate::advanced_ui::state::{
    InteractionState, require_finite, require_finite_non_negative, validation_border_color,
};
use crate::advanced_ui::tokens::{CONTROL_RADIUS, ControlSize, control_border_color};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityContext, AccessibilityError, AccessibilityNode, AccessibilityRole,
};
use crate::core::color::Color;
use crate::core::geometry::{Bounds, Edges};
use crate::core::style::{Corners, Dimension as StyleDimension, Style};
use crate::elements::element::{
    Element, LayoutContext, PaintContext, style_dimension_or, style_to_taffy,
};
use crate::renderer::Primitive;
use taffy::prelude::*;

pub struct ProgressBar {
    id: ElementId,
    value: f32,
    size: ControlSize,
    width: StyleDimension,
    show_label: bool,
    accessibility_label: Option<String>,
    color: Color,
    background: Color,
    state: InteractionState,
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
            width: StyleDimension::px(220.0),
            show_label: true,
            accessibility_label: None,
            color: Color::hex(0x2563eb),
            background: Color::hex(0xe5e7eb),
            state: InteractionState::default(),
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
        require_finite_non_negative(
            width,
            "progress bar width must be a finite non-negative value",
        );
        self.width = StyleDimension::px(width);
        self
    }

    pub fn width_percent(mut self, percent: f32) -> Self {
        require_finite_non_negative(
            percent,
            "progress bar width percent must be a finite non-negative value",
        );
        self.width = StyleDimension::percent(percent);
        self
    }

    pub fn width_fill(mut self) -> Self {
        self.width = StyleDimension::fill();
        self
    }

    pub fn show_label(mut self, show_label: bool) -> Self {
        self.show_label = show_label;
        self
    }

    pub fn accessibility_label(mut self, label: impl Into<String>) -> Self {
        let label = label.into();
        crate::advanced_ui::state::require_non_empty(
            &label,
            "progress bar accessibility label must not be empty",
        );
        self.accessibility_label = Some(label);
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

    pub fn invalid(mut self, invalid: bool) -> Self {
        self.state.set_invalid(invalid);
        self
    }

    pub fn interaction_state(&self) -> InteractionState {
        self.state
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
        style.size.width = style_dimension_or(Some(self.width), None, Dimension::Auto);
        style.size.height = Dimension::Length(self.size.control_height() / 2.0);

        match cx.taffy.new_leaf(style) {
            Ok(node) => node,
            Err(err) => panic!(
                "failed to create advanced progress bar layout node: {}",
                err
            ),
        }
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        let radius = self.style.border.radius;

        cx.paint(Primitive::Quad {
            bounds,
            background: self.background.to_rgba(),
            border_color: validation_border_color(self.state.invalid(), control_border_color())
                .to_rgba(),
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

    fn accessibility(
        &self,
        _cx: &AccessibilityContext,
    ) -> Result<Option<AccessibilityNode>, AccessibilityError> {
        let label = self.accessibility_label.as_deref().unwrap_or("Progress");
        Ok(Some(
            AccessibilityNode::label_required(
                self.id,
                AccessibilityRole::ProgressIndicator,
                label,
            )?
            .with_value(format!("{}%", (self.value * 100.0).round() as u32))
            .with_invalid(self.state.invalid()),
        ))
    }
}

fn validate_progress_value(value: f32) {
    require_finite(value, "progress bar value must be finite");
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
        ProgressBar::new(f32::NAN);
    }

    #[test]
    #[should_panic(expected = "progress bar width must be a finite non-negative value")]
    fn advanced_ui_progress_bar_rejects_invalid_width() {
        ProgressBar::new(0.5).width(-1.0);
    }

    #[test]
    fn advanced_ui_progress_bar_preserves_invalid_state_for_style_resolution() {
        let bar = ProgressBar::new(0.5).invalid(true);

        assert!(bar.interaction_state().invalid());
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
        let mut paint_cx =
            PaintContext::new(&mut scene, Bounds::from_xywh(0.0, 0.0, 200.0, 18.0), &taffy);
        bar.paint(&mut paint_cx);

        assert_eq!(scene.primitives().len(), 3);
    }
}
