//! An element that paints primitives from a closure, for grids, charts and editors.

use crate::core::ElementId;
use crate::core::color::{Color, Rgba};
use crate::core::geometry::{Bounds, Edges, Point};
use crate::core::style::{Corners, Dimension as StyleDimension, Style};
use crate::elements::element::{
    Element, EventContext, LayoutContext, PaintContext, PointerEvent, PointerEventKind, StableId,
    style_to_taffy,
};
use crate::renderer::{Primitive, Scene};
use taffy::prelude::*;

/// Inserts primitives into the scene while a [`Canvas`] paints.
///
/// Coordinates are absolute window coordinates, like every other primitive in the
/// scene: the paint closure gets the canvas bounds and offsets from their origin.
/// Translating each primitive instead would cost per cell and rewrite every path
/// vertex for no gain.
pub struct Painter<'a> {
    scene: &'a mut Scene,
}

impl Painter<'_> {
    /// Add `primitive` to the scene.
    ///
    /// # Panics
    ///
    /// On `PushClip` or `PopClip`: clip with [`Painter::clip`], which keeps clips
    /// balanced.
    pub fn paint(&mut self, primitive: Primitive) {
        assert!(
            !matches!(primitive, Primitive::PushClip { .. } | Primitive::PopClip),
            "canvas clips must go through Painter::clip so they stay balanced"
        );
        self.scene.insert(primitive);
    }

    /// Fill `bounds` with `color`.
    pub fn fill(&mut self, bounds: Bounds, color: impl Into<Color>) {
        self.scene.insert(Primitive::Quad {
            bounds,
            background: color.into().to_rgba(),
            border_color: Rgba::TRANSPARENT,
            border_widths: Edges::ZERO,
            corner_radii: Corners::default(),
        });
    }

    /// Paint `f` clipped to `bounds`, within the clip of the canvas.
    pub fn clip(&mut self, bounds: Bounds, f: impl FnOnce(&mut Painter)) {
        self.scene.push_layer(bounds);
        f(self);
        self.scene.pop_layer();
    }
}

type PaintFn = Box<dyn FnMut(Bounds, &mut Painter)>;
type PointerFn = Box<dyn Fn(&PointerEvent)>;

/// A leaf element that calls a closure with its bounds to paint primitives, clipped
/// to those bounds. Size it like a [`Div`](crate::elements::Div).
pub struct Canvas {
    id: StableId,
    style: Style,
    paint: PaintFn,
    on_pointer: Option<PointerFn>,
    pressed: bool,
}

/// A [`Canvas`] painted by `paint(bounds, painter)`.
pub fn canvas(paint: impl FnMut(Bounds, &mut Painter) + 'static) -> Canvas {
    Canvas {
        id: StableId::new(),
        style: Style::default(),
        paint: Box::new(paint),
        on_pointer: None,
        pressed: false,
    }
}

impl Canvas {
    pub fn id(mut self, id: ElementId) -> Self {
        self.id = StableId::explicit(id);
        self
    }

    pub fn w(mut self, width: f32) -> Self {
        self.style.set_width_dimension(width);
        self
    }

    pub fn h(mut self, height: f32) -> Self {
        self.style.set_height_dimension(height);
        self
    }

    pub fn w_full(mut self) -> Self {
        self.style.set_width_dimension(StyleDimension::fill());
        self.style.flex_grow = 1.0;
        self
    }

    pub fn h_full(mut self) -> Self {
        self.style.set_height_dimension(StyleDimension::fill());
        self.style.flex_grow = 1.0;
        self
    }

    pub fn flex_grow(mut self, grow: f32) -> Self {
        self.style.flex_grow = grow;
        self
    }

    /// Receive pointer events with positions relative to the canvas origin.
    ///
    /// Gets presses and releases inside the canvas, moves over it, and every move
    /// and the release of a press that started inside, so drags can leave it.
    pub fn on_pointer(mut self, handler: impl Fn(&PointerEvent) + 'static) -> Self {
        self.on_pointer = Some(Box::new(handler));
        self
    }
}

impl Element for Canvas {
    fn id(&self) -> Option<ElementId> {
        Some(self.id.get())
    }

    fn stable_id_mut(&mut self) -> Option<&mut StableId> {
        Some(&mut self.id)
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        cx.new_leaf(style_to_taffy(&self.style))
            .expect("Failed to create canvas layout node")
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        if self.on_pointer.is_some() {
            cx.register_hit_region(self.id.get(), bounds);
        }
        cx.scene.push_layer(bounds);
        (self.paint)(bounds, &mut Painter { scene: cx.scene });
        cx.scene.pop_layer();
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, event: &PointerEvent) -> bool {
        let Some(handler) = &self.on_pointer else {
            return false;
        };
        let inside = cx.contains_pointer(event.position);
        let deliver = match event.kind {
            PointerEventKind::Down => {
                self.pressed = inside;
                inside
            }
            PointerEventKind::Move => inside || self.pressed,
            PointerEventKind::Up => std::mem::take(&mut self.pressed) || inside,
        };
        if !deliver {
            return false;
        }
        let origin = cx.layout_bounds();
        handler(&PointerEvent {
            position: Point::new(event.position.x - origin.x(), event.position.y - origin.y()),
            ..*event
        });
        // Moves keep propagating so hover elsewhere stays current, as in `Div`.
        !matches!(event.kind, PointerEventKind::Move)
    }
}
