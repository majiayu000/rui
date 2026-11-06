//! SVG geometry: path data, transforms, flattening, and strokes as fills.
//!
//! The path primitive fills one polygon with the nonzero rule. Several contours
//! go into one polygon joined to a common anchor and back, along the same edge
//! both ways, so the joins add nothing. A stroke is filled too: a rectangle per
//! segment and a piece per join and cap, all wound the same way, so where they
//! overlap the winding only grows and coverage, clamped to one, stays one.

use crate::renderer::PathVertex;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(super) struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }

    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }

    fn scale(self, by: f32) -> Self {
        Self::new(self.x * by, self.y * by)
    }

    fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    fn cross(self, other: Self) -> f32 {
        self.x * other.y - self.y * other.x
    }

    fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Segment {
    Line(Point),
    Cubic(Point, Point, Point),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Subpath {
    pub start: Point,
    pub segments: Vec<Segment>,
    pub closed: bool,
}

/// `[a b c d e f]`: x' = a x + c y + e, y' = b x + d y + f.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Transform(pub [f32; 6]);

impl Transform {
    pub const IDENTITY: Self = Self([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    pub fn translate(x: f32, y: f32) -> Self {
        Self([1.0, 0.0, 0.0, 1.0, x, y])
    }

    pub fn scale(x: f32, y: f32) -> Self {
        Self([x, 0.0, 0.0, y, 0.0, 0.0])
    }

    pub fn rotate(degrees: f32) -> Self {
        let (sin, cos) = degrees.to_radians().sin_cos();
        Self([cos, sin, -sin, cos, 0.0, 0.0])
    }

    /// `self` applied after `inner`.
    pub fn then(self, inner: Self) -> Self {
        let [a, b, c, d, e, f] = self.0;
        let [p, q, r, s, t, u] = inner.0;
        Self([
            a * p + c * q,
            b * p + d * q,
            a * r + c * s,
            b * r + d * s,
            a * t + c * u + e,
            b * t + d * u + f,
        ])
    }

    pub fn apply(self, point: Point) -> Point {
        let [a, b, c, d, e, f] = self.0;
        Point::new(a * point.x + c * point.y + e, b * point.x + d * point.y + f)
    }

    /// How much the transform scales lengths, on average over directions.
    pub fn mean_scale(self) -> f32 {
        let [a, b, c, d, ..] = self.0;
        (a * d - b * c).abs().sqrt()
    }
}

impl Subpath {
    pub fn transformed(&self, transform: Transform) -> Self {
        let apply = |point| transform.apply(point);
        Self {
            start: apply(self.start),
            segments: self
                .segments
                .iter()
                .map(|segment| match *segment {
                    Segment::Line(to) => Segment::Line(apply(to)),
                    Segment::Cubic(a, b, to) => Segment::Cubic(apply(a), apply(b), apply(to)),
                })
                .collect(),
            closed: self.closed,
        }
    }

    /// The subpath as points no further than `tolerance` from the curve.
    pub fn flatten(&self, tolerance: f32) -> Vec<Point> {
        let mut points = vec![self.start];
        let mut from = self.start;
        for segment in &self.segments {
            match *segment {
                Segment::Line(to) => {
                    points.push(to);
                    from = to;
                }
                Segment::Cubic(a, b, to) => {
                    // A chord of a step h strays at most h^2 / 8 times the
                    // largest second derivative, which is 6 times the largest
                    // second difference of the control points.
                    let second = from
                        .sub(a.scale(2.0))
                        .add(b)
                        .length()
                        .max(a.sub(b.scale(2.0)).add(to).length());
                    let steps = (0.75 * second / tolerance).sqrt().ceil().clamp(1.0, 256.0);
                    for step in 1..=steps as usize {
                        let t = step as f32 / steps;
                        let mt = 1.0 - t;
                        let point = from
                            .scale(mt * mt * mt)
                            .add(a.scale(3.0 * mt * mt * t))
                            .add(b.scale(3.0 * mt * t * t))
                            .add(to.scale(t * t * t));
                        points.push(point);
                    }
                    from = to;
                }
            }
        }
        points.dedup_by(|a, b| a.sub(*b).length() < 1e-4);
        if self.closed
            && points.len() > 1
            && points[0].sub(points[points.len() - 1]).length() < 1e-4
        {
            points.pop();
        }
        points
    }
}

/// Builds subpaths from SVG path commands and shapes.
#[derive(Default)]
pub(super) struct PathBuilder {
    subpaths: Vec<Subpath>,
    current: Option<Subpath>,
    point: Point,
}

impl PathBuilder {
    pub fn finish(mut self) -> Vec<Subpath> {
        self.flush();
        self.subpaths
    }

    fn flush(&mut self) {
        if let Some(subpath) = self.current.take()
            && !subpath.segments.is_empty()
        {
            self.subpaths.push(subpath);
        }
    }

    pub fn move_to(&mut self, to: Point) {
        self.flush();
        self.current = Some(Subpath {
            start: to,
            segments: Vec::new(),
            closed: false,
        });
        self.point = to;
    }

    fn segment(&mut self, segment: Segment, to: Point) {
        let start = self.point;
        self.current
            .get_or_insert_with(|| Subpath {
                start,
                segments: Vec::new(),
                closed: false,
            })
            .segments
            .push(segment);
        self.point = to;
    }

    pub fn line_to(&mut self, to: Point) {
        self.segment(Segment::Line(to), to);
    }

    pub fn cubic_to(&mut self, a: Point, b: Point, to: Point) {
        self.segment(Segment::Cubic(a, b, to), to);
    }

    pub fn quad_to(&mut self, control: Point, to: Point) {
        let from = self.point;
        let a = from.add(control.sub(from).scale(2.0 / 3.0));
        let b = to.add(control.sub(to).scale(2.0 / 3.0));
        self.cubic_to(a, b, to);
    }

    /// An elliptical arc as SVG's `A` command gives it, as cubics.
    pub fn arc_to(&mut self, radii: Point, rotation: f32, large: bool, sweep: bool, to: Point) {
        let from = self.point;
        let (mut rx, mut ry) = (radii.x.abs(), radii.y.abs());
        if rx == 0.0 || ry == 0.0 || from == to {
            self.line_to(to);
            return;
        }
        // The SVG 1.1 implementation notes, F.6.5 and F.6.6.
        let (sin, cos) = rotation.to_radians().sin_cos();
        let half = from.sub(to).scale(0.5);
        let x1 = cos * half.x + sin * half.y;
        let y1 = -sin * half.x + cos * half.y;
        let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
        if lambda > 1.0 {
            rx *= lambda.sqrt();
            ry *= lambda.sqrt();
        }
        let numerator = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
        let denominator = rx * rx * y1 * y1 + ry * ry * x1 * x1;
        let mut root = (numerator / denominator).max(0.0).sqrt();
        if large == sweep {
            root = -root;
        }
        let cx1 = root * rx * y1 / ry;
        let cy1 = -root * ry * x1 / rx;
        let mid = from.add(to).scale(0.5);
        let center = Point::new(cos * cx1 - sin * cy1 + mid.x, sin * cx1 + cos * cy1 + mid.y);
        let angle = |ux: f32, uy: f32| uy.atan2(ux);
        let start = angle((x1 - cx1) / rx, (y1 - cy1) / ry);
        let mut delta = angle((-x1 - cx1) / rx, (-y1 - cy1) / ry) - start;
        let tau = std::f32::consts::TAU;
        if sweep && delta < 0.0 {
            delta += tau;
        } else if !sweep && delta > 0.0 {
            delta -= tau;
        }
        // At most a quarter turn per cubic.
        let pieces = (delta.abs() / (tau / 4.0)).ceil().max(1.0) as usize;
        let step = delta / pieces as f32;
        let k = 4.0 / 3.0 * (step / 4.0).tan();
        let on_ellipse = |theta: f32| {
            let (s, c) = theta.sin_cos();
            Point::new(
                center.x + rx * c * cos - ry * s * sin,
                center.y + rx * c * sin + ry * s * cos,
            )
        };
        let tangent = |theta: f32| {
            let (s, c) = theta.sin_cos();
            Point::new(-rx * s * cos - ry * c * sin, -rx * s * sin + ry * c * cos)
        };
        for piece in 0..pieces {
            let t0 = start + step * piece as f32;
            let t1 = t0 + step;
            let p0 = on_ellipse(t0);
            let p1 = if piece + 1 == pieces {
                to
            } else {
                on_ellipse(t1)
            };
            let a = p0.add(tangent(t0).scale(k));
            let b = p1.sub(tangent(t1).scale(k));
            self.cubic_to(a, b, p1);
        }
    }

    pub fn close(&mut self) {
        if let Some(mut subpath) = self.current.take() {
            let start = subpath.start;
            if !subpath.segments.is_empty() {
                subpath.closed = true;
                self.subpaths.push(subpath);
            }
            self.point = start;
        }
    }

    /// An ellipse, clockwise from its right end.
    pub fn ellipse(&mut self, center: Point, rx: f32, ry: f32) {
        self.move_to(Point::new(center.x + rx, center.y));
        for end in [(0.0, ry), (-rx, 0.0), (0.0, -ry), (rx, 0.0)] {
            self.arc_to(
                Point::new(rx, ry),
                0.0,
                false,
                true,
                Point::new(center.x + end.0, center.y + end.1),
            );
        }
        self.close();
    }

    /// A rectangle, its corners rounded by `rx` and `ry`.
    pub fn rect(&mut self, origin: Point, width: f32, height: f32, rx: f32, ry: f32) {
        let (x0, y0, x1, y1) = (origin.x, origin.y, origin.x + width, origin.y + height);
        if rx <= 0.0 || ry <= 0.0 {
            self.move_to(Point::new(x0, y0));
            self.line_to(Point::new(x1, y0));
            self.line_to(Point::new(x1, y1));
            self.line_to(Point::new(x0, y1));
            self.close();
            return;
        }
        let radii = Point::new(rx, ry);
        self.move_to(Point::new(x0 + rx, y0));
        self.line_to(Point::new(x1 - rx, y0));
        self.arc_to(radii, 0.0, false, true, Point::new(x1, y0 + ry));
        self.line_to(Point::new(x1, y1 - ry));
        self.arc_to(radii, 0.0, false, true, Point::new(x1 - rx, y1));
        self.line_to(Point::new(x0 + rx, y1));
        self.arc_to(radii, 0.0, false, true, Point::new(x0, y1 - ry));
        self.line_to(Point::new(x0, y0 + ry));
        self.arc_to(radii, 0.0, false, true, Point::new(x0 + rx, y0));
        self.close();
    }
}

/// Parse SVG path data, the `d` attribute.
pub(super) fn parse_path_data(data: &str) -> Result<Vec<Subpath>, String> {
    let mut numbers = Numbers::new(data);
    let mut builder = PathBuilder::default();
    let mut command = None;
    // The last cubic or quadratic control point, for S and T.
    let mut last_cubic: Option<Point> = None;
    let mut last_quad: Option<Point> = None;
    loop {
        numbers.skip_separators();
        let Some(next) = numbers.peek() else {
            break;
        };
        if next.is_ascii_alphabetic() {
            numbers.bump();
            command = Some(next);
        } else if command.is_none() {
            return Err(format!("path data starts with {next:?}, not a command"));
        }
        let Some(letter) = command else {
            unreachable!("a command was set above");
        };
        let relative = letter.is_ascii_lowercase();
        let origin = if relative {
            builder.point
        } else {
            Point::new(0.0, 0.0)
        };
        let point = |numbers: &mut Numbers| -> Result<Point, String> {
            Ok(origin.add(Point::new(numbers.number()?, numbers.number()?)))
        };
        let (mut cubic, mut quad) = (None, None);
        match letter.to_ascii_uppercase() {
            'M' => {
                let to = point(&mut numbers)?;
                builder.move_to(to);
                // Pairs after the first are line-tos.
                command = Some(if relative { 'l' } else { 'L' });
            }
            'L' => builder.line_to(point(&mut numbers)?),
            'H' => {
                let x = numbers.number()? + if relative { builder.point.x } else { 0.0 };
                builder.line_to(Point::new(x, builder.point.y));
            }
            'V' => {
                let y = numbers.number()? + if relative { builder.point.y } else { 0.0 };
                builder.line_to(Point::new(builder.point.x, y));
            }
            'C' => {
                let (a, b, to) = (
                    point(&mut numbers)?,
                    point(&mut numbers)?,
                    point(&mut numbers)?,
                );
                builder.cubic_to(a, b, to);
                cubic = Some(b);
            }
            'S' => {
                let from = builder.point;
                let a = last_cubic.map_or(from, |b| from.add(from.sub(b)));
                let (b, to) = (point(&mut numbers)?, point(&mut numbers)?);
                builder.cubic_to(a, b, to);
                cubic = Some(b);
            }
            'Q' => {
                let (control, to) = (point(&mut numbers)?, point(&mut numbers)?);
                builder.quad_to(control, to);
                quad = Some(control);
            }
            'T' => {
                let from = builder.point;
                let control = last_quad.map_or(from, |c| from.add(from.sub(c)));
                let to = point(&mut numbers)?;
                builder.quad_to(control, to);
                quad = Some(control);
            }
            'A' => {
                let radii = Point::new(numbers.number()?, numbers.number()?);
                let rotation = numbers.number()?;
                let large = numbers.flag()?;
                let sweep = numbers.flag()?;
                let to = point(&mut numbers)?;
                builder.arc_to(radii, rotation, large, sweep, to);
            }
            'Z' => {
                builder.close();
                command = None;
            }
            _ => return Err(format!("unknown path command {letter:?}")),
        }
        last_cubic = cubic;
        last_quad = quad;
    }
    Ok(builder.finish())
}

/// Parse the `points` of a polyline or polygon.
pub(super) fn parse_points(data: &str) -> Result<Vec<Point>, String> {
    let mut numbers = Numbers::new(data);
    let mut points = Vec::new();
    loop {
        numbers.skip_separators();
        if numbers.peek().is_none() {
            return Ok(points);
        }
        points.push(Point::new(numbers.number()?, numbers.number()?));
    }
}

/// Numbers in SVG's compact syntax: `1.5.5` is two, `1-2` is two.
pub(super) struct Numbers<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Numbers<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            bytes: text.as_bytes(),
            at: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.bytes.get(self.at).map(|&byte| byte as char)
    }

    fn bump(&mut self) {
        self.at += 1;
    }

    pub fn skip_separators(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_ascii_whitespace() || c == ',') {
            self.bump();
        }
    }

    fn flag(&mut self) -> Result<bool, String> {
        self.skip_separators();
        match self.peek() {
            Some('0') => {
                self.bump();
                Ok(false)
            }
            Some('1') => {
                self.bump();
                Ok(true)
            }
            other => Err(format!("expected an arc flag, found {other:?}")),
        }
    }

    pub fn number(&mut self) -> Result<f32, String> {
        self.skip_separators();
        let start = self.at;
        if matches!(self.peek(), Some('+' | '-')) {
            self.bump();
        }
        let digits = |numbers: &mut Self| {
            let from = numbers.at;
            while matches!(numbers.peek(), Some(c) if c.is_ascii_digit()) {
                numbers.bump();
            }
            numbers.at > from
        };
        let whole = digits(self);
        let mut fraction = false;
        if self.peek() == Some('.') {
            self.bump();
            fraction = digits(self);
        }
        if !whole && !fraction {
            self.at = start;
            return Err(match self.peek() {
                Some(c) => format!("expected a number, found {c:?}"),
                None => "expected a number, found the end".to_string(),
            });
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            let mark = self.at;
            self.bump();
            if matches!(self.peek(), Some('+' | '-')) {
                self.bump();
            }
            if !digits(self) {
                self.at = mark;
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.at]).expect("ascii");
        text.parse::<f32>()
            .map_err(|err| format!("bad number {text:?}: {err}"))
    }

    pub fn is_done(&mut self) -> bool {
        self.skip_separators();
        self.peek().is_none()
    }
}

