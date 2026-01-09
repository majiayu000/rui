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
        Self::new(ImageSource::Data { data, width, height })
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
                if image_size.width <= container.width() && image_size.height <= container.height() {
                    // No scaling needed
                    let x = container.x() + (container.width() - image_size.width) / 2.0;
                    let y = container.y() + (container.height() - image_size.height) / 2.0;
                    Bounds::from_xywh(x, y, image_size.width, image_size.height)
                } else {
                    // Scale down like contain
                    self.calculate_dest_bounds(container, image_size)
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

        let node = cx.taffy.new_leaf(style).expect("Failed to create image layout node");
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
