//! A practical SVG subset: `svg`, `g`, `path`, `rect`, `circle`, `ellipse`,
//! `line`, `polyline` and `polygon`, with transforms, solid fill and stroke
//! colors, stroke widths, caps and joins, and opacities. Anything else is an
//! error naming the element, never drawn as something it is not.

use super::geometry::{
    LineCap, LineJoin, Numbers, PathBuilder, Point, StrokeShape, Subpath, Transform,
    parse_path_data, parse_points,
};
use crate::core::color::Rgba;
use std::fmt;

/// Why markup could not be drawn as an SVG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgError {
    /// The element, as `<path id="x">`, or empty for the document itself.
    pub element: String,
    pub message: String,
}

impl fmt::Display for SvgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.element.is_empty() {
            write!(f, "svg: {}", self.message)
        } else {
            write!(f, "svg {}: {}", self.element, self.message)
        }
    }
}

impl std::error::Error for SvgError {}

/// A color, or `currentColor`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ColorValue {
    Rgba(Rgba),
    Current,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Paint {
    pub color: ColorValue,
    pub opacity: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Shape {
    /// In viewBox units.
    pub subpaths: Vec<Subpath>,
    pub fill: Option<Paint>,
    pub stroke: Option<(Paint, StrokeShape)>,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Document {
    /// x, y, width, height.
    pub view_box: [f32; 4],
    /// `preserveAspectRatio="none"`: stretch instead of fit and centre.
    pub stretch: bool,
    pub shapes: Vec<Shape>,
}

struct Node {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<Node>,
}

impl Node {
    fn label(&self) -> String {
        match self.attribute("id") {
            Some(id) => format!("<{} id=\"{id}\">", self.name),
            None => format!("<{}>", self.name),
        }
    }

    fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    fn error(&self, message: impl Into<String>) -> SvgError {
        SvgError {
            element: self.label(),
            message: message.into(),
        }
    }
}

/// Inherited presentation state.
#[derive(Clone)]
struct Style {
    transform: Transform,
    fill: Option<ColorValue>,
    stroke: Option<ColorValue>,
    current: ColorValue,
    stroke_width: f32,
    cap: LineCap,
    join: LineJoin,
    miter_limit: f32,
    opacity: f32,
    fill_opacity: f32,
    stroke_opacity: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            transform: Transform::IDENTITY,
            fill: Some(ColorValue::Rgba(Rgba::BLACK)),
            stroke: None,
            current: ColorValue::Current,
            stroke_width: 1.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter_limit: 4.0,
            opacity: 1.0,
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
        }
    }
}

/// Attributes of every element that change nothing drawn.
fn ignored(name: &str) -> bool {
    matches!(
        name,
        "id" | "class" | "version" | "baseProfile" | "role" | "focusable" | "clip-rule"
    ) || name.starts_with("xmlns")
        || name.starts_with("xml:")
        || name.starts_with("data-")
        || name.starts_with("aria-")
}

pub(super) fn parse(markup: &str) -> Result<Document, SvgError> {
    let root = Xml::new(markup).document()?;
    if root.name != "svg" {
        return Err(root.error("the document is not an <svg>"));
    }
    let size = |name| {
        root.attribute(name)
            .map(|value| length(value).map_err(|err| root.error(format!("{name}: {err}"))))
            .transpose()
    };
    let (width, height) = (size("width")?, size("height")?);
    let view_box = match root.attribute("viewBox") {
        Some(value) => {
            let mut numbers = Numbers::new(value);
            let mut parts = [0.0; 4];
            for part in &mut parts {
                *part = numbers
                    .number()
                    .map_err(|err| root.error(format!("viewBox: {err}")))?;
            }
            if !numbers.is_done() || parts[2] <= 0.0 || parts[3] <= 0.0 {
                return Err(root.error(format!("viewBox {value:?} is not x y width height")));
            }
            parts
        }
        None => match (width, height) {
            (Some(width), Some(height)) if width > 0.0 && height > 0.0 => [0.0, 0.0, width, height],
            _ => return Err(root.error("needs a viewBox, or a width and a height")),
        },
    };
    let stretch = match root.attribute("preserveAspectRatio").map(str::trim) {
        None | Some("xMidYMid") | Some("xMidYMid meet") => false,
        Some("none") => true,
        Some(other) => {
            return Err(root.error(format!(
                "preserveAspectRatio {other:?} is not supported, only xMidYMid meet and none"
            )));
        }
    };
    let mut shapes = Vec::new();
    let style = apply_style(
        &root,
        Style::default(),
        &[
            "width",
            "height",
            "viewBox",
            "preserveAspectRatio",
            "x",
            "y",
        ],
    )?;
    for child in &root.children {
        walk(child, &style, &mut shapes)?;
    }
    Ok(Document {
        view_box,
        stretch,
        shapes,
    })
}

