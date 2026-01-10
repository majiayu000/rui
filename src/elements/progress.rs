//! Progress bar element for displaying progress

use crate::core::color::Color;
use crate::core::geometry::Bounds;
use crate::core::style::Style;
use crate::core::ElementId;
use crate::elements::element::{style_to_taffy, Element, LayoutContext, PaintContext};
use crate::renderer::Primitive;
use taffy::prelude::*;

/// Progress bar element
pub struct Progress {
    id: Option<ElementId>,
    style: Style,
    /// Progress value (0.0 to 1.0)
    value: f32,
    /// Width of the progress bar in pixels
    width: f32,
    /// Height of the progress bar in pixels
    height: f32,
    /// Character used for filled portion
    filled_char: char,
    /// Character used for empty portion
    empty_char: char,
    /// Color of the filled portion
    color: Color,
    /// Color of the empty/background portion
    background_color: Color,
    /// Whether to show percentage text
    show_percentage: bool,
    layout_node: Option<NodeId>,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            id: None,
            style: Style::new(),
            value: 0.0,
            width: 200.0,
            height: 20.0,
            filled_char: '=',
            empty_char: '-',
            color: Color::GREEN,
            background_color: Color::rgba(0.3, 0.3, 0.3, 1.0),
            show_percentage: true,
            layout_node: None,
        }
    }
}

impl Progress {
    /// Create a new Progress bar with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the element ID
    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the progress value (clamped to 0.0-1.0)
    pub fn value(mut self, value: f32) -> Self {
        self.value = value.clamp(0.0, 1.0);
        self
    }

    /// Get the current progress value
    pub fn get_value(&self) -> f32 {
        self.value
    }

    /// Set the width of the progress bar
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(0.0);
        self
    }

    /// Get the current width
    pub fn get_width(&self) -> f32 {
        self.width
    }

    /// Set the height of the progress bar
    pub fn height(mut self, height: f32) -> Self {
        self.height = height.max(0.0);
        self
    }

    /// Get the current height
    pub fn get_height(&self) -> f32 {
        self.height
    }

    /// Set the character used for the filled portion
    pub fn filled_char(mut self, c: char) -> Self {
        self.filled_char = c;
        self
    }

    /// Get the filled character
    pub fn get_filled_char(&self) -> char {
        self.filled_char
    }

    /// Set the character used for the empty portion
    pub fn empty_char(mut self, c: char) -> Self {
        self.empty_char = c;
        self
    }

    /// Get the empty character
    pub fn get_empty_char(&self) -> char {
        self.empty_char
    }

    /// Set the color of the filled portion
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = color.into();
        self
    }

    /// Get the current color
    pub fn get_color(&self) -> Color {
        self.color
    }

    /// Set the background color
    pub fn background_color(mut self, color: impl Into<Color>) -> Self {
        self.background_color = color.into();
        self
    }

    /// Get the background color
    pub fn get_background_color(&self) -> Color {
        self.background_color
    }

    /// Set whether to show percentage text
    pub fn show_percentage(mut self, show: bool) -> Self {
        self.show_percentage = show;
        self
    }

    /// Get whether percentage is shown
    pub fn get_show_percentage(&self) -> bool {
        self.show_percentage
    }

    /// Render the progress bar as a text string (for terminal-style rendering)
    pub fn render_text(&self, char_width: usize) -> String {
        let filled_count = ((self.value * char_width as f32).round() as usize).min(char_width);
        let empty_count = char_width.saturating_sub(filled_count);

        let bar: String = std::iter::repeat(self.filled_char)
            .take(filled_count)
            .chain(std::iter::repeat(self.empty_char).take(empty_count))
            .collect();

        if self.show_percentage {
            format!("[{}] {:>3}%", bar, (self.value * 100.0).round() as u32)
        } else {
            format!("[{}]", bar)
        }
    }
}

