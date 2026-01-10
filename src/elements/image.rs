//! Image element for displaying images

use crate::core::color::Color;
use crate::core::geometry::{Bounds, Edges, Size};
use crate::core::style::{Corners, Style};
use crate::core::ElementId;
use crate::elements::element::{style_to_taffy, Element, LayoutContext, PaintContext};
use crate::renderer::Primitive;
use taffy::prelude::*;

/// Image fit mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageFit {
    /// Scale to fill, may crop
    Cover,
    /// Scale to fit, may have letterboxing
    #[default]
    Contain,
    /// Stretch to fill exactly
    Fill,
    /// No scaling
    None,
    /// Scale down only if needed
    ScaleDown,
}

/// Image loading state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageState {
    Loading,
    Loaded,
    Error,
}

/// Image element
pub struct Image {
    id: Option<ElementId>,
    source: ImageSource,
    fit: ImageFit,
    style: Style,
    state: ImageState,
    alt_text: Option<String>,
    placeholder_color: Color,
    texture_id: Option<u32>,
    intrinsic_size: Option<Size>,
    on_load: Option<Box<dyn Fn()>>,
    on_error: Option<Box<dyn Fn()>>,
    layout_node: Option<NodeId>,
}

/// Image source
#[derive(Debug, Clone)]
pub enum ImageSource {
    /// Path to local file
    File(String),
    /// URL to remote image
    Url(String),
    /// Raw pixel data (RGBA)
    Data {
        data: Vec<u8>,
        width: u32,
        height: u32,
    },
    /// Reference to a texture already in GPU memory
    Texture(u32),
}

impl Image {
    pub fn new(source: ImageSource) -> Self {
        Self {
            id: None,
            source,
            fit: ImageFit::default(),
            style: Style::new(),
            state: ImageState::Loading,
            alt_text: None,
            placeholder_color: Color::hex(0xf3f4f6),
            texture_id: None,
            intrinsic_size: None,
            on_load: None,
            on_error: None,
            layout_node: None,
        }
    }

    pub fn from_file(path: impl Into<String>) -> Self {
        Self::new(ImageSource::File(path.into()))
    }

    pub fn from_url(url: impl Into<String>) -> Self {
        Self::new(ImageSource::Url(url.into()))
    }

    pub fn from_data(data: Vec<u8>, width: u32, height: u32) -> Self {
        Self::new(ImageSource::Data {
            data,
            width,
            height,
        })
    }

    pub fn from_texture(texture_id: u32) -> Self {
        Self::new(ImageSource::Texture(texture_id))
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }

    pub fn cover(mut self) -> Self {
        self.fit = ImageFit::Cover;
        self
    }

    pub fn contain(mut self) -> Self {
        self.fit = ImageFit::Contain;
        self
    }

    pub fn fill(mut self) -> Self {
        self.fit = ImageFit::Fill;
        self
    }

    pub fn w(mut self, width: f32) -> Self {
        self.style.width = Some(width);
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        self.style.height = Some(height);
        self
    }

    pub fn size(mut self, size: impl Into<Size>) -> Self {
        let s = size.into();
        self.style.width = Some(s.width);
        self.style.height = Some(s.height);
        self
    }

    pub fn rounded(mut self, radius: f32) -> Self {
        self.style.border.radius = Corners::all(radius);
        self
    }

    pub fn rounded_full(mut self) -> Self {
        self.style.border.radius = Corners::all(9999.0);
        self
    }

    pub fn alt(mut self, text: impl Into<String>) -> Self {
        self.alt_text = Some(text.into());
        self
    }

    pub fn placeholder(mut self, color: impl Into<Color>) -> Self {
        self.placeholder_color = color.into();
        self
    }

    pub fn on_load(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_load = Some(Box::new(handler));
        self
    }

    pub fn on_error(mut self, handler: impl Fn() + 'static) -> Self {
        self.on_error = Some(Box::new(handler));
        self
    }

