//! Spinner element for displaying loading animations

use crate::core::color::Color;
use crate::core::style::Style;
use crate::core::ElementId;
use crate::elements::element::{style_to_taffy, Element, LayoutContext, PaintContext};
use crate::renderer::Primitive;
use taffy::prelude::*;

/// Types of spinner animations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpinnerType {
    /// Dots animation: ⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏
    #[default]
    Dots,
    /// Line animation: -\|/
    Line,
    /// Circle animation: ◐◓◑◒
    Circle,
    /// Arrow animation: ←↖↑↗→↘↓↙
    Arrow,
    /// Box animation: ▖▘▝▗
    Box,
    /// Bounce animation: ⠁⠂⠄⠂
    Bounce,
    /// Grow animation: ▁▃▄▅▆▇█▇▆▅▄▃
    Grow,
    /// Star animation: ✶✸✹✺✹✷
    Star,
}

impl SpinnerType {
    /// Get the frames for this spinner type
    pub fn frames(&self) -> &'static [&'static str] {
        match self {
            SpinnerType::Dots => &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
            SpinnerType::Line => &["-", "\\", "|", "/"],
            SpinnerType::Circle => &["◐", "◓", "◑", "◒"],
            SpinnerType::Arrow => &["←", "↖", "↑", "↗", "→", "↘", "↓", "↙"],
            SpinnerType::Box => &["▖", "▘", "▝", "▗"],
            SpinnerType::Bounce => &["⠁", "⠂", "⠄", "⠂"],
            SpinnerType::Grow => &["▁", "▃", "▄", "▅", "▆", "▇", "█", "▇", "▆", "▅", "▄", "▃"],
            SpinnerType::Star => &["✶", "✸", "✹", "✺", "✹", "✷"],
        }
    }

    /// Get the number of frames for this spinner type
    pub fn frame_count(&self) -> usize {
        self.frames().len()
    }

    /// Get a specific frame by index (wraps around)
    pub fn get_frame(&self, index: usize) -> &'static str {
        let frames = self.frames();
        frames[index % frames.len()]
    }
}

/// Spinner element for loading animations
pub struct Spinner {
    id: Option<ElementId>,
    style: Style,
    /// Type of spinner animation
    spinner_type: SpinnerType,
    /// Current animation frame
    frame: usize,
    /// Color of the spinner
    color: Color,
    /// Optional label text
    label: Option<String>,
    /// Font size for rendering
    font_size: f32,
    layout_node: Option<NodeId>,
}

impl Default for Spinner {
    fn default() -> Self {
        Self {
            id: None,
            style: Style::new(),
            spinner_type: SpinnerType::Dots,
            frame: 0,
            color: Color::WHITE,
            label: None,
            font_size: 14.0,
            layout_node: None,
        }
    }
}

impl Spinner {
    /// Create a new Spinner with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the element ID
    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the spinner type
    pub fn spinner_type(mut self, spinner_type: SpinnerType) -> Self {
        self.spinner_type = spinner_type;
        // Reset frame when changing type to avoid out-of-bounds
        self.frame = self.frame % spinner_type.frame_count();
        self
    }

    /// Get the current spinner type
    pub fn get_spinner_type(&self) -> SpinnerType {
        self.spinner_type
    }

    /// Set the current frame
    pub fn frame(mut self, frame: usize) -> Self {
        self.frame = frame % self.spinner_type.frame_count();
        self
    }

    /// Get the current frame index
    pub fn get_frame(&self) -> usize {
        self.frame
    }

    /// Advance to the next frame
    pub fn next_frame(&mut self) {
        self.frame = (self.frame + 1) % self.spinner_type.frame_count();
    }

