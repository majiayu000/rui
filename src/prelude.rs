//! Prelude module - commonly used types and traits

pub use crate::core::{
    color::{Color, Hsla, Rgba},
    geometry::{Bounds, Edges, Point, Rect, Size},
    style::{Background, BorderStyle, Corners, Style},
    event::{MouseEvent, KeyEvent, KeyCode, Modifiers, Cursor},
    animation::{Animation, Animatable, Easing, Transform, Transition},
    action::{
        ActionError, ActionHandler, ActionId, ActionOutcome, ActionRouter, KeyChord, Keymap,
        StandardAction,
    },
    accessibility::{
        AccessibilityAction, AccessibilityAnnouncement, AccessibilityAnnouncementKind,
        AccessibilityBridge, AccessibilityContext, AccessibilityError, AccessibilityNode,
        AccessibilityRole, AccessibilityTree, UnsupportedAccessibilityBridge,
    },
    text_editing::{
        CaretGeometry, Clipboard, ClipboardError, MemoryClipboard, SelectionRect, TextComposition,
        TextEditBuffer, TextEditError, TextEditLayout, TextEditOutcome, TextInputEvent, TextLine,
        TextRange, TextSelection,
    },
    App, AppContext, ElementId, EntityId, View, ViewContext, ViewNotifier, Window, WindowOptions,
};

pub use crate::elements::{
    div, text, button, input, scroll_view, image, image_url,
    Div, Text, Button, Input, ScrollView, Image,
    Element, EventResult, IntoElement, Render,
    ButtonVariant, ButtonSize, InputType, ScrollDirection, ImageFit,
};

pub use crate::advanced_ui;

pub use std::time::Duration;