fn walk(node: &Node, parent: &Style, shapes: &mut Vec<Shape>) -> Result<(), SvgError> {
    let own: &[&str] = match node.name.as_str() {
        "title" | "desc" | "metadata" => return Ok(()),
        "defs" if node.children.is_empty() => return Ok(()),
        "g" => &[],
        "path" => &["d"],
        "rect" => &["x", "y", "width", "height", "rx", "ry"],
        "circle" => &["cx", "cy", "r"],
        "ellipse" => &["cx", "cy", "rx", "ry"],
        "line" => &["x1", "y1", "x2", "y2"],
        "polyline" | "polygon" => &["points"],
        _ => return Err(node.error("this element is not supported")),
    };
    let style = apply_style(node, parent.clone(), own)?;
    if node.attribute("display").map(str::trim) == Some("none") {
        return Ok(());
    }
    if node.name == "g" {
        for child in &node.children {
            walk(child, &style, shapes)?;
        }
        return Ok(());
    }
    if let Some(child) = node.children.first() {
        return Err(child.error(format!("cannot be inside <{}>", node.name)));
    }
    let number = |name: &str| -> Result<f32, SvgError> {
        node.attribute(name)
            .map_or(Ok(0.0), length)
            .map_err(|err| node.error(format!("{name}: {err}")))
    };
    let mut builder = PathBuilder::default();
    match node.name.as_str() {
        "path" => {
            let data = node.attribute("d").unwrap_or("");
            let subpaths = parse_path_data(data).map_err(|err| node.error(format!("d: {err}")))?;
            return push_shape(node, &style, subpaths, shapes);
        }
        "rect" => {
            let (width, height) = (number("width")?, number("height")?);
            let (rx, ry) = match (node.attribute("rx"), node.attribute("ry")) {
                (None, None) => (0.0, 0.0),
                (Some(_), None) => (number("rx")?, number("rx")?),
                (None, Some(_)) => (number("ry")?, number("ry")?),
                (Some(_), Some(_)) => (number("rx")?, number("ry")?),
            };
            if width > 0.0 && height > 0.0 {
                let origin = Point::new(number("x")?, number("y")?);
                builder.rect(
                    origin,
                    width,
                    height,
                    rx.min(width / 2.0),
                    ry.min(height / 2.0),
                );
            }
        }
        "circle" => {
            let r = number("r")?;
            if r > 0.0 {
                builder.ellipse(Point::new(number("cx")?, number("cy")?), r, r);
            }
        }
        "ellipse" => {
            let (rx, ry) = (number("rx")?, number("ry")?);
            if rx > 0.0 && ry > 0.0 {
                builder.ellipse(Point::new(number("cx")?, number("cy")?), rx, ry);
            }
        }
        "line" => {
            builder.move_to(Point::new(number("x1")?, number("y1")?));
            builder.line_to(Point::new(number("x2")?, number("y2")?));
        }
        _ => {
            let points = parse_points(node.attribute("points").unwrap_or(""))
                .map_err(|err| node.error(format!("points: {err}")))?;
            if let Some((first, rest)) = points.split_first() {
                builder.move_to(*first);
                for point in rest {
                    builder.line_to(*point);
                }
                if node.name == "polygon" {
                    builder.close();
                }
            }
        }
    }
    push_shape(node, &style, builder.finish(), shapes)
}

