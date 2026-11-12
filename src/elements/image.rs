//! Image element for displaying images

use crate::core::ElementId;
use crate::core::color::Color;
use crate::core::geometry::{Bounds, Edges, Size};
use crate::core::style::{Corners, Style};
use crate::elements::element::{
    Capabilities, Element, LayoutContext, PaintContext, style_to_taffy,
};
use crate::renderer::Primitive;
use crate::renderer::text::BaseDirection;
use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
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
    /// Being decoded off the UI thread; the placeholder shows.
    Loading,
    /// Ready to draw.
    Loaded,
    /// Could not be loaded; the image shows its alt text or the error.
    Error,
}

/// RGBA pixels decoded off the UI thread by [`crate::AppContext::image`],
/// shared by the elements that show them and the renderer, which keeps one
/// texture for each.
#[derive(Debug)]
pub struct DecodedImage {
    id: u64,
    width: u32,
    height: u32,
    pixels: Arc<[u8]>,
}

impl DecodedImage {
    /// `pixels` are `width * height` RGBA pixels.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, String> {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let expected = width as usize * height as usize * 4;
        if width == 0 || height == 0 || pixels.len() != expected {
            return Err(format!(
                "a {width} x {height} image needs {expected} RGBA bytes, got {}",
                pixels.len()
            ));
        }
        Ok(Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            width,
            height,
            pixels: pixels.into(),
        })
    }

    /// Unique to this image, for the renderer to key its texture by.
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &Arc<[u8]> {
        &self.pixels
    }
}

impl PartialEq for DecodedImage {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

/// Image element
pub struct Image {
    id: Option<ElementId>,
    /// `None` while loading or after a failed load.
    source: Option<ImageSource>,
    fit: ImageFit,
    style: Style,
    state: ImageState,
    error: Option<Arc<str>>,
    alt_text: Option<String>,
    placeholder_color: Color,
    texture_id: Option<u32>,
    intrinsic_size: Option<Size>,
    on_load: Option<Box<dyn Fn()>>,
    on_error: Option<Box<dyn Fn()>>,
    layout_node: Option<NodeId>,
}

/// Image source
#[derive(Debug, Clone, PartialEq)]
pub enum ImageSource {
    /// Path to local file
    File(String),
    /// Raw pixel data (RGBA)
    Data {
        data: Vec<u8>,
        width: u32,
        height: u32,
    },
    /// Reference to a texture already in GPU memory
    Texture(u32),
    /// Pixels decoded by [`crate::AppContext::image`]
    Decoded(Arc<DecodedImage>),
}

impl Image {
    /// An image of `source`. A file is decoded where it is first drawn; use
    /// [`crate::AppContext::image`] to decode it off the UI thread instead.
    pub fn new(source: ImageSource) -> Self {
        Self::with_state(Some(source), ImageState::Loaded, None)
    }

    /// An image still being decoded, which shows its placeholder.
    pub(crate) fn loading() -> Self {
        Self::with_state(None, ImageState::Loading, None)
    }

    /// An image that could not be loaded, which shows its alt text or `error`.
    pub(crate) fn failed(error: Arc<str>) -> Self {
        Self::with_state(None, ImageState::Error, Some(error))
    }

    /// An image of pixels decoded off the UI thread, at their size unless sized.
    pub(crate) fn decoded(image: Arc<DecodedImage>) -> Self {
        let size = Size::new(image.width() as f32, image.height() as f32);
        let mut decoded = Self::new(ImageSource::Decoded(image));
        decoded.intrinsic_size = Some(size);
        decoded
    }