    /// Get the current frame character
    pub fn current_frame_char(&self) -> &'static str {
        self.spinner_type.get_frame(self.frame)
    }

    /// Set the color
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = color.into();
        self
    }

    /// Get the current color
    pub fn get_color(&self) -> Color {
        self.color
    }

    /// Set the label text
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Get the label
    pub fn get_label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Clear the label
    pub fn clear_label(mut self) -> Self {
        self.label = None;
        self
    }

    /// Set the font size
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size.max(1.0);
        self
    }

    /// Get the font size
    pub fn get_font_size(&self) -> f32 {
        self.font_size
    }

    /// Render the spinner as a text string
    pub fn render_text(&self) -> String {
        let frame_char = self.current_frame_char();
        match &self.label {
            Some(label) => format!("{} {}", frame_char, label),
            None => frame_char.to_string(),
        }
    }

    /// Estimate the width needed for rendering
    fn estimate_width(&self) -> f32 {
        let frame_width = self.font_size; // Approximate width of spinner char
        let label_width = self.label.as_ref().map_or(0.0, |l| l.len() as f32 * self.font_size * 0.5);
        let spacing = if self.label.is_some() { self.font_size * 0.5 } else { 0.0 };
        frame_width + spacing + label_width
    }

    /// Estimate the height needed for rendering
    fn estimate_height(&self) -> f32 {
        self.font_size * 1.4
    }
}

impl Element for Spinner {
    fn id(&self) -> Option<ElementId> {
        self.id
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let width = self.estimate_width();
        let height = self.estimate_height();

        let mut style = style_to_taffy(&self.style);
        style.size = taffy::Size {
            width: Dimension::Length(width),
            height: Dimension::Length(height),
        };

        let node = cx
            .taffy
            .new_leaf(style)
            .expect("Failed to create spinner layout node");

        self.layout_node = Some(node);
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        let content = self.render_text();

        cx.paint(Primitive::Text {
            bounds,
            content,
            color: self.color.to_rgba(),
            font_size: self.font_size,
            font_weight: 400,
            font_family: None,
            line_height: 1.4,
            align: crate::elements::text::TextAlign::Left,
        });
    }
}