fn push_shape(
    node: &Node,
    style: &Style,
    subpaths: Vec<Subpath>,
    shapes: &mut Vec<Shape>,
) -> Result<(), SvgError> {
    if subpaths.is_empty() {
        return Ok(());
    }
    let resolve = |color: Option<ColorValue>, opacity: f32| {
        color.map(|color| Paint {
            color: match color {
                ColorValue::Current => style.current,
                rgba => rgba,
            },
            opacity: style.opacity * opacity,
        })
    };
    // A line has nothing to fill.
    let fill = if node.name == "line" {
        None
    } else {
        resolve(style.fill, style.fill_opacity)
    };
    let stroke = resolve(style.stroke, style.stroke_opacity)
        .filter(|_| style.stroke_width > 0.0)
        .map(|paint| {
            (
                paint,
                StrokeShape {
                    width: style.stroke_width * style.transform.mean_scale(),
                    cap: style.cap,
                    join: style.join,
                    miter_limit: style.miter_limit,
                },
            )
        });
    if fill.is_none() && stroke.is_none() {
        return Ok(());
    }
    shapes.push(Shape {
        subpaths: subpaths
            .iter()
            .map(|subpath| subpath.transformed(style.transform))
            .collect(),
        fill,
        stroke,
    });
    Ok(())
}

/// The element's presentation attributes and `style` over the inherited ones.
/// `own` are the element's geometry attributes, read elsewhere.
fn apply_style(node: &Node, mut style: Style, own: &[&str]) -> Result<Style, SvgError> {
    let declarations = node
        .attribute("style")
        .into_iter()
        .flat_map(|css| css.split(';'))
        .filter_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            Some((name.trim().to_string(), value.trim().to_string()))
        });
    let attributes = node
        .attributes
        .iter()
        .filter(|(name, _)| name != "style")
        .cloned()
        // Declarations in `style` win over attributes.
        .chain(declarations)
        .collect::<Vec<_>>();
    // `color` first: fill and stroke may say currentColor.
    for (name, value) in attributes.iter().filter(|(name, _)| name == "color") {
        style.current = color(value).map_err(|err| node.error(format!("{name}: {err}")))?;
    }
    for (name, value) in &attributes {
        let error = |message: String| node.error(format!("{name}: {message}"));
        let value = value.trim();
        match name.as_str() {
            "color" | "display" => {}
            "transform" => style.transform = style.transform.then(transform(value).map_err(error)?),
            "fill" => style.fill = paint(value).map_err(error)?,
            "stroke" => style.stroke = paint(value).map_err(error)?,
            "stroke-width" => style.stroke_width = length(value).map_err(error)?,
            "stroke-linecap" => {
                style.cap = match value {
                    "butt" => LineCap::Butt,
                    "round" => LineCap::Round,
                    "square" => LineCap::Square,
                    _ => return Err(error(format!("{value:?} is not butt, round or square"))),
                }
            }
            "stroke-linejoin" => {
                style.join = match value {
                    "miter" => LineJoin::Miter,
                    "round" => LineJoin::Round,
                    "bevel" => LineJoin::Bevel,
                    _ => return Err(error(format!("{value:?} is not miter, round or bevel"))),
                }
            }
            "stroke-miterlimit" => style.miter_limit = number(value).map_err(error)?.max(1.0),
            "opacity" => style.opacity *= unit(value).map_err(error)?,
            "fill-opacity" => style.fill_opacity = unit(value).map_err(error)?,
            "stroke-opacity" => style.stroke_opacity = unit(value).map_err(error)?,
            "fill-rule" if value == "nonzero" => {}
            "fill-rule" => return Err(error(format!("{value:?} is not supported, only nonzero"))),
            "stroke-dasharray" if value == "none" => {}
            "visibility" if value == "visible" => {}
            _ if own.contains(&name.as_str()) || ignored(name) => {}
            _ => return Err(node.error(format!("the attribute {name} is not supported"))),
        }
    }
    Ok(style)
}

fn number(value: &str) -> Result<f32, String> {
    let mut numbers = Numbers::new(value);
    let number = numbers.number()?;
    if !numbers.is_done() {
        return Err(format!("{value:?} is not a number"));
    }
    Ok(number)
}

/// A number in user units: plain, or in `px`.
fn length(value: &str) -> Result<f32, String> {
    let value = value.trim();
    number(value.strip_suffix("px").unwrap_or(value))
        .map_err(|_| format!("{value:?} is not a length in user units or px"))
}

fn unit(value: &str) -> Result<f32, String> {
    Ok(number(value)?.clamp(0.0, 1.0))
}

