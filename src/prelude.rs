//! Prelude module - commonly used types and traits

pub use crate::core::{
    App, AppContext, ElementId, EntityId, View, ViewContext, ViewNotifier, Window, WindowOptions,
    accessibility::{
        AccessibilityAction, AccessibilityAnnouncement, AccessibilityAnnouncementKind,
        AccessibilityBridge, AccessibilityContext, AccessibilityError, AccessibilityNode,
        AccessibilityRole, AccessibilityTree, UnsupportedAccessibilityBridge,
    },
    action::{
        ActionError, ActionHandler, ActionId, ActionOutcome, ActionRouter, KeyChord, Keymap,
        StandardAction,
    },
    animation::{Animatable, Animation, Easing, Transform, Transition},
    color::{Color, Hsla, Rgba},
    event::{Cursor, KeyCode, KeyEvent, Modifiers, MouseEvent},
    geometry::{Bounds, Edges, Point, Rect, Size},
    style::{Background, BorderStyle, Corners, Dimension, DimensionConstraints, Style},
    text_editing::{
        CaretGeometry, Clipboard, ClipboardError, MemoryClipboard, SelectionRect, TextComposition,
        TextEditBuffer, TextEditError, TextEditLayout, TextEditOutcome, TextInputEvent, TextLine,
        TextRange, TextSelection,
    },
};

pub use crate::elements::{
    Button, ButtonSize, ButtonVariant, Div, Element, EventResult, Image, ImageFit, Input,
    InputType, IntoElement, Render, ScrollDirection, ScrollView, Text, TextArea, TextAreaState,
    button, div, image, input, scroll_view, text, text_area,
};

pub use crate::advanced_ui;

pub use std::time::Duration;
