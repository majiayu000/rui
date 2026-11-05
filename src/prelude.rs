//! Prelude module - commonly used types and traits

pub use crate::core::{
    App, AppContext, AsyncApp, ElementId, EntityId, Task, TaskPriority, View, ViewContext,
    ViewNotifier, Window, WindowOptions,
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
    Canvas, Div, Element, EventResult, Image, ImageFit, Input, InputType, IntoElement, Render,
    ScrollDirection, ScrollView, StyledText, Text, TextArea, TextAreaState, TextSpan, canvas, div,
    image, input, scroll_view, span, styled_text, text, text_area,
};

pub use crate::advanced_ui;
pub use crate::advanced_ui::{Button, ControlSize, ControlVariant, Theme, button};

pub use std::time::Duration;