fn paint(value: &str) -> Result<Option<ColorValue>, String> {
    if value == "none" {
        return Ok(None);
    }
    if value.starts_with("url(") {
        return Err(format!(
            "{value:?}: gradients and patterns are not supported"
        ));
    }
    color(value).map(Some)
}

fn color(value: &str) -> Result<ColorValue, String> {
    let rgb = |r: u8, g: u8, b: u8| Ok(ColorValue::Rgba(Rgba::from_u8(r, g, b, 255)));
    if value == "currentColor" {
        return Ok(ColorValue::Current);
    }
    if let Some(hex) = value.strip_prefix('#') {
        let digits: Vec<u8> = hex
            .chars()
            .map(|c| c.to_digit(16).map(|digit| digit as u8))
            .collect::<Option<_>>()
            .ok_or_else(|| format!("{value:?} is not a hex color"))?;
        return match digits.as_slice() {
            [r, g, b] => rgb(r * 17, g * 17, b * 17),
            [r1, r0, g1, g0, b1, b0] => rgb(r1 * 16 + r0, g1 * 16 + g0, b1 * 16 + b0),
            _ => Err(format!("{value:?} is not #rgb or #rrggbb")),
        };
    }
    if let Some(inner) = value
        .strip_prefix("rgb(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let parts: Vec<u8> = inner
            .split(',')
            .map(|part| part.trim().parse::<u8>().ok())
            .collect::<Option<_>>()
            .filter(|parts: &Vec<u8>| parts.len() == 3)
            .ok_or_else(|| format!("{value:?} is not rgb(r, g, b) of 0 to 255"))?;
        return rgb(parts[0], parts[1], parts[2]);
    }
    match value {
        "black" => rgb(0, 0, 0),
        "white" => rgb(255, 255, 255),
        "red" => rgb(255, 0, 0),
        "green" => rgb(0, 128, 0),
        "lime" => rgb(0, 255, 0),
        "blue" => rgb(0, 0, 255),
        "yellow" => rgb(255, 255, 0),
        "orange" => rgb(255, 165, 0),
        "purple" => rgb(128, 0, 128),
        "gray" | "grey" => rgb(128, 128, 128),
        "silver" => rgb(192, 192, 192),
        "transparent" => Ok(ColorValue::Rgba(Rgba::TRANSPARENT)),
        _ => Err(format!("{value:?} is not a supported color")),
    }
}

fn transform(value: &str) -> Result<Transform, String> {
    let mut result = Transform::IDENTITY;
    let mut rest = value.trim();
    while !rest.is_empty() {
        let open = rest
            .find('(')
            .ok_or_else(|| format!("{rest:?} is not a transform"))?;
        let close = rest
            .find(')')
            .ok_or_else(|| format!("{rest:?} has no closing parenthesis"))?;
        let name = rest[..open].trim();
        let mut numbers = Numbers::new(&rest[open + 1..close]);
        let mut args = Vec::new();
        while !numbers.is_done() {
            args.push(numbers.number()?);
        }
        let next = match (name, args.as_slice()) {
            ("matrix", &[a, b, c, d, e, f]) => Transform([a, b, c, d, e, f]),
            ("translate", &[x]) => Transform::translate(x, 0.0),
            ("translate", &[x, y]) => Transform::translate(x, y),
            ("scale", &[s]) => Transform::scale(s, s),
            ("scale", &[x, y]) => Transform::scale(x, y),
            ("rotate", &[a]) => Transform::rotate(a),
            ("rotate", &[a, x, y]) => Transform::translate(x, y)
                .then(Transform::rotate(a))
                .then(Transform::translate(-x, -y)),
            ("skewX", &[a]) => Transform([1.0, 0.0, a.to_radians().tan(), 1.0, 0.0, 0.0]),
            ("skewY", &[a]) => Transform([1.0, a.to_radians().tan(), 0.0, 1.0, 0.0, 0.0]),
            _ => return Err(format!("{name}({args:?}) is not a transform")),
        };
        result = result.then(next);
        rest = rest[close + 1..].trim_start_matches(|c: char| c.is_whitespace() || c == ',');
    }
    Ok(result)
}

