//! Shared platform window contract.

use crate::core::event::{KeyEvent, MouseButton, ScrollEvent};
use crate::core::geometry::{Point, Size};
use crate::core::window::WindowOptions;
use crate::renderer::RendererError;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformWindowFeature {
    Lifecycle,
    InputEvents,
    Dpi,
    Resizing,
    Focus,
    Clipboard,
    RendererAttachment,
}

impl PlatformWindowFeature {
    pub fn name(self) -> &'static str {
        match self {
            Self::Lifecycle => "lifecycle",
            Self::InputEvents => "input events",
            Self::Dpi => "dpi",
            Self::Resizing => "resizing",
            Self::Focus => "focus",
            Self::Clipboard => "clipboard",
            Self::RendererAttachment => "renderer attachment",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformWindowFeatures {
    pub lifecycle: bool,
    pub input_events: bool,
    pub dpi: bool,
    pub resizing: bool,
    pub focus: bool,
    pub clipboard: bool,
    pub renderer_attachment: bool,
}

impl PlatformWindowFeatures {
    pub const REQUIRED: [PlatformWindowFeature; 7] = [
        PlatformWindowFeature::Lifecycle,
        PlatformWindowFeature::InputEvents,
        PlatformWindowFeature::Dpi,
        PlatformWindowFeature::Resizing,
        PlatformWindowFeature::Focus,
        PlatformWindowFeature::Clipboard,
        PlatformWindowFeature::RendererAttachment,
    ];

    pub fn none() -> Self {
        Self {
            lifecycle: false,
            input_events: false,
            dpi: false,
            resizing: false,
            focus: false,
            clipboard: false,
            renderer_attachment: false,
        }
    }

    pub fn supported() -> Self {
        Self {
            lifecycle: true,
            input_events: true,
            dpi: true,
            resizing: true,
            focus: true,
            clipboard: true,
            renderer_attachment: true,
        }
    }

    pub fn supports(self, feature: PlatformWindowFeature) -> bool {
        match feature {
            PlatformWindowFeature::Lifecycle => self.lifecycle,
            PlatformWindowFeature::InputEvents => self.input_events,
            PlatformWindowFeature::Dpi => self.dpi,
            PlatformWindowFeature::Resizing => self.resizing,
            PlatformWindowFeature::Focus => self.focus,
            PlatformWindowFeature::Clipboard => self.clipboard,
            PlatformWindowFeature::RendererAttachment => self.renderer_attachment,
        }
    }

    pub fn missing_required(self) -> Vec<PlatformWindowFeature> {
        Self::REQUIRED
            .into_iter()
            .filter(|feature| !self.supports(*feature))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlatformWindowState {
    pub size: Size,
    pub scale_factor: f32,
    pub focused: bool,
    pub visible: bool,
    pub renderer_attached: bool,
}

impl PlatformWindowState {
    pub fn from_options(options: &WindowOptions) -> Self {
        Self {
            size: options.size,
            scale_factor: 1.0,
            focused: false,
            visible: false,
            renderer_attached: false,
        }
    }

    pub fn with_scale_factor(mut self, scale_factor: f32) -> Self {
        self.scale_factor = scale_factor;
        self
    }

    pub fn with_focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn with_renderer_attached(mut self, renderer_attached: bool) -> Self {
        self.renderer_attached = renderer_attached;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlatformRendererTarget {
    MetalLayer,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlatformRendererAttachment {
    pub target: PlatformRendererTarget,
    pub viewport_size: Size,
    pub scale_factor: f32,
}

#[derive(Debug, Clone)]
pub enum PlatformInputEvent {
    KeyDown(KeyEvent),
    KeyUp(KeyEvent),
    Mouse(PlatformMouseEvent),
    Scroll(ScrollEvent),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformMouseEventKind {
    Down,
    Up,
    Move,
}

#[derive(Debug, Clone)]
pub struct PlatformMouseEvent {
    pub kind: PlatformMouseEventKind,
    pub position: Point,
    pub button: Option<MouseButton>,
}

#[derive(Debug, Clone)]
pub enum PlatformWindowEvent {
    Created,
    CloseRequested,
    Resized(Size),
    ScaleFactorChanged(f32),
    FocusChanged(bool),
    Input(PlatformInputEvent),
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlatformWindowError {
    Unsupported {
        platform: String,
        feature: PlatformWindowFeature,
    },
    InvalidOptions {
        message: String,
    },
    Backend {
        platform: String,
        message: String,
    },
    Renderer(RendererError),
}

impl PlatformWindowError {
    pub fn unsupported(platform: impl Into<String>, feature: PlatformWindowFeature) -> Self {
        Self::Unsupported {
            platform: platform.into(),
            feature,
        }
    }

    pub fn invalid_options(message: impl Into<String>) -> Self {
        Self::InvalidOptions {
            message: message.into(),
        }
    }

    pub fn backend(platform: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Backend {
            platform: platform.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for PlatformWindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported { platform, feature } => {
                write!(
                    f,
                    "{} does not support platform window feature: {}",
                    platform,
                    feature.name()
                )
            }
            Self::InvalidOptions { message } => write!(f, "invalid window options: {message}"),
            Self::Backend { platform, message } => {
                write!(f, "{platform} platform window failed: {message}")
            }
            Self::Renderer(err) => write!(f, "{err}"),
        }
    }
}

impl Error for PlatformWindowError {}

impl From<RendererError> for PlatformWindowError {
    fn from(value: RendererError) -> Self {
        Self::Renderer(value)
    }
}

pub trait PlatformWindow {
    fn platform_name(&self) -> &'static str;

    fn features(&self) -> PlatformWindowFeatures;

    fn state(&self) -> Result<PlatformWindowState, PlatformWindowError>;

    fn set_title(&mut self, _title: &str) -> Result<(), PlatformWindowError> {
        Err(PlatformWindowError::unsupported(
            self.platform_name(),
            PlatformWindowFeature::Lifecycle,
        ))
    }

    fn set_size(&mut self, _size: Size) -> Result<(), PlatformWindowError> {
        Err(PlatformWindowError::unsupported(
            self.platform_name(),
            PlatformWindowFeature::Resizing,
        ))
    }

    fn set_focus(&mut self, _focused: bool) -> Result<(), PlatformWindowError> {
        Err(PlatformWindowError::unsupported(
            self.platform_name(),
            PlatformWindowFeature::Focus,
        ))
    }

    fn read_clipboard_text(&mut self) -> Result<String, PlatformWindowError> {
        Err(PlatformWindowError::unsupported(
            self.platform_name(),
            PlatformWindowFeature::Clipboard,
        ))
    }

    fn write_clipboard_text(&mut self, _text: &str) -> Result<(), PlatformWindowError> {
        Err(PlatformWindowError::unsupported(
            self.platform_name(),
            PlatformWindowFeature::Clipboard,
        ))
    }

    fn poll_events(&mut self) -> Result<Vec<PlatformWindowEvent>, PlatformWindowError> {
        Err(PlatformWindowError::unsupported(
            self.platform_name(),
            PlatformWindowFeature::InputEvents,
        ))
    }

    fn renderer_attachment(&self) -> Result<PlatformRendererAttachment, PlatformWindowError> {
        Err(PlatformWindowError::unsupported(
            self.platform_name(),
            PlatformWindowFeature::RendererAttachment,
        ))
    }

    fn close(&mut self) -> Result<(), PlatformWindowError> {
        Err(PlatformWindowError::unsupported(
            self.platform_name(),
            PlatformWindowFeature::Lifecycle,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct UnsupportedPlatformWindow {
    platform_name: &'static str,
    state: PlatformWindowState,
}

impl UnsupportedPlatformWindow {
    pub fn new(platform_name: &'static str, options: WindowOptions) -> Self {
        Self {
            platform_name,
            state: PlatformWindowState::from_options(&options),
        }
    }
}

impl PlatformWindow for UnsupportedPlatformWindow {
    fn platform_name(&self) -> &'static str {
        self.platform_name
    }

    fn features(&self) -> PlatformWindowFeatures {
        PlatformWindowFeatures::none()
    }

    fn state(&self) -> Result<PlatformWindowState, PlatformWindowError> {
        Ok(self.state.clone())
    }
}

pub fn validate_window_options(options: &WindowOptions) -> Result<(), PlatformWindowError> {
    if options.size.width <= 0.0 || options.size.height <= 0.0 {
        return Err(PlatformWindowError::invalid_options(
            "window width and height must be greater than zero",
        ));
    }

    if let Some(min_size) = options.min_size
        && (min_size.width <= 0.0 || min_size.height <= 0.0)
    {
        return Err(PlatformWindowError::invalid_options(
            "minimum window width and height must be greater than zero",
        ));
    }

    if let Some(max_size) = options.max_size
        && (max_size.width <= 0.0 || max_size.height <= 0.0)
    {
        return Err(PlatformWindowError::invalid_options(
            "maximum window width and height must be greater than zero",
        ));
    }

    if let (Some(min_size), Some(max_size)) = (options.min_size, options.max_size)
        && (min_size.width > max_size.width || min_size.height > max_size.height)
    {
        return Err(PlatformWindowError::invalid_options(
            "minimum window size must not exceed maximum window size",
        ));
    }

    Ok(())
}