    /// Calculate the destination bounds based on fit mode
    fn calculate_dest_bounds(&self, container: Bounds, image_size: Size) -> Bounds {
        match self.fit {
            ImageFit::Fill => container,
            ImageFit::Contain => {
                let scale_x = container.width() / image_size.width;
                let scale_y = container.height() / image_size.height;
                let scale = scale_x.min(scale_y);
                let width = image_size.width * scale;
                let height = image_size.height * scale;
                let x = container.x() + (container.width() - width) / 2.0;
                let y = container.y() + (container.height() - height) / 2.0;
                Bounds::from_xywh(x, y, width, height)
            }
            ImageFit::Cover => {
                let scale_x = container.width() / image_size.width;
                let scale_y = container.height() / image_size.height;
                let scale = scale_x.max(scale_y);
                let width = image_size.width * scale;
                let height = image_size.height * scale;
                let x = container.x() + (container.width() - width) / 2.0;
                let y = container.y() + (container.height() - height) / 2.0;
                Bounds::from_xywh(x, y, width, height)
            }
            ImageFit::None => {
                let x = container.x() + (container.width() - image_size.width) / 2.0;
                let y = container.y() + (container.height() - image_size.height) / 2.0;
                Bounds::from_xywh(x, y, image_size.width, image_size.height)
            }
            ImageFit::ScaleDown => {
                if image_size.width <= container.width() && image_size.height <= container.height()
                {
                    // No scaling needed
                    let x = container.x() + (container.width() - image_size.width) / 2.0;
                    let y = container.y() + (container.height() - image_size.height) / 2.0;
                    Bounds::from_xywh(x, y, image_size.width, image_size.height)
                } else {
                    // Scale down like contain
                    let scale_x = container.width() / image_size.width;
                    let scale_y = container.height() / image_size.height;
                    let scale = scale_x.min(scale_y);
                    let width = image_size.width * scale;
                    let height = image_size.height * scale;
                    let x = container.x() + (container.width() - width) / 2.0;
                    let y = container.y() + (container.height() - height) / 2.0;
                    Bounds::from_xywh(x, y, width, height)
                }
            }
        }
    }
}

impl Element for Image {
    fn id(&self) -> Option<ElementId> {
        self.id
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let mut style = style_to_taffy(&self.style);

        // Use intrinsic size if no explicit size set
        if self.style.width.is_none() && self.style.height.is_none() {
            if let Some(intrinsic) = self.intrinsic_size {
                style.size = taffy::Size {
                    width: Dimension::Length(intrinsic.width),
                    height: Dimension::Length(intrinsic.height),
                };
            }
        }

        let node = cx
            .taffy
            .new_leaf(style)
            .expect("Failed to create image layout node");
        self.layout_node = Some(node);
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();

        // Paint placeholder/background
        cx.paint(Primitive::Quad {
            bounds,
            background: self.placeholder_color.to_rgba(),
            border_color: crate::core::color::Rgba::TRANSPARENT,
            border_widths: Edges::ZERO,
            corner_radii: self.style.border.radius,
        });

        // Paint image if loaded
        if let Some(texture_id) = self.texture_id {
            let image_size = self.intrinsic_size.unwrap_or(bounds.size);
            let dest_bounds = self.calculate_dest_bounds(bounds, image_size);

            // Push clip for rounded corners
            if !self.style.border.radius.is_zero() {
                cx.scene.push_layer(bounds);
            }

            cx.paint(Primitive::Image {
                bounds: dest_bounds,
                texture_id,
                corner_radii: self.style.border.radius,
                opacity: self.style.opacity,
            });

            if !self.style.border.radius.is_zero() {
                cx.scene.pop_layer();
            }
        }

        // Show alt text on error
        if self.state == ImageState::Error {
            if let Some(ref alt) = self.alt_text {
                let text_x = bounds.x() + 8.0;
                let text_y = bounds.y() + bounds.height() / 2.0 - 7.0;
                cx.paint(Primitive::Text {
                    bounds: Bounds::from_xywh(text_x, text_y, bounds.width() - 16.0, 14.0),
                    content: alt.clone(),
                    color: Color::hex(0x6b7280).to_rgba(),
                    font_size: 12.0,
                    font_weight: 400,
                    font_family: None,
                    line_height: 1.2,
                    align: crate::elements::text::TextAlign::Center,
                });
            }
        }
    }
}

/// Create an image from file path
pub fn image(source: impl Into<String>) -> Image {
    Image::from_file(source)
}