impl Element for Progress {
    fn id(&self) -> Option<ElementId> {
        self.id
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let mut style = style_to_taffy(&self.style);
        style.size = taffy::Size {
            width: Dimension::Length(self.width),
            height: Dimension::Length(self.height),
        };

        let node = cx
            .taffy
            .new_leaf(style)
            .expect("Failed to create progress layout node");

        self.layout_node = Some(node);
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();

        // Draw background (empty portion)
        cx.paint(Primitive::Quad {
            bounds,
            background: self.background_color.to_rgba(),
            border_color: crate::core::color::Rgba::TRANSPARENT,
            border_widths: crate::core::geometry::Edges::ZERO,
            corner_radii: crate::core::style::Corners::all(4.0),
        });

        // Draw filled portion
        if self.value > 0.0 {
            let filled_width = bounds.width() * self.value;
            let filled_bounds = Bounds::new(
                bounds.origin,
                crate::core::geometry::Size::new(filled_width, bounds.height()),
            );

            cx.paint(Primitive::Quad {
                bounds: filled_bounds,
                background: self.color.to_rgba(),
                border_color: crate::core::color::Rgba::TRANSPARENT,
                border_widths: crate::core::geometry::Edges::ZERO,
                corner_radii: crate::core::style::Corners::new(4.0, 0.0, 0.0, 4.0),
            });
        }

        // Draw percentage text if enabled
        if self.show_percentage {
            let percentage_text = format!("{}%", (self.value * 100.0).round() as u32);
            cx.paint(Primitive::Text {
                bounds,
                content: percentage_text,
                color: crate::core::color::Rgba::WHITE,
                font_size: self.height * 0.6,
                font_weight: 500,
                font_family: None,
                line_height: 1.0,
                align: crate::elements::text::TextAlign::Center,
            });
        }
    }
}

