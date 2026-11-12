//! SVG icons drawn as path primitives.
//!
//! [`svg`] parses a practical subset of SVG (see [`parse`]) once per markup and
//! turns it into filled paths once per size and tint, kept for every element
//! that shows the same icon at the same size. What it cannot draw is an error
//! naming the element, shown in place of the icon.

mod geometry;
mod parse;

pub use parse::SvgError;

use crate::core::ElementId;
use crate::core::color::{Color, Rgba};
use crate::core::geometry::{Bounds, Edges, Size};
use crate::core::style::Style;
use crate::elements::element::{
    Capabilities, Element, LayoutContext, PaintContext, style_to_taffy,
};
use crate::elements::text::TextAlign;
use crate::renderer::Primitive;
use geometry::{Transform, fill_polygon, stroke_outline};
use parse::{ColorValue, Document};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use taffy::prelude::*;

/// Curves are flattened to within this many logical pixels.
const TOLERANCE: f32 = 0.05;

/// Icons at a size and tint kept; past this the cache starts over.
const SHAPED_ICONS: usize = 1024;

/// Markup kept by address; past this that cache starts over.
const ADDRESSED_MARKUP: usize = 1024;

/// A parsed document, with an id the shaped icons are kept by.
struct Parsed {
    id: u64,
    document: Document,
}

/// Markup, and what it parsed to.
type ParsedMarkup = (Rc<str>, Result<Rc<Parsed>, SvgError>);

/// Document id, size bits and tint bits.
type ShapeKey = (u64, [u32; 2], Option<[u32; 4]>);

#[derive(Default)]
struct Caches {
    /// By markup hash, with the markup to tell collisions apart.
    documents: HashMap<u64, Vec<ParsedMarkup>>,
    /// By the address and length of markup passed before, with the markup to
    /// check it is unchanged: markup kept in one place, as `include_str!` keeps
    /// it, is found every frame by comparing it rather than hashing it.
    addressed: HashMap<(usize, usize), ParsedMarkup>,
    next_id: u64,
    /// At the origin.
    shaped: HashMap<ShapeKey, Rc<[Primitive]>>,
}

thread_local! {
    static CACHES: RefCell<Caches> = RefCell::default();
}

fn parsed(markup: &str) -> Result<Rc<Parsed>, SvgError> {
    let address = (markup.as_ptr() as usize, markup.len());
    let found = CACHES.with(|caches| {
        let caches = caches.borrow();
        let (kept, parsed) = caches.addressed.get(&address)?;
        (**kept == *markup).then(|| parsed.clone())
    });
    if let Some(parsed) = found {
        return parsed;
    }
    let parsed = parsed_by_hash(markup);
    CACHES.with(|caches| {
        let mut caches = caches.borrow_mut();
        if caches.addressed.len() >= ADDRESSED_MARKUP {
            caches.addressed.clear();
        }
        caches
            .addressed
            .insert(address, (markup.into(), parsed.clone()));
    });
    parsed
}

fn parsed_by_hash(markup: &str) -> Result<Rc<Parsed>, SvgError> {
    let mut hasher = DefaultHasher::new();
    markup.hash(&mut hasher);
    let hash = hasher.finish();
    CACHES.with(|caches| {
        let mut caches = caches.borrow_mut();
        if let Some((_, parsed)) = caches
            .documents
            .get(&hash)
            .and_then(|entries| entries.iter().find(|(kept, _)| &**kept == markup))
        {
            return parsed.clone();
        }
        caches.next_id += 1;
        let id = caches.next_id;
        let parsed = parse::parse(markup).map(|document| Rc::new(Parsed { id, document }));
        caches
            .documents
            .entry(hash)
            .or_default()
            .push((markup.into(), parsed.clone()));
        parsed
    })
}

/// The icon's paths at the origin, `size` large, painted `tint` if given.
fn shaped(parsed: &Parsed, size: Size, tint: Option<Rgba>) -> Rc<[Primitive]> {
    let key = (
        parsed.id,
        [size.width.to_bits(), size.height.to_bits()],
        tint.map(|tint| tint.to_array().map(f32::to_bits)),
    );
    CACHES.with(|caches| {
        if let Some(primitives) = caches.borrow().shaped.get(&key) {
            return Rc::clone(primitives);
        }
        let primitives: Rc<[Primitive]> = shape(&parsed.document, size, tint).into();
        let mut caches = caches.borrow_mut();
        if caches.shaped.len() >= SHAPED_ICONS {
            caches.shaped.clear();
        }
        caches.shaped.insert(key, Rc::clone(&primitives));
        primitives
    })
}