/// Create an image from URL
pub fn image_url(url: impl Into<String>) -> Image {
    Image::from_url(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elements::element::Element;

    // ==================== ImageFit Enum Tests ====================

    #[test]
    fn test_image_fit_default() {
        let fit: ImageFit = ImageFit::default();
        assert_eq!(fit, ImageFit::Contain);
    }

    #[test]
    fn test_image_fit_variants() {
        let test_cases = [
            (ImageFit::Cover, "Cover"),
            (ImageFit::Contain, "Contain"),
            (ImageFit::Fill, "Fill"),
            (ImageFit::None, "None"),
            (ImageFit::ScaleDown, "ScaleDown"),
        ];

        for (fit, expected_name) in test_cases {
            let debug_str = format!("{:?}", fit);
            assert!(
                debug_str.contains(expected_name),
                "Expected {} in {:?}",
                expected_name,
                debug_str
            );
        }
    }

    #[test]
    fn test_image_fit_equality() {
        assert_eq!(ImageFit::Cover, ImageFit::Cover);
        assert_eq!(ImageFit::Contain, ImageFit::Contain);
        assert_ne!(ImageFit::Cover, ImageFit::Contain);
        assert_ne!(ImageFit::Fill, ImageFit::None);
    }

    #[test]
    fn test_image_fit_clone() {
        let fit = ImageFit::Cover;
        let cloned = fit.clone();
        assert_eq!(fit, cloned);
    }

    #[test]
    fn test_image_fit_copy() {
        let fit = ImageFit::ScaleDown;
        let copied = fit;
        assert_eq!(fit, copied);
    }

    // ==================== ImageState Enum Tests ====================

    #[test]
    fn test_image_state_variants() {
        let test_cases = [
            (ImageState::Loading, "Loading"),
            (ImageState::Loaded, "Loaded"),
            (ImageState::Error, "Error"),
        ];

        for (state, expected_name) in test_cases {
            let debug_str = format!("{:?}", state);
            assert!(debug_str.contains(expected_name));
        }
    }

    #[test]
    fn test_image_state_equality() {
        assert_eq!(ImageState::Loading, ImageState::Loading);
        assert_eq!(ImageState::Loaded, ImageState::Loaded);
        assert_eq!(ImageState::Error, ImageState::Error);
        assert_ne!(ImageState::Loading, ImageState::Loaded);
        assert_ne!(ImageState::Loaded, ImageState::Error);
    }

    // ==================== ImageSource Enum Tests ====================

    #[test]
    fn test_image_source_file() {
        let source = ImageSource::File("test.png".to_string());
        if let ImageSource::File(path) = source {
            assert_eq!(path, "test.png");
        } else {
            panic!("Expected File variant");
        }
    }

    #[test]
    fn test_image_source_url() {
        let source = ImageSource::Url("https://example.com/image.png".to_string());
        if let ImageSource::Url(url) = source {
            assert_eq!(url, "https://example.com/image.png");
        } else {
            panic!("Expected Url variant");
        }
    }

    #[test]
    fn test_image_source_data() {
        let data = vec![255u8, 0, 0, 255]; // One RGBA pixel
        let source = ImageSource::Data {
            data: data.clone(),
            width: 1,
            height: 1,
        };
        if let ImageSource::Data {
            data: d,
            width,
            height,
        } = source
        {
            assert_eq!(d, data);
            assert_eq!(width, 1);
            assert_eq!(height, 1);
        } else {
            panic!("Expected Data variant");
        }
    }

    #[test]
    fn test_image_source_texture() {
        let source = ImageSource::Texture(42);
        if let ImageSource::Texture(id) = source {
            assert_eq!(id, 42);
        } else {
            panic!("Expected Texture variant");
        }
    }

    #[test]
    fn test_image_source_clone() {
        let source = ImageSource::File("test.png".to_string());
        let cloned = source.clone();
        if let (ImageSource::File(p1), ImageSource::File(p2)) = (source, cloned) {
            assert_eq!(p1, p2);
        } else {
            panic!("Clone failed");
        }
    }

    // ==================== Image Constructor Tests ====================

    #[test]
    fn test_image_new() {
        let source = ImageSource::File("test.png".to_string());
        let image = Image::new(source);

        assert!(image.id.is_none());
        assert_eq!(image.fit, ImageFit::Contain);
        assert_eq!(image.state, ImageState::Loading);
        assert!(image.alt_text.is_none());
        assert!(image.texture_id.is_none());
        assert!(image.intrinsic_size.is_none());
        assert!(image.layout_node.is_none());
    }

    #[test]
    fn test_image_from_file() {
        let image = Image::from_file("assets/logo.png");

        if let ImageSource::File(path) = &image.source {
            assert_eq!(path, "assets/logo.png");
        } else {
            panic!("Expected File source");
        }
    }

    #[test]
    fn test_image_from_file_with_string() {
        let path = String::from("assets/logo.png");
        let image = Image::from_file(path);

        if let ImageSource::File(p) = &image.source {
            assert_eq!(p, "assets/logo.png");
        } else {
            panic!("Expected File source");
        }
    }

    #[test]
    fn test_image_from_url() {
        let image = Image::from_url("https://example.com/image.jpg");

        if let ImageSource::Url(url) = &image.source {
            assert_eq!(url, "https://example.com/image.jpg");
        } else {
            panic!("Expected Url source");
        }
    }

    #[test]
    fn test_image_from_data() {
        let pixel_data = vec![255u8, 128, 64, 255, 0, 0, 0, 255];
        let image = Image::from_data(pixel_data.clone(), 2, 1);

        if let ImageSource::Data {
            data,
            width,
            height,
        } = &image.source
        {
            assert_eq!(*data, pixel_data);
            assert_eq!(*width, 2);
            assert_eq!(*height, 1);
        } else {
            panic!("Expected Data source");
        }
    }

    #[test]
    fn test_image_from_texture() {
        let image = Image::from_texture(123);

        if let ImageSource::Texture(id) = &image.source {
            assert_eq!(*id, 123);
        } else {
            panic!("Expected Texture source");
        }
    }

    // ==================== Builder Method Tests ====================

    #[test]
    fn test_image_id() {
        let id = ElementId(42);
        let image = Image::from_file("test.png").id(id);

        assert_eq!(image.id, Some(id));
    }

    #[test]
    fn test_image_fit_method() {
        let test_cases = [
            (ImageFit::Cover, ImageFit::Cover),
            (ImageFit::Contain, ImageFit::Contain),
            (ImageFit::Fill, ImageFit::Fill),
            (ImageFit::None, ImageFit::None),
            (ImageFit::ScaleDown, ImageFit::ScaleDown),
        ];

        for (input, expected) in test_cases {
            let image = Image::from_file("test.png").fit(input);
            assert_eq!(image.fit, expected);
        }
    }

    #[test]
    fn test_image_cover() {
        let image = Image::from_file("test.png").cover();
        assert_eq!(image.fit, ImageFit::Cover);
    }

    #[test]
    fn test_image_contain() {
        let image = Image::from_file("test.png").contain();
        assert_eq!(image.fit, ImageFit::Contain);
    }

    #[test]
    fn test_image_fill() {
        let image = Image::from_file("test.png").fill();
        assert_eq!(image.fit, ImageFit::Fill);
    }

    #[test]
    fn test_image_w() {
        let image = Image::from_file("test.png").w(200.0);
        assert_eq!(image.style.width, Some(200.0));
    }

    #[test]
    fn test_image_h() {
        let image = Image::from_file("test.png").h(150.0);
        assert_eq!(image.style.height, Some(150.0));
    }

    #[test]
    fn test_image_size_tuple() {
        let image = Image::from_file("test.png").size((300.0, 200.0));
        assert_eq!(image.style.width, Some(300.0));
        assert_eq!(image.style.height, Some(200.0));
    }

    #[test]
    fn test_image_size_struct() {
        let size = Size::new(400.0, 300.0);
        let image = Image::from_file("test.png").size(size);
        assert_eq!(image.style.width, Some(400.0));
        assert_eq!(image.style.height, Some(300.0));
    }

    #[test]
    fn test_image_rounded() {
        let image = Image::from_file("test.png").rounded(8.0);
        let radius = image.style.border.radius;
        assert_eq!(radius.top_left, 8.0);
        assert_eq!(radius.top_right, 8.0);
        assert_eq!(radius.bottom_right, 8.0);
        assert_eq!(radius.bottom_left, 8.0);
    }

    #[test]
    fn test_image_rounded_full() {
        let image = Image::from_file("test.png").rounded_full();
        let radius = image.style.border.radius;
        assert_eq!(radius.top_left, 9999.0);
        assert_eq!(radius.top_right, 9999.0);
        assert_eq!(radius.bottom_right, 9999.0);
        assert_eq!(radius.bottom_left, 9999.0);
    }

    #[test]
    fn test_image_alt() {
        let image = Image::from_file("test.png").alt("A test image");
        assert_eq!(image.alt_text, Some("A test image".to_string()));
    }

    #[test]
    fn test_image_alt_with_string() {
        let alt = String::from("Description");
        let image = Image::from_file("test.png").alt(alt);
        assert_eq!(image.alt_text, Some("Description".to_string()));
    }

    #[test]
    fn test_image_placeholder() {
        let image = Image::from_file("test.png").placeholder(Color::RED);
        assert_eq!(image.placeholder_color, Color::RED);
    }

    #[test]
    fn test_image_placeholder_hex() {
        let image = Image::from_file("test.png").placeholder(Color::hex(0xFF00FF));
        let expected = Color::hex(0xFF00FF);
        assert_eq!(image.placeholder_color, expected);
    }

    #[test]
    fn test_image_on_load() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let image = Image::from_file("test.png").on_load(move || {
            called_clone.store(true, Ordering::SeqCst);
        });

        assert!(image.on_load.is_some());

        // Call the handler
        if let Some(handler) = &image.on_load {
            handler();
        }
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_image_on_error() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let image = Image::from_file("test.png").on_error(move || {
            called_clone.store(true, Ordering::SeqCst);
        });

        assert!(image.on_error.is_some());

        // Call the handler
        if let Some(handler) = &image.on_error {
            handler();
        }
        assert!(called.load(Ordering::SeqCst));
    }

    // ==================== Method Chaining Tests ====================

    #[test]
    fn test_image_chaining_all_methods() {
        let image = Image::from_file("test.png")
            .id(ElementId(1))
            .cover()
            .w(200.0)
            .h(150.0)
            .rounded(16.0)
            .alt("A beautiful sunset")
            .placeholder(Color::BLACK);

        assert_eq!(image.id, Some(ElementId(1)));
        assert_eq!(image.fit, ImageFit::Cover);
        assert_eq!(image.style.width, Some(200.0));
        assert_eq!(image.style.height, Some(150.0));
        assert_eq!(image.style.border.radius.top_left, 16.0);
        assert_eq!(image.alt_text, Some("A beautiful sunset".to_string()));
        assert_eq!(image.placeholder_color, Color::BLACK);
    }

    #[test]
    fn test_image_chaining_size_then_individual() {
        let image = Image::from_file("test.png").size((100.0, 100.0)).w(200.0);

        // w() should override the width from size()
        assert_eq!(image.style.width, Some(200.0));
        assert_eq!(image.style.height, Some(100.0));
    }

    #[test]
    fn test_image_chaining_fit_override() {
        let image = Image::from_file("test.png").cover().contain().fill();

        // Last fit should win
        assert_eq!(image.fit, ImageFit::Fill);
    }

    // ==================== calculate_dest_bounds Tests ====================

    fn create_test_image() -> Image {
        Image::from_file("test.png")
    }

    #[test]
    fn test_calculate_dest_bounds_fill() {
        let mut image = create_test_image().fill();
        image.intrinsic_size = Some(Size::new(100.0, 100.0));

        let container = Bounds::from_xywh(0.0, 0.0, 200.0, 300.0);
        let image_size = Size::new(100.0, 100.0);

        let result = image.calculate_dest_bounds(container, image_size);

        // Fill should return the container bounds exactly
        assert_eq!(result.x(), 0.0);
        assert_eq!(result.y(), 0.0);
        assert_eq!(result.width(), 200.0);
        assert_eq!(result.height(), 300.0);
    }

    #[test]
    fn test_calculate_dest_bounds_contain_landscape_image() {
        let mut image = create_test_image().contain();
        image.intrinsic_size = Some(Size::new(200.0, 100.0));

        let container = Bounds::from_xywh(0.0, 0.0, 400.0, 400.0);
        let image_size = Size::new(200.0, 100.0);

        let result = image.calculate_dest_bounds(container, image_size);

        // Should scale by 2x (400/200), resulting in 400x200, centered vertically
        assert_eq!(result.width(), 400.0);
        assert_eq!(result.height(), 200.0);
        assert_eq!(result.x(), 0.0);
        assert_eq!(result.y(), 100.0); // (400-200)/2
    }

    #[test]
    fn test_calculate_dest_bounds_contain_portrait_image() {
        let mut image = create_test_image().contain();
        image.intrinsic_size = Some(Size::new(100.0, 200.0));

        let container = Bounds::from_xywh(0.0, 0.0, 400.0, 400.0);
        let image_size = Size::new(100.0, 200.0);

        let result = image.calculate_dest_bounds(container, image_size);

        // Should scale by 2x (400/200), resulting in 200x400, centered horizontally
        assert_eq!(result.width(), 200.0);
        assert_eq!(result.height(), 400.0);
        assert_eq!(result.x(), 100.0); // (400-200)/2
        assert_eq!(result.y(), 0.0);
    }

    #[test]
    fn test_calculate_dest_bounds_cover_landscape_image() {
        let mut image = create_test_image().cover();
        image.intrinsic_size = Some(Size::new(200.0, 100.0));

        let container = Bounds::from_xywh(0.0, 0.0, 400.0, 400.0);
        let image_size = Size::new(200.0, 100.0);

        let result = image.calculate_dest_bounds(container, image_size);

        // Should scale by 4x (400/100), resulting in 800x400, centered horizontally
        assert_eq!(result.width(), 800.0);
        assert_eq!(result.height(), 400.0);
        assert_eq!(result.x(), -200.0); // (400-800)/2
        assert_eq!(result.y(), 0.0);
    }

    #[test]
    fn test_calculate_dest_bounds_cover_portrait_image() {
        let mut image = create_test_image().cover();
        image.intrinsic_size = Some(Size::new(100.0, 200.0));

        let container = Bounds::from_xywh(0.0, 0.0, 400.0, 400.0);
        let image_size = Size::new(100.0, 200.0);

        let result = image.calculate_dest_bounds(container, image_size);

        // Should scale by 4x (400/100), resulting in 400x800, centered vertically
        assert_eq!(result.width(), 400.0);
        assert_eq!(result.height(), 800.0);
        assert_eq!(result.x(), 0.0);
        assert_eq!(result.y(), -200.0); // (400-800)/2
    }

    #[test]
    fn test_calculate_dest_bounds_none() {
        let image = create_test_image().fit(ImageFit::None);

        let container = Bounds::from_xywh(0.0, 0.0, 400.0, 400.0);
        let image_size = Size::new(100.0, 100.0);

        let result = image.calculate_dest_bounds(container, image_size);

        // Should not scale, just center
        assert_eq!(result.width(), 100.0);
        assert_eq!(result.height(), 100.0);
        assert_eq!(result.x(), 150.0); // (400-100)/2
        assert_eq!(result.y(), 150.0); // (400-100)/2
    }

    #[test]
    fn test_calculate_dest_bounds_scale_down_fits() {
        let image = create_test_image().fit(ImageFit::ScaleDown);

        let container = Bounds::from_xywh(0.0, 0.0, 400.0, 400.0);
        let image_size = Size::new(100.0, 100.0);

        let result = image.calculate_dest_bounds(container, image_size);

        // Image fits in container, should not scale
        assert_eq!(result.width(), 100.0);
        assert_eq!(result.height(), 100.0);
        assert_eq!(result.x(), 150.0); // (400-100)/2
        assert_eq!(result.y(), 150.0); // (400-100)/2
    }

    #[test]
    fn test_calculate_dest_bounds_scale_down_too_large() {
        let image = create_test_image().fit(ImageFit::ScaleDown);

        let container = Bounds::from_xywh(0.0, 0.0, 100.0, 100.0);
        let image_size = Size::new(200.0, 200.0);

        let result = image.calculate_dest_bounds(container, image_size);

        // Image is too large, should scale down like contain
        assert_eq!(result.width(), 100.0);
        assert_eq!(result.height(), 100.0);
        assert_eq!(result.x(), 0.0);
        assert_eq!(result.y(), 0.0);
    }

    #[test]
    fn test_calculate_dest_bounds_with_offset_container() {
        let image = create_test_image().contain();

        let container = Bounds::from_xywh(50.0, 100.0, 200.0, 200.0);
        let image_size = Size::new(100.0, 200.0);

        let result = image.calculate_dest_bounds(container, image_size);

        // Image should be centered within the offset container
        assert_eq!(result.width(), 100.0);
        assert_eq!(result.height(), 200.0);
        assert_eq!(result.x(), 100.0); // 50 + (200-100)/2
        assert_eq!(result.y(), 100.0);
    }

    // ==================== Element Trait Tests ====================

    #[test]
    fn test_element_id_trait() {
        let image = Image::from_file("test.png");
        assert!(Element::id(&image).is_none());

        let image_with_id = Image::from_file("test.png").id(ElementId(42));
        assert_eq!(Element::id(&image_with_id), Some(ElementId(42)));
    }

    #[test]
    fn test_element_style_trait() {
        let image = Image::from_file("test.png").w(200.0).h(100.0);

        let style = image.style();
        assert_eq!(style.width, Some(200.0));
        assert_eq!(style.height, Some(100.0));
    }

    // ==================== Default Values Tests ====================

    #[test]
    fn test_image_default_values() {
        let image = Image::from_file("test.png");

        // Check all default values
        assert!(image.id.is_none());
        assert_eq!(image.fit, ImageFit::Contain);
        assert_eq!(image.state, ImageState::Loading);
        assert!(image.alt_text.is_none());
        assert!(image.texture_id.is_none());
        assert!(image.intrinsic_size.is_none());
        assert!(image.on_load.is_none());
        assert!(image.on_error.is_none());
        assert!(image.layout_node.is_none());

        // Style defaults
        assert!(image.style.width.is_none());
        assert!(image.style.height.is_none());
        assert!(image.style.border.radius.is_zero());
    }

    #[test]
    fn test_image_placeholder_default_color() {
        let image = Image::from_file("test.png");
        // Default placeholder color is Color::hex(0xf3f4f6)
        let expected = Color::hex(0xf3f4f6);
        assert_eq!(image.placeholder_color, expected);
    }

    // ==================== Helper Function Tests ====================

    #[test]
    fn test_image_helper_function() {
        let img = image("path/to/file.png");

        if let ImageSource::File(path) = &img.source {
            assert_eq!(path, "path/to/file.png");
        } else {
            panic!("Expected File source from image() helper");
        }
    }

    #[test]
    fn test_image_url_helper_function() {
        let img = image_url("https://example.com/photo.jpg");

        if let ImageSource::Url(url) = &img.source {
            assert_eq!(url, "https://example.com/photo.jpg");
        } else {
            panic!("Expected Url source from image_url() helper");
        }
    }

    // ==================== Edge Cases Tests ====================

    #[test]
    fn test_image_empty_path() {
        let image = Image::from_file("");

        if let ImageSource::File(path) = &image.source {
            assert_eq!(path, "");
        } else {
            panic!("Expected File source");
        }
    }

    #[test]
    fn test_image_empty_url() {
        let image = Image::from_url("");

        if let ImageSource::Url(url) = &image.source {
            assert_eq!(url, "");
        } else {
            panic!("Expected Url source");
        }
    }

    #[test]
    fn test_image_empty_data() {
        let image = Image::from_data(vec![], 0, 0);

        if let ImageSource::Data {
            data,
            width,
            height,
        } = &image.source
        {
            assert!(data.is_empty());
            assert_eq!(*width, 0);
            assert_eq!(*height, 0);
        } else {
            panic!("Expected Data source");
        }
    }

    #[test]
    fn test_image_zero_dimensions() {
        let image = Image::from_file("test.png").w(0.0).h(0.0);

        assert_eq!(image.style.width, Some(0.0));
        assert_eq!(image.style.height, Some(0.0));
    }

    #[test]
    fn test_image_negative_dimensions() {
        let image = Image::from_file("test.png").w(-100.0).h(-50.0);

        // Negative values should be accepted (validation happens elsewhere)
        assert_eq!(image.style.width, Some(-100.0));
        assert_eq!(image.style.height, Some(-50.0));
    }

    #[test]
    fn test_image_large_dimensions() {
        let image = Image::from_file("test.png").w(f32::MAX).h(f32::MAX);

        assert_eq!(image.style.width, Some(f32::MAX));
        assert_eq!(image.style.height, Some(f32::MAX));
    }

    #[test]
    fn test_image_zero_radius() {
        let image = Image::from_file("test.png").rounded(0.0);

        let radius = image.style.border.radius;
        assert!(radius.is_zero());
    }

    #[test]
    fn test_image_empty_alt_text() {
        let image = Image::from_file("test.png").alt("");
        assert_eq!(image.alt_text, Some("".to_string()));
    }

    #[test]
    fn test_image_unicode_alt_text() {
        let image = Image::from_file("test.png").alt("A photo of a cat");
        assert_eq!(image.alt_text, Some("A photo of a cat".to_string()));
    }

    #[test]
    fn test_image_unicode_path() {
        let image = Image::from_file("image.png");

        if let ImageSource::File(path) = &image.source {
            assert_eq!(path, "image.png");
        } else {
            panic!("Expected File source");
        }
    }

    // ==================== Table-Driven Tests ====================

    #[test]
    fn test_fit_methods_table() {
        struct TestCase {
            name: &'static str,
            builder: fn(Image) -> Image,
            expected: ImageFit,
        }

        let test_cases = [
            TestCase {
                name: "cover",
                builder: |img| img.cover(),
                expected: ImageFit::Cover,
            },
            TestCase {
                name: "contain",
                builder: |img| img.contain(),
                expected: ImageFit::Contain,
            },
            TestCase {
                name: "fill",
                builder: |img| img.fill(),
                expected: ImageFit::Fill,
            },
            TestCase {
                name: "fit(Cover)",
                builder: |img| img.fit(ImageFit::Cover),
                expected: ImageFit::Cover,
            },
            TestCase {
                name: "fit(Contain)",
                builder: |img| img.fit(ImageFit::Contain),
                expected: ImageFit::Contain,
            },
            TestCase {
                name: "fit(Fill)",
                builder: |img| img.fit(ImageFit::Fill),
                expected: ImageFit::Fill,
            },
            TestCase {
                name: "fit(None)",
                builder: |img| img.fit(ImageFit::None),
                expected: ImageFit::None,
            },
            TestCase {
                name: "fit(ScaleDown)",
                builder: |img| img.fit(ImageFit::ScaleDown),
                expected: ImageFit::ScaleDown,
            },
        ];

        for case in test_cases {
            let image = (case.builder)(Image::from_file("test.png"));
            assert_eq!(image.fit, case.expected, "Failed for case: {}", case.name);
        }
    }

    #[test]
    fn test_size_methods_table() {
        struct TestCase {
            name: &'static str,
            builder: fn(Image) -> Image,
            expected_width: Option<f32>,
            expected_height: Option<f32>,
        }

        let test_cases = [
            TestCase {
                name: "w only",
                builder: |img| img.w(100.0),
                expected_width: Some(100.0),
                expected_height: None,
            },
            TestCase {
                name: "h only",
                builder: |img| img.h(200.0),
                expected_width: None,
                expected_height: Some(200.0),
            },
            TestCase {
                name: "w and h",
                builder: |img| img.w(100.0).h(200.0),
                expected_width: Some(100.0),
                expected_height: Some(200.0),
            },
            TestCase {
                name: "size",
                builder: |img| img.size((300.0, 400.0)),
                expected_width: Some(300.0),
                expected_height: Some(400.0),
            },
        ];

        for case in test_cases {
            let image = (case.builder)(Image::from_file("test.png"));
            assert_eq!(
                image.style.width, case.expected_width,
                "Width mismatch for case: {}",
                case.name
            );
            assert_eq!(
                image.style.height, case.expected_height,
                "Height mismatch for case: {}",
                case.name
            );
        }
    }

    #[test]
    fn test_constructor_variants_table() {
        struct TestCase {
            name: &'static str,
            image: Image,
            check: fn(&ImageSource) -> bool,
        }

        let test_cases = [
            TestCase {
                name: "from_file",
                image: Image::from_file("test.png"),
                check: |s| matches!(s, ImageSource::File(_)),
            },
            TestCase {
                name: "from_url",
                image: Image::from_url("https://example.com/img.png"),
                check: |s| matches!(s, ImageSource::Url(_)),
            },
            TestCase {
                name: "from_data",
                image: Image::from_data(vec![0u8; 4], 1, 1),
                check: |s| matches!(s, ImageSource::Data { .. }),
            },
            TestCase {
                name: "from_texture",
                image: Image::from_texture(1),
                check: |s| matches!(s, ImageSource::Texture(_)),
            },
        ];

        for case in test_cases {
            assert!(
                (case.check)(&case.image.source),
                "Source check failed for case: {}",
                case.name
            );
        }
    }

    #[test]
    fn test_calculate_dest_bounds_table() {
        struct TestCase {
            name: &'static str,
            fit: ImageFit,
            container: Bounds,
            image_size: Size,
            expected_width: f32,
            expected_height: f32,
        }

        let test_cases = [
            TestCase {
                name: "Fill - stretch to container",
                fit: ImageFit::Fill,
                container: Bounds::from_xywh(0.0, 0.0, 200.0, 100.0),
                image_size: Size::new(50.0, 50.0),
                expected_width: 200.0,
                expected_height: 100.0,
            },
            TestCase {
                name: "Contain - landscape in square",
                fit: ImageFit::Contain,
                container: Bounds::from_xywh(0.0, 0.0, 100.0, 100.0),
                image_size: Size::new(200.0, 100.0),
                expected_width: 100.0,
                expected_height: 50.0,
            },
            TestCase {
                name: "Cover - landscape in square",
                fit: ImageFit::Cover,
                container: Bounds::from_xywh(0.0, 0.0, 100.0, 100.0),
                image_size: Size::new(200.0, 100.0),
                expected_width: 200.0,
                expected_height: 100.0,
            },
            TestCase {
                name: "None - no scaling",
                fit: ImageFit::None,
                container: Bounds::from_xywh(0.0, 0.0, 100.0, 100.0),
                image_size: Size::new(50.0, 50.0),
                expected_width: 50.0,
                expected_height: 50.0,
            },
        ];

        for case in test_cases {
            let image = Image::from_file("test.png").fit(case.fit);
            let result = image.calculate_dest_bounds(case.container, case.image_size);

            assert!(
                (result.width() - case.expected_width).abs() < 0.001,
                "Width mismatch for case: {}. Expected {}, got {}",
                case.name,
                case.expected_width,
                result.width()
            );
            assert!(
                (result.height() - case.expected_height).abs() < 0.001,
                "Height mismatch for case: {}. Expected {}, got {}",
                case.name,
                case.expected_height,
                result.height()
            );
        }
    }
}