/// Create a new Progress element
pub fn progress() -> Progress {
    Progress::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_new() {
        let p = Progress::new();
        assert_eq!(p.get_value(), 0.0);
        assert_eq!(p.get_width(), 200.0);
        assert_eq!(p.get_height(), 20.0);
        assert_eq!(p.get_filled_char(), '=');
        assert_eq!(p.get_empty_char(), '-');
        assert!(p.get_show_percentage());
    }

    #[test]
    fn test_progress_builder_pattern() {
        let p = Progress::new()
            .value(0.5)
            .width(300.0)
            .height(30.0)
            .filled_char('#')
            .empty_char('.')
            .show_percentage(false);

        assert_eq!(p.get_value(), 0.5);
        assert_eq!(p.get_width(), 300.0);
        assert_eq!(p.get_height(), 30.0);
        assert_eq!(p.get_filled_char(), '#');
        assert_eq!(p.get_empty_char(), '.');
        assert!(!p.get_show_percentage());
    }

    #[test]
    fn test_progress_value_clamping_low() {
        let p = Progress::new().value(-0.5);
        assert_eq!(p.get_value(), 0.0);
    }

    #[test]
    fn test_progress_value_clamping_high() {
        let p = Progress::new().value(1.5);
        assert_eq!(p.get_value(), 1.0);
    }

    #[test]
    fn test_progress_value_clamping_valid() {
        let p = Progress::new().value(0.75);
        assert_eq!(p.get_value(), 0.75);
    }

    #[test]
    fn test_progress_value_zero() {
        let p = Progress::new().value(0.0);
        assert_eq!(p.get_value(), 0.0);
    }

    #[test]
    fn test_progress_value_one() {
        let p = Progress::new().value(1.0);
        assert_eq!(p.get_value(), 1.0);
    }

    #[test]
    fn test_progress_width_non_negative() {
        let p = Progress::new().width(-100.0);
        assert_eq!(p.get_width(), 0.0);
    }

    #[test]
    fn test_progress_height_non_negative() {
        let p = Progress::new().height(-50.0);
        assert_eq!(p.get_height(), 0.0);
    }

    #[test]
    fn test_progress_color() {
        let p = Progress::new().color(Color::RED);
        assert_eq!(p.get_color(), Color::RED);
    }

    #[test]
    fn test_progress_background_color() {
        let p = Progress::new().background_color(Color::BLUE);
        assert_eq!(p.get_background_color(), Color::BLUE);
    }

    #[test]
    fn test_progress_render_text_empty() {
        let p = Progress::new()
            .value(0.0)
            .filled_char('=')
            .empty_char('-')
            .show_percentage(true);
        let text = p.render_text(10);
        assert_eq!(text, "[----------]   0%");
    }

    #[test]
    fn test_progress_render_text_full() {
        let p = Progress::new()
            .value(1.0)
            .filled_char('=')
            .empty_char('-')
            .show_percentage(true);
        let text = p.render_text(10);
        assert_eq!(text, "[==========] 100%");
    }

    #[test]
    fn test_progress_render_text_half() {
        let p = Progress::new()
            .value(0.5)
            .filled_char('=')
            .empty_char('-')
            .show_percentage(true);
        let text = p.render_text(10);
        assert_eq!(text, "[=====-----]  50%");
    }

    #[test]
    fn test_progress_render_text_no_percentage() {
        let p = Progress::new()
            .value(0.5)
            .filled_char('#')
            .empty_char('.')
            .show_percentage(false);
        let text = p.render_text(10);
        assert_eq!(text, "[#####.....]");
    }

    #[test]
    fn test_progress_render_text_custom_chars() {
        let p = Progress::new()
            .value(0.3)
            .filled_char('*')
            .empty_char(' ')
            .show_percentage(true);
        let text = p.render_text(10);
        assert_eq!(text, "[***       ]  30%");
    }

    #[test]
    fn test_progress_render_text_different_widths() {
        let p = Progress::new().value(0.5).show_percentage(false);

        let text_5 = p.render_text(5);
        assert!(text_5.contains("=="));
        assert!(text_5.contains("--"));

        let text_20 = p.render_text(20);
        assert!(text_20.len() > text_5.len());
    }

    #[test]
    fn test_progress_id() {
        let p = Progress::new().id(ElementId::from(42u64));
        assert_eq!(Element::id(&p), Some(ElementId::from(42u64)));
    }

    #[test]
    fn test_progress_default_id_is_none() {
        let p = Progress::new();
        assert_eq!(Element::id(&p), None);
    }

    #[test]
    fn test_progress_style() {
        let p = Progress::new();
        let _ = p.style(); // Just verify it doesn't panic
    }

    #[test]
    fn test_progress_function() {
        let p = progress();
        assert_eq!(p.get_value(), 0.0);
    }

    #[test]
    fn test_progress_chained_builder() {
        let p = progress()
            .value(0.25)
            .width(150.0)
            .height(15.0)
            .color(Color::BLUE)
            .background_color(Color::BLACK)
            .filled_char('>')
            .empty_char('<')
            .show_percentage(true);

        assert_eq!(p.get_value(), 0.25);
        assert_eq!(p.get_width(), 150.0);
        assert_eq!(p.get_height(), 15.0);
        assert_eq!(p.get_color(), Color::BLUE);
        assert_eq!(p.get_background_color(), Color::BLACK);
        assert_eq!(p.get_filled_char(), '>');
        assert_eq!(p.get_empty_char(), '<');
        assert!(p.get_show_percentage());
    }

    #[test]
    fn test_progress_render_text_rounding() {
        // Test that 0.33 rounds to 3 chars out of 10
        let p = Progress::new().value(0.33).show_percentage(true);
        let text = p.render_text(10);
        assert!(text.contains("==="));
        assert!(text.contains("33%"));
    }

    #[test]
    fn test_progress_render_text_zero_width() {
        let p = Progress::new().value(0.5).show_percentage(false);
        let text = p.render_text(0);
        assert_eq!(text, "[]");
    }
}
