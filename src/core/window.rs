//! Window management

use crate::core::geometry::Size;

/// Window configuration options
#[derive(Debug, Clone)]
pub struct WindowOptions {
    pub title: String,
    pub size: Size,
    pub min_size: Option<Size>,
    pub max_size: Option<Size>,
    pub resizable: bool,
    pub transparent: bool,
    pub decorations: bool,
}

impl Default for WindowOptions {
    fn default() -> Self {
        Self {
            title: String::from("RUI Window"),
            size: Size::new(800.0, 600.0),
            min_size: None,
            max_size: None,
            resizable: true,
            transparent: false,
            decorations: true,
        }
    }
}

impl WindowOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Size::new(width, height);
        self
    }

    pub fn min_size(mut self, width: f32, height: f32) -> Self {
        self.min_size = Some(Size::new(width, height));
        self
    }

    pub fn max_size(mut self, width: f32, height: f32) -> Self {
        self.max_size = Some(Size::new(width, height));
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    pub fn transparent(mut self, transparent: bool) -> Self {
        self.transparent = transparent;
        self
    }

    pub fn decorations(mut self, decorations: bool) -> Self {
        self.decorations = decorations;
        self
    }
}

/// Represents an open window
pub struct Window {
    pub(crate) id: WindowId,
    pub(crate) options: WindowOptions,
    pub(crate) size: Size,
    pub(crate) scale_factor: f32,
    pub(crate) focused: bool,
}

/// Unique window identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub(crate) u64);

impl WindowId {
    pub(crate) fn new(id: u64) -> Self {
        Self(id)
    }
}

impl Window {
    pub fn new(id: WindowId, options: WindowOptions) -> Self {
        Self {
            id,
            size: options.size,
            options,
            scale_factor: 1.0,
            focused: false,
        }
    }

    pub fn id(&self) -> WindowId {
        self.id
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn title(&self) -> &str {
        &self.options.title
    }

    /// Get the physical size in pixels
    pub fn physical_size(&self) -> Size {
        Size::new(
            self.size.width * self.scale_factor,
            self.size.height * self.scale_factor,
        )
    }
}
