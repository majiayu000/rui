//! Core Element trait and related types

use crate::core::geometry::{Bounds, Size};
use crate::core::style::Style;
use crate::core::ElementId;
use crate::renderer::{Scene, Primitive};
use taffy::prelude::*;

/// Layout context passed during layout phase
pub struct LayoutContext<'a> {
    pub taffy: &'a mut TaffyTree<ElementId>,
    pub available_space: Size,
}

impl<'a> LayoutContext<'a> {
    pub fn new(taffy: &'a mut TaffyTree<ElementId>, available_space: Size) -> Self {
        Self {
            taffy,
            available_space,
        }
    }
}

/// Paint context passed during paint phase
pub struct PaintContext<'a> {
    pub scene: &'a mut Scene,
    pub bounds: Bounds,
}

impl<'a> PaintContext<'a> {
    pub fn new(scene: &'a mut Scene, bounds: Bounds) -> Self {
        Self { scene, bounds }
    }

    /// Add a primitive to the scene
    pub fn paint(&mut self, primitive: Primitive) {
        self.scene.insert(primitive);
    }

    /// Get the current bounds
    pub fn bounds(&self) -> Bounds {
        self.bounds
    }

    /// Create a child paint context with new bounds
    pub fn with_bounds(&mut self, bounds: Bounds) -> PaintContext<'_> {
        PaintContext {
            scene: self.scene,
            bounds,
        }
    }
}

/// The core Element trait - all UI components implement this
pub trait Element: 'static {
    /// Unique identifier for this element (optional)
    fn id(&self) -> Option<ElementId> {
        None
    }

    /// Get the element's style
    fn style(&self) -> &Style;

    /// Request layout from Taffy
    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId;

    /// Paint the element to the scene
    fn paint(&mut self, cx: &mut PaintContext);

    /// Get child elements
    fn children(&self) -> &[AnyElement] {
        &[]
    }
}

/// Type-erased element wrapper
pub struct AnyElement {
    inner: Box<dyn Element>,
}

impl AnyElement {
    pub fn new<E: Element>(element: E) -> Self {
        Self {
            inner: Box::new(element),
        }
    }

    pub fn style(&self) -> &Style {
        self.inner.style()
    }

    pub fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        self.inner.layout(cx)
    }

    pub fn paint(&mut self, cx: &mut PaintContext) {
        self.inner.paint(cx)
    }
}

impl<E: Element> From<E> for AnyElement {
    fn from(element: E) -> Self {
        AnyElement::new(element)
    }
}

/// Trait for types that can be converted into elements
pub trait IntoElement {
    type Element: Element;

    fn into_element(self) -> Self::Element;

    fn into_any_element(self) -> AnyElement
    where
        Self: Sized,
    {
        AnyElement::new(self.into_element())
    }
}

impl<E: Element> IntoElement for E {
    type Element = E;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Trait for renderable components (views)
pub trait Render {
    fn render(&mut self) -> impl Element;
}

/// Convert Taffy style to our style
pub fn style_to_taffy(style: &Style) -> taffy::Style {
    taffy::Style {
        display: match style.display {
            crate::core::style::Display::Flex => taffy::Display::Flex,
            crate::core::style::Display::Block => taffy::Display::Block,
            crate::core::style::Display::None => taffy::Display::None,
        },
        position: match style.position {
            crate::core::style::Position::Relative => taffy::Position::Relative,
            crate::core::style::Position::Absolute => taffy::Position::Absolute,
        },
        flex_direction: match style.flex_direction {
            crate::core::style::FlexDirection::Row => taffy::FlexDirection::Row,
            crate::core::style::FlexDirection::Column => taffy::FlexDirection::Column,
            crate::core::style::FlexDirection::RowReverse => taffy::FlexDirection::RowReverse,
            crate::core::style::FlexDirection::ColumnReverse => taffy::FlexDirection::ColumnReverse,
        },
        justify_content: Some(match style.justify_content {
            crate::core::style::JustifyContent::FlexStart => taffy::JustifyContent::FlexStart,
            crate::core::style::JustifyContent::FlexEnd => taffy::JustifyContent::FlexEnd,
            crate::core::style::JustifyContent::Center => taffy::JustifyContent::Center,
            crate::core::style::JustifyContent::SpaceBetween => taffy::JustifyContent::SpaceBetween,
            crate::core::style::JustifyContent::SpaceAround => taffy::JustifyContent::SpaceAround,
            crate::core::style::JustifyContent::SpaceEvenly => taffy::JustifyContent::SpaceEvenly,
        }),
        align_items: Some(match style.align_items {
            crate::core::style::AlignItems::FlexStart => taffy::AlignItems::FlexStart,
            crate::core::style::AlignItems::FlexEnd => taffy::AlignItems::FlexEnd,
            crate::core::style::AlignItems::Center => taffy::AlignItems::Center,
            crate::core::style::AlignItems::Stretch => taffy::AlignItems::Stretch,
            crate::core::style::AlignItems::Baseline => taffy::AlignItems::Baseline,
        }),
        flex_grow: style.flex_grow,
        flex_shrink: style.flex_shrink,
        gap: taffy::Size {
            width: LengthPercentage::Length(style.gap),
            height: LengthPercentage::Length(style.gap),
        },
        size: taffy::Size {
            width: style.width.map(|w| Dimension::Length(w)).unwrap_or(Dimension::Auto),
            height: style.height.map(|h| Dimension::Length(h)).unwrap_or(Dimension::Auto),
        },
        min_size: taffy::Size {
            width: style.min_width.map(|w| Dimension::Length(w)).unwrap_or(Dimension::Auto),
            height: style.min_height.map(|h| Dimension::Length(h)).unwrap_or(Dimension::Auto),
        },
        max_size: taffy::Size {
            width: style.max_width.map(|w| Dimension::Length(w)).unwrap_or(Dimension::Auto),
            height: style.max_height.map(|h| Dimension::Length(h)).unwrap_or(Dimension::Auto),
        },
        margin: taffy::Rect {
            top: LengthPercentageAuto::Length(style.margin.top),
            right: LengthPercentageAuto::Length(style.margin.right),
            bottom: LengthPercentageAuto::Length(style.margin.bottom),
            left: LengthPercentageAuto::Length(style.margin.left),
        },
        padding: taffy::Rect {
            top: LengthPercentage::Length(style.padding.top),
            right: LengthPercentage::Length(style.padding.right),
            bottom: LengthPercentage::Length(style.padding.bottom),
            left: LengthPercentage::Length(style.padding.left),
        },
        ..Default::default()
    }
}