    fn with_state(source: Option<ImageSource>, state: ImageState, error: Option<Arc<str>>) -> Self {
        Self {
            id: None,
            source,
            fit: ImageFit::default(),
            style: Style::new(),
            state,
            error,
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

    pub fn state(&self) -> ImageState {
        self.state
    }

    /// Why the image could not be loaded, in [`ImageState::Error`].
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Calculate the destination bounds based on fit mode
    fn calculate_dest_bounds(&self, container: Bounds, image_size: Size) -> Bounds {
        fit_bounds(container, image_size, self.fit)
    }
}

/// Where an `image_size` image lands in `container` under `fit`. Cover and
/// None can land past the container; the renderer clips them to it.
pub(crate) fn fit_bounds(container: Bounds, image_size: Size, fit: ImageFit) -> Bounds {
    if image_size.width <= 0.0 || image_size.height <= 0.0 {
        return container;
    }
    let centered = |width: f32, height: f32| {
        let x = container.x() + (container.width() - width) / 2.0;
        let y = container.y() + (container.height() - height) / 2.0;
        Bounds::from_xywh(x, y, width, height)
    };
    let scale_x = container.width() / image_size.width;
    let scale_y = container.height() / image_size.height;
    let scaled = |scale: f32| centered(image_size.width * scale, image_size.height * scale);
    match fit {
        ImageFit::Fill => container,
        ImageFit::Contain => scaled(scale_x.min(scale_y)),
        ImageFit::Cover => scaled(scale_x.max(scale_y)),
        ImageFit::None => centered(image_size.width, image_size.height),
        ImageFit::ScaleDown => {
            if image_size.width <= container.width() && image_size.height <= container.height() {
                centered(image_size.width, image_size.height)
            } else {
                scaled(scale_x.min(scale_y))
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

    fn capabilities(&self) -> Capabilities {
        Capabilities::PURE_INERT
    }

    /// The same when it shows the same thing. Handlers cannot be compared, so
    /// an image with one is built again.
    fn same_props(&self, previous: &dyn Element) -> bool {
        let Some(previous) = (previous as &dyn Any).downcast_ref::<Image>() else {
            return false;
        };
        self.on_load.is_none()
            && self.on_error.is_none()
            && previous.on_load.is_none()
            && previous.on_error.is_none()
            && self.id == previous.id
            && self.source == previous.source
            && self.fit == previous.fit
            && self.style == previous.style
            && self.state == previous.state
            && self.error == previous.error
            && self.alt_text == previous.alt_text
            && self.placeholder_color == previous.placeholder_color
            && self.texture_id == previous.texture_id
            && self.intrinsic_size == previous.intrinsic_size
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
            .new_leaf(style)
            .expect("Failed to create image layout node");
        self.layout_node = Some(node);
        node
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        let failed = self.state == ImageState::Error;
        let background = if failed {
            Color::hex(0xfee2e2)
        } else {
            self.placeholder_color
        };
        cx.paint(Primitive::Quad {
            bounds,
            background: background.to_rgba(),
            border_color: crate::core::color::Rgba::TRANSPARENT,
            border_widths: Edges::ZERO,
            corner_radii: self.style.border.radius,
        });

        // The renderer resolves the source and fits it.
        if let Some(source) = &self.source {
            cx.paint(Primitive::Image {
                bounds,
                source: source.clone(),
                fit: self.fit,
                corner_radii: self.style.border.radius,
                opacity: self.style.opacity,
            });
        }

        // A failed image says so, never shows blank.
        if failed {
            let message = self.alt_text.as_deref().or(self.error.as_deref());
            if let Some(message) = message {
                let text_x = bounds.x() + 8.0;
                let text_y = bounds.y() + bounds.height() / 2.0 - 7.0;
                cx.paint(Primitive::Text {
                    bounds: Bounds::from_xywh(text_x, text_y, bounds.width() - 16.0, 14.0),
                    content: message.to_string().into(),
                    color: Color::hex(0xb91c1c).to_rgba(),
                    font_size: 12.0,
                    font_weight: 400,
                    font_family: None,
                    line_height: 1.2,
                    align: crate::elements::text::TextAlign::Center,
                    direction: BaseDirection::Auto,
                });
            }
        }
    }
}

/// Create an image from file path
pub fn image(source: impl Into<String>) -> Image {
    Image::from_file(source)
}

#[cfg(test)]
mod tests;
