//! Prelude module - commonly used types and traits

pub use crate::core::{
    color::{Color, Hsla, Rgba},
    geometry::{Bounds, Edges, Point, Rect, Size},
    style::{Background, BorderStyle, Corners, Style},
    event::{MouseEvent, KeyEvent, KeyCode, Modifiers, Cursor},
    animation::{Animation, Animatable, Easing, Transform, Transition},
    App, AppContext, ElementId, EntityId, View, ViewContext, Window, WindowOptions,
};

pub use crate::elements::{
    div, text, button, input, scroll_view, image, image_url,
    Div, Text, Button, Input, ScrollView, Image,
    Element, IntoElement, Render,
    ButtonVariant, ButtonSize, InputType, ScrollDirection, ImageFit,
};

pub use std::time::Duration;