/// Create a new Spinner element
pub fn spinner() -> Spinner {
    Spinner::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::element::Element;

    // SpinnerType tests
    #[test]
    fn test_spinner_type_default() {
        let st = SpinnerType::default();
        assert_eq!(st, SpinnerType::Dots);
    }

    #[test]
    fn test_spinner_type_dots_frames() {
        let frames = SpinnerType::Dots.frames();
        assert_eq!(frames.len(), 10);
        assert_eq!(frames[0], "⠋");
    }

    #[test]
    fn test_spinner_type_line_frames() {
        let frames = SpinnerType::Line.frames();
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0], "-");
        assert_eq!(frames[1], "\\");
        assert_eq!(frames[2], "|");
        assert_eq!(frames[3], "/");
    }

    #[test]
    fn test_spinner_type_circle_frames() {
        let frames = SpinnerType::Circle.frames();
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0], "◐");
    }

    #[test]
    fn test_spinner_type_arrow_frames() {
        let frames = SpinnerType::Arrow.frames();
        assert_eq!(frames.len(), 8);
        assert_eq!(frames[0], "←");
    }

    #[test]
    fn test_spinner_type_box_frames() {
        let frames = SpinnerType::Box.frames();
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0], "▖");
    }

    #[test]
    fn test_spinner_type_bounce_frames() {
        let frames = SpinnerType::Bounce.frames();
        assert_eq!(frames.len(), 4);
        assert_eq!(frames[0], "⠁");
    }

    #[test]
    fn test_spinner_type_grow_frames() {
        let frames = SpinnerType::Grow.frames();
        assert_eq!(frames.len(), 12);
        assert_eq!(frames[0], "▁");
    }

    #[test]
    fn test_spinner_type_star_frames() {
        let frames = SpinnerType::Star.frames();
        assert_eq!(frames.len(), 6);
        assert_eq!(frames[0], "✶");
    }

    #[test]
    fn test_spinner_type_frame_count() {
        assert_eq!(SpinnerType::Dots.frame_count(), 10);
        assert_eq!(SpinnerType::Line.frame_count(), 4);
        assert_eq!(SpinnerType::Circle.frame_count(), 4);
        assert_eq!(SpinnerType::Arrow.frame_count(), 8);
        assert_eq!(SpinnerType::Box.frame_count(), 4);
        assert_eq!(SpinnerType::Bounce.frame_count(), 4);
        assert_eq!(SpinnerType::Grow.frame_count(), 12);
        assert_eq!(SpinnerType::Star.frame_count(), 6);
    }

    #[test]
    fn test_spinner_type_get_frame() {
        let st = SpinnerType::Line;
        assert_eq!(st.get_frame(0), "-");
        assert_eq!(st.get_frame(1), "\\");
        assert_eq!(st.get_frame(2), "|");
        assert_eq!(st.get_frame(3), "/");
    }

    #[test]
    fn test_spinner_type_get_frame_wraps() {
        let st = SpinnerType::Line;
        assert_eq!(st.get_frame(4), "-"); // Wraps to 0
        assert_eq!(st.get_frame(5), "\\"); // Wraps to 1
        assert_eq!(st.get_frame(100), "-"); // 100 % 4 = 0
    }

    // Spinner tests
    #[test]
    fn test_spinner_new() {
        let s = Spinner::new();
        assert_eq!(s.get_spinner_type(), SpinnerType::Dots);
        assert_eq!(s.get_frame(), 0);
        assert_eq!(s.get_color(), Color::WHITE);
        assert_eq!(s.get_label(), None);
        assert_eq!(s.get_font_size(), 14.0);
    }

    #[test]
    fn test_spinner_builder_pattern() {
        let s = Spinner::new()
            .spinner_type(SpinnerType::Circle)
            .frame(2)
            .color(Color::GREEN)
            .label("Loading...")
            .font_size(16.0);

        assert_eq!(s.get_spinner_type(), SpinnerType::Circle);
        assert_eq!(s.get_frame(), 2);
        assert_eq!(s.get_color(), Color::GREEN);
        assert_eq!(s.get_label(), Some("Loading..."));
        assert_eq!(s.get_font_size(), 16.0);
    }

    #[test]
    fn test_spinner_type_change_resets_frame() {
        let s = Spinner::new()
            .spinner_type(SpinnerType::Grow) // 12 frames
            .frame(10)
            .spinner_type(SpinnerType::Line); // 4 frames, 10 % 4 = 2

        assert_eq!(s.get_frame(), 2);
    }

    #[test]
    fn test_spinner_frame_wraps() {
        let s = Spinner::new()
            .spinner_type(SpinnerType::Line) // 4 frames
            .frame(10); // 10 % 4 = 2

        assert_eq!(s.get_frame(), 2);
    }

    #[test]
    fn test_spinner_next_frame() {
        let mut s = Spinner::new().spinner_type(SpinnerType::Line);
        assert_eq!(s.get_frame(), 0);

        s.next_frame();
        assert_eq!(s.get_frame(), 1);

        s.next_frame();
        assert_eq!(s.get_frame(), 2);

        s.next_frame();
        assert_eq!(s.get_frame(), 3);

        s.next_frame();
        assert_eq!(s.get_frame(), 0); // Wraps
    }

    #[test]
    fn test_spinner_current_frame_char() {
        let s = Spinner::new()
            .spinner_type(SpinnerType::Line)
            .frame(2);

        assert_eq!(s.current_frame_char(), "|");
    }

    #[test]
    fn test_spinner_color() {
        let s = Spinner::new().color(Color::RED);
        assert_eq!(s.get_color(), Color::RED);
    }

    #[test]
    fn test_spinner_color_from_hex() {
        let s = Spinner::new().color(Color::hex(0xFF00FF));
        // Just verify it doesn't panic
        let _ = s.get_color();
    }

    #[test]
    fn test_spinner_label() {
        let s = Spinner::new().label("Processing");
        assert_eq!(s.get_label(), Some("Processing"));
    }

    #[test]
    fn test_spinner_label_string() {
        let s = Spinner::new().label(String::from("Working"));
        assert_eq!(s.get_label(), Some("Working"));
    }

    #[test]
    fn test_spinner_clear_label() {
        let s = Spinner::new()
            .label("Test")
            .clear_label();
        assert_eq!(s.get_label(), None);
    }

    #[test]
    fn test_spinner_font_size() {
        let s = Spinner::new().font_size(20.0);
        assert_eq!(s.get_font_size(), 20.0);
    }

    #[test]
    fn test_spinner_font_size_minimum() {
        let s = Spinner::new().font_size(-5.0);
        assert_eq!(s.get_font_size(), 1.0);
    }

    #[test]
    fn test_spinner_render_text_no_label() {
        let s = Spinner::new()
            .spinner_type(SpinnerType::Line)
            .frame(0);
        assert_eq!(s.render_text(), "-");
    }

    #[test]
    fn test_spinner_render_text_with_label() {
        let s = Spinner::new()
            .spinner_type(SpinnerType::Line)
            .frame(0)
            .label("Loading");
        assert_eq!(s.render_text(), "- Loading");
    }

    #[test]
    fn test_spinner_render_text_different_frames() {
        let mut s = Spinner::new().spinner_type(SpinnerType::Line);

        assert_eq!(s.render_text(), "-");
        s.next_frame();
        assert_eq!(s.render_text(), "\\");
        s.next_frame();
        assert_eq!(s.render_text(), "|");
        s.next_frame();
        assert_eq!(s.render_text(), "/");
    }

    #[test]
    fn test_spinner_id() {
        let s = Spinner::new().id(ElementId::from(123u64));
        assert_eq!(Element::id(&s), Some(ElementId::from(123u64)));
    }

    #[test]
    fn test_spinner_default_id_is_none() {
        let s = Spinner::new();
        assert_eq!(Element::id(&s), None);
    }

    #[test]
    fn test_spinner_style() {
        let s = Spinner::new();
        let _ = s.style(); // Just verify it doesn't panic
    }

    #[test]
    fn test_spinner_function() {
        let s = spinner();
        assert_eq!(s.get_spinner_type(), SpinnerType::Dots);
    }

    #[test]
    fn test_spinner_chained_builder() {
        let s = spinner()
            .spinner_type(SpinnerType::Star)
            .frame(3)
            .color(Color::BLUE)
            .label("Please wait")
            .font_size(18.0);

        assert_eq!(s.get_spinner_type(), SpinnerType::Star);
        assert_eq!(s.get_frame(), 3);
        assert_eq!(s.get_color(), Color::BLUE);
        assert_eq!(s.get_label(), Some("Please wait"));
        assert_eq!(s.get_font_size(), 18.0);
    }

    #[test]
    fn test_spinner_all_types_render() {
        let types = [
            SpinnerType::Dots,
            SpinnerType::Line,
            SpinnerType::Circle,
            SpinnerType::Arrow,
            SpinnerType::Box,
            SpinnerType::Bounce,
            SpinnerType::Grow,
            SpinnerType::Star,
        ];

        for spinner_type in types {
            let s = Spinner::new().spinner_type(spinner_type);
            let text = s.render_text();
            assert!(!text.is_empty(), "Spinner type {:?} should render non-empty text", spinner_type);
        }
    }

    #[test]
    fn test_spinner_full_cycle() {
        let mut s = Spinner::new().spinner_type(SpinnerType::Line);
        let frame_count = SpinnerType::Line.frame_count();

        // Cycle through all frames and back to start
        for i in 0..frame_count {
            assert_eq!(s.get_frame(), i);
            s.next_frame();
        }
        assert_eq!(s.get_frame(), 0); // Back to start
    }

    #[test]
    fn test_spinner_estimate_dimensions() {
        let s = Spinner::new().font_size(14.0);
        // Just verify the private methods work via layout
        assert!(s.estimate_width() > 0.0);
        assert!(s.estimate_height() > 0.0);
    }

    #[test]
    fn test_spinner_estimate_width_with_label() {
        let s1 = Spinner::new().font_size(14.0);
        let s2 = Spinner::new().font_size(14.0).label("Loading...");

        assert!(s2.estimate_width() > s1.estimate_width());
    }

    #[test]
    fn test_spinner_type_equality() {
        assert_eq!(SpinnerType::Dots, SpinnerType::Dots);
        assert_ne!(SpinnerType::Dots, SpinnerType::Line);
    }

    #[test]
    fn test_spinner_type_clone() {
        let st = SpinnerType::Circle;
        let st_clone = st.clone();
        assert_eq!(st, st_clone);
    }

    #[test]
    fn test_spinner_type_copy() {
        let st = SpinnerType::Arrow;
        let st_copy: SpinnerType = st; // Copy
        assert_eq!(st, st_copy);
    }

    #[test]
    fn test_spinner_type_debug() {
        let st = SpinnerType::Box;
        let debug_str = format!("{:?}", st);
        assert!(debug_str.contains("Box"));
    }
}