fn shape(document: &Document, size: Size, tint: Option<Rgba>) -> Vec<Primitive> {
    let [x, y, width, height] = document.view_box;
    let (mut sx, mut sy) = (size.width / width, size.height / height);
    let (mut dx, mut dy) = (0.0, 0.0);
    if !document.stretch {
        let scale = sx.min(sy);
        dx = (size.width - width * scale) / 2.0;
        dy = (size.height - height * scale) / 2.0;
        (sx, sy) = (scale, scale);
    }
    let fit = Transform::translate(dx - x * sx, dy - y * sy).then(Transform::scale(sx, sy));
    let color = |paint: parse::Paint| {
        let base = tint.unwrap_or(match paint.color {
            ColorValue::Rgba(rgba) => rgba,
            ColorValue::Current => Rgba::BLACK,
        });
        let color = base.with_alpha(base.a * paint.opacity);
        (color.a > 0.0).then_some(color)
    };
    let mut primitives = Vec::new();
    for shape in &document.shapes {
        let lines: Vec<_> = shape
            .subpaths
            .iter()
            .map(|subpath| (subpath.transformed(fit).flatten(TOLERANCE), subpath.closed))
            .collect();
        if let Some(color) = shape.fill.and_then(color) {
            let contours: Vec<_> = lines.iter().map(|(points, _)| points.clone()).collect();
            let vertices = fill_polygon(&contours);
            if vertices.len() >= 3 {
                primitives.push(Primitive::Path {
                    vertices,
                    color,
                    stroke_width: None,
                });
            }
        }
        if let Some((paint, mut stroke)) = shape.stroke
            && let Some(color) = color(paint)
        {
            stroke.width *= fit.mean_scale();
            let mut contours = Vec::new();
            for (points, closed) in &lines {
                stroke_outline(points, *closed, stroke, TOLERANCE, &mut contours);
            }
            let vertices = fill_polygon(&contours);
            if vertices.len() >= 3 {
                primitives.push(Primitive::Path {
                    vertices,
                    color,
                    stroke_width: None,
                });
            }
        }
    }
    primitives
}

/// An SVG icon, sized like an image and drawn as paths.
pub struct Svg {
    id: Option<ElementId>,
    parsed: Result<Rc<Parsed>, SvgError>,
    style: Style,
    tint: Option<Color>,
}

impl Svg {
    /// The icon `markup` draws, or an error if it uses what is not supported.
    pub fn parse(markup: &str) -> Result<Self, SvgError> {
        let parsed = parsed(markup)?;
        Ok(Self::with(Ok(parsed)))
    }

    fn with(parsed: Result<Rc<Parsed>, SvgError>) -> Self {
        Self {
            id: None,
            parsed,
            style: Style::new(),
            tint: None,
        }
    }

    pub fn id(mut self, id: ElementId) -> Self {
        self.id = Some(id);
        self
    }

    /// Paint every fill and stroke `color`, keeping their opacities.
    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.tint = Some(color.into());
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
        let size = size.into();
        self.style.width = Some(size.width);
        self.style.height = Some(size.height);
        self
    }

    /// Why the markup cannot be drawn.
    pub fn error(&self) -> Option<&SvgError> {
        self.parsed.as_ref().err()
    }
}

/// An SVG icon of `markup`. Unsupported markup shows its error in place of the
/// icon; [`Svg::parse`] returns it instead.
pub fn svg(markup: &str) -> Svg {
    Svg::with(parsed(markup))
}

impl Element for Svg {
    fn id(&self) -> Option<ElementId> {
        self.id
    }

    fn style(&self) -> &Style {
        &self.style
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::PURE_INERT
    }

    fn same_props(&self, previous: &dyn Element) -> bool {
        let Some(previous) = (previous as &dyn Any).downcast_ref::<Svg>() else {
            return false;
        };
        let parsed = match (&self.parsed, &previous.parsed) {
            (Ok(a), Ok(b)) => a.id == b.id,
            (Err(a), Err(b)) => a == b,
            _ => false,
        };
        parsed
            && self.id == previous.id
            && self.style == previous.style
            && self.tint == previous.tint
    }

    fn layout(&mut self, cx: &mut LayoutContext) -> NodeId {
        let mut style = style_to_taffy(&self.style);
        // At the viewBox size unless sized.
        if let Ok(parsed) = &self.parsed
            && self.style.width.is_none()
            && self.style.height.is_none()
        {
            let [_, _, width, height] = parsed.document.view_box;
            style.size = taffy::Size {
                width: Dimension::Length(width),
                height: Dimension::Length(height),
            };
        }
        cx.new_leaf(style)
            .expect("Failed to create svg layout node")
    }

    fn paint(&mut self, cx: &mut PaintContext) {
        let bounds = cx.bounds();
        let parsed = match &self.parsed {
            Ok(parsed) => parsed,
            Err(error) => {
                paint_error(cx, bounds, &error.to_string());
                return;
            }
        };
        if bounds.is_empty() {
            return;
        }
        let tint = self.tint.map(|tint| tint.to_rgba());
        for primitive in shaped(parsed, bounds.size, tint).iter() {
            let mut primitive = primitive.clone();
            primitive.translate(bounds.x(), bounds.y());
            cx.paint(primitive);
        }
    }
}

fn paint_error(cx: &mut PaintContext, bounds: Bounds, message: &str) {
    cx.paint(Primitive::Quad {
        bounds,
        background: Color::hex(0xfee2e2).to_rgba(),
        border_color: Color::hex(0xb91c1c).to_rgba(),
        border_widths: Edges::all(1.0),
        corner_radii: Default::default(),
    });
    cx.paint(Primitive::Text {
        bounds,
        content: message.to_string().into(),
        color: Color::hex(0xb91c1c).to_rgba(),
        font_size: 11.0,
        font_weight: 400,
        font_family: None,
        line_height: 1.2,
        align: TextAlign::Left,
    });
}

#[cfg(test)]
mod tests;