/// Enough XML for SVG files: elements, attributes, comments, declarations.
struct Xml<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Xml<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, at: 0 }
    }

    fn rest(&self) -> &'a str {
        &self.text[self.at..]
    }

    fn error(&self, message: impl Into<String>) -> SvgError {
        let line = self.text[..self.at].matches('\n').count() + 1;
        SvgError {
            element: String::new(),
            message: format!("line {line}: {}", message.into()),
        }
    }

    /// Skip whitespace, comments, `<?...?>` and `<!DOCTYPE ...>`.
    fn skip_misc(&mut self) -> Result<(), SvgError> {
        loop {
            let trimmed = self.rest().trim_start();
            self.at = self.text.len() - trimmed.len();
            let (open, close) = if trimmed.starts_with("<!--") {
                ("<!--", "-->")
            } else if trimmed.starts_with("<?") {
                ("<?", "?>")
            } else if trimmed.starts_with("<!DOCTYPE") {
                ("<!DOCTYPE", ">")
            } else {
                return Ok(());
            };
            let end = trimmed[open.len()..]
                .find(close)
                .ok_or_else(|| self.error(format!("{open} is never closed")))?;
            self.at += open.len() + end + close.len();
        }
    }

    fn document(&mut self) -> Result<Node, SvgError> {
        self.skip_misc()?;
        let root = self.element()?;
        self.skip_misc()?;
        if !self.rest().is_empty() {
            return Err(self.error("text after the root element"));
        }
        Ok(root)
    }

    fn name(&mut self) -> &'a str {
        let rest = self.rest();
        let end = rest
            .find(|c: char| !(c.is_alphanumeric() || matches!(c, '-' | '_' | ':' | '.')))
            .unwrap_or(rest.len());
        self.at += end;
        &rest[..end]
    }

    fn element(&mut self) -> Result<Node, SvgError> {
        if !self.rest().starts_with('<') {
            return Err(self.error("expected an element"));
        }
        self.at += 1;
        let name = self.name().to_string();
        if name.is_empty() {
            return Err(self.error("expected an element name"));
        }
        let mut node = Node {
            name,
            attributes: Vec::new(),
            children: Vec::new(),
        };
        loop {
            let trimmed = self.rest().trim_start();
            self.at = self.text.len() - trimmed.len();
            if let Some(after) = trimmed.strip_prefix("/>") {
                self.at = self.text.len() - after.len();
                return Ok(node);
            }
            if let Some(after) = trimmed.strip_prefix('>') {
                self.at = self.text.len() - after.len();
                break;
            }
            let key = self.name().to_string();
            if key.is_empty() {
                return Err(self.error(format!("bad attribute in <{}>", node.name)));
            }
            let rest = self.rest().trim_start();
            let Some(rest) = rest.strip_prefix('=') else {
                return Err(self.error(format!("attribute {key} has no value")));
            };
            let rest = rest.trim_start();
            let quote = rest
                .chars()
                .next()
                .filter(|c| matches!(c, '"' | '\''))
                .ok_or_else(|| self.error(format!("attribute {key} is not quoted")))?;
            let value_end = rest[1..]
                .find(quote)
                .ok_or_else(|| self.error(format!("attribute {key} is never closed")))?;
            let value = unescape(&rest[1..1 + value_end]);
            self.at = self.text.len() - rest.len() + value_end + 2;
            node.attributes.push((key, value));
        }
        // Children, until the closing tag. Text between them is ignored.
        loop {
            let text_end = self
                .rest()
                .find('<')
                .ok_or_else(|| self.error(format!("<{}> is never closed", node.name)))?;
            self.at += text_end;
            if self.rest().starts_with("<!--") || self.rest().starts_with("<?") {
                self.skip_misc()?;
                continue;
            }
            if self.rest().starts_with("<![CDATA[") {
                return Err(self.error(format!("CDATA in <{}> is not supported", node.name)));
            }
            if let Some(after) = self.rest().strip_prefix("</") {
                self.at = self.text.len() - after.len();
                let name = self.name();
                if name != node.name {
                    return Err(self.error(format!("</{name}> closes <{}>", node.name)));
                }
                let rest = self.rest().trim_start();
                let Some(after) = rest.strip_prefix('>') else {
                    return Err(self.error(format!("</{name} is not closed")));
                };
                self.at = self.text.len() - after.len();
                return Ok(node);
            }
            node.children.push(self.element()?);
        }
    }
}

fn unescape(value: &str) -> String {
    value
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}