/// Contours filled as one polygon, by the nonzero rule.
pub(super) fn fill_polygon(contours: &[Vec<Point>]) -> Vec<PathVertex> {
    let mut contours = contours.iter().filter(|contour| contour.len() >= 3);
    let Some(first) = contours.next() else {
        return Vec::new();
    };
    let anchor = first[0];
    let mut vertices: Vec<PathVertex> = first.iter().map(vertex).collect();
    vertices.push(vertex(&anchor));
    for contour in contours {
        // Out to the contour and back the same way: the two edges cancel.
        vertices.extend(contour.iter().map(vertex));
        vertices.push(vertex(&contour[0]));
        vertices.push(vertex(&anchor));
    }
    vertices
}

fn vertex(point: &Point) -> PathVertex {
    PathVertex::new(point.x, point.y)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct StrokeShape {
    pub width: f32,
    pub cap: LineCap,
    pub join: LineJoin,
    pub miter_limit: f32,
}

/// The outline of `polyline` stroked as `stroke`, as contours wound alike.
/// Arcs are flattened to `tolerance`.
pub(super) fn stroke_outline(
    polyline: &[Point],
    closed: bool,
    stroke: StrokeShape,
    tolerance: f32,
    out: &mut Vec<Vec<Point>>,
) {
    let half = stroke.width * 0.5;
    if polyline.len() < 2 || half <= 0.0 {
        return;
    }
    let count = polyline.len();
    let segments = if closed { count } else { count - 1 };
    let direction = |index: usize| {
        let (from, to) = (polyline[index], polyline[(index + 1) % count]);
        let along = to.sub(from);
        along.scale(1.0 / along.length())
    };
    let normal = |dir: Point| Point::new(-dir.y, dir.x).scale(half);
    let mut push = |mut contour: Vec<Point>| {
        if signed_area(&contour) < 0.0 {
            contour.reverse();
        }
        out.push(contour);
    };
    for index in 0..segments {
        let (from, to) = (polyline[index], polyline[(index + 1) % count]);
        let offset = normal(direction(index));
        push(vec![
            from.add(offset),
            to.add(offset),
            to.sub(offset),
            from.sub(offset),
        ]);
    }
    // Joins at every vertex between two segments.
    let joins = if closed { 0..count } else { 1..count - 1 };
    for at in joins {
        let before = direction((at + count - 1) % count);
        let after = direction(at);
        let turn = before.cross(after);
        if turn.abs() < 1e-6 && before.dot(after) > 0.0 {
            continue;
        }
        let center = polyline[at];
        // The outer side, away from the turn.
        let side = if turn > 0.0 { -1.0 } else { 1.0 };
        let (n0, n1) = (normal(before).scale(side), normal(after).scale(side));
        let (o0, o1) = (center.add(n0), center.add(n1));
        let bevel = vec![center, o0, o1];
        match stroke.join {
            LineJoin::Bevel => push(bevel),
            LineJoin::Miter => {
                // Miter length over stroke width is 1 / sin(half the angle
                // between the segments).
                let cos_angle = (-before.dot(after)).clamp(-1.0, 1.0);
                let sin_half = ((1.0 - cos_angle) * 0.5).sqrt();
                let bisector = n0.add(n1);
                if sin_half > 1e-6
                    && 1.0 / sin_half <= stroke.miter_limit
                    && bisector.length() > 0.0
                {
                    let tip = center.add(bisector.scale(half / (bisector.length() * sin_half)));
                    push(vec![center, o0, tip, o1]);
                } else {
                    push(bevel);
                }
            }
            LineJoin::Round => {
                let mut wedge = vec![center];
                wedge.extend(arc(center, half, n0, n1, tolerance));
                push(wedge);
            }
        }
    }
    if !closed {
        let ends = [
            (polyline[0], direction(0).scale(-1.0)),
            (polyline[count - 1], direction(count - 2)),
        ];
        for (end, outward) in ends {
            let offset = normal(outward);
            match stroke.cap {
                LineCap::Butt => {}
                LineCap::Square => {
                    let tip = outward.scale(half);
                    push(vec![
                        end.add(offset),
                        end.add(offset).add(tip),
                        end.sub(offset).add(tip),
                        end.sub(offset),
                    ]);
                }
                LineCap::Round => {
                    // Two quarter turns, so the half turn goes outwards.
                    let tip = outward.scale(half);
                    let mut cap = vec![end];
                    cap.extend(arc(end, half, offset, tip, tolerance));
                    cap.extend(
                        arc(end, half, tip, offset.scale(-1.0), tolerance)
                            .into_iter()
                            .skip(1),
                    );
                    push(cap);
                }
            }
        }
    }
}

/// Points of the circle around `center` of radius `radius` from offset `from`
/// to offset `to`, the short way, or through the outside for a half turn.
fn arc(center: Point, radius: f32, from: Point, to: Point, tolerance: f32) -> Vec<Point> {
    let start = from.y.atan2(from.x);
    let mut sweep = to.y.atan2(to.x) - start;
    let pi = std::f32::consts::PI;
    if sweep > pi {
        sweep -= 2.0 * pi;
    } else if sweep < -pi {
        sweep += 2.0 * pi;
    }
    // A chord of angle a strays r (1 - cos(a / 2)) from the circle.
    let step = 2.0 * (1.0 - tolerance / radius).clamp(-1.0, 1.0).acos();
    let steps = (sweep.abs() / step.max(1e-3)).ceil().clamp(1.0, 64.0) as usize;
    (0..=steps)
        .map(|index| {
            let angle = start + sweep * index as f32 / steps as f32;
            let (sin, cos) = angle.sin_cos();
            Point::new(center.x + radius * cos, center.y + radius * sin)
        })
        .collect()
}

fn signed_area(contour: &[Point]) -> f32 {
    let mut area = 0.0;
    for (index, point) in contour.iter().enumerate() {
        area += point.cross(contour[(index + 1) % contour.len()]);
    }
    area * 0.5
}

#[cfg(test)]
pub(super) fn area(contour: &[Point]) -> f32 {
    signed_area(contour)
}
