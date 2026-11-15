//! The vector scenes of the `paths` and `paths_icons` cases, the same on every side.
//! RUI's `benches/compare_frame.rs`, `benchmarks/egui_compare` and
//! `benchmarks/vello_compare` include this file and draw its shapes with their own
//! API. Everything is generated from a fixed seed, in logical pixels of a 1440x900
//! window; colors are straight-alpha sRGB.
//!
//! `paths`: 2 000 filled shapes (stars, pentagrams that need the nonzero rule,
//! cubic blobs and leaves), then 500 stroked polylines of 20 to 50 segments with
//! miter joins (limit 4) and butt caps, then 200 circles, overlapping, all
//! translucent. Each frame the fill [`MOVING`] turns about its center.
//!
//! `paths_icons`: 300 icons in a 20 by 15 grid, each one of 12 designs of 20 paths
//! in a 24 by 24 box drawn 48 px large. Each frame icon [`MOVING_ICON`] moves.

#![allow(dead_code)]

pub const FILLS: usize = 2_000;
pub const STROKES: usize = 500;
pub const CIRCLES: usize = 200;
/// The fill that turns each frame.
pub const MOVING: usize = 1_000;
pub const MITER_LIMIT: f32 = 4.0;

pub const ICON_DESIGNS: usize = 12;
pub const ICON_PATHS: usize = 20;
pub const ICON_GRID: (usize, usize) = (20, 15);
/// Side of an icon's view box, and of the icon drawn.
pub const ICON_BOX: f32 = 24.0;
pub const ICON_SIZE: f32 = 48.0;
pub const ICON_CELL: (f32, f32) = (72.0, 60.0);
pub const MOVING_ICON: usize = 157;

pub type Point = [f32; 2];

#[derive(Clone, Copy, Debug)]
pub enum Segment {
    Line(Point),
    Cubic(Point, Point, Point),
}

#[derive(Clone, Debug)]
pub struct Outline {
    pub start: Point,
    pub segments: Vec<Segment>,
    pub closed: bool,
}

#[derive(Clone, Debug)]
pub enum Shape {
    Fill {
        outline: Outline,
        color: [u8; 4],
    },
    Stroke {
        outline: Outline,
        width: f32,
        color: [u8; 4],
    },
    Circle {
        center: Point,
        radius: f32,
        color: [u8; 4],
    },
}

/// A small PCG, so every side draws the same numbers.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let x = (((self.0 >> 18) ^ self.0) >> 27) as u32;
        x.rotate_right((self.0 >> 59) as u32)
    }

    /// Uniform in `low..high`.
    fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * (self.next() as f32 / u32::MAX as f32)
    }

    fn below(&mut self, n: usize) -> usize {
        self.next() as usize % n
    }

    fn color(&mut self, alpha: (f32, f32)) -> [u8; 4] {
        let a = self.range(alpha.0, alpha.1);
        [
            self.range(40.0, 255.0) as u8,
            self.range(40.0, 255.0) as u8,
            self.range(40.0, 255.0) as u8,
            (a * 255.0) as u8,
        ]
    }
}

fn polar(center: Point, radius: f32, angle: f32) -> Point {
    [
        center[0] + radius * angle.cos(),
        center[1] + radius * angle.sin(),
    ]
}

fn polygon(points: &[Point]) -> Outline {
    Outline {
        start: points[0],
        segments: points[1..].iter().map(|&p| Segment::Line(p)).collect(),
        closed: true,
    }
}

fn star(center: Point, radius: f32, tips: usize, turn: f32) -> Outline {
    let points: Vec<Point> = (0..tips * 2)
        .map(|i| {
            let r = if i % 2 == 0 { radius } else { radius * 0.45 };
            polar(
                center,
                r,
                turn + i as f32 * std::f32::consts::PI / tips as f32,
            )
        })
        .collect();
    polygon(&points)
}

/// A five-pointed star drawn as one self-crossing line: its center winds twice, so
/// it is filled by the nonzero rule and empty by even-odd.
fn pentagram(center: Point, radius: f32, turn: f32) -> Outline {
    let points: Vec<Point> = (0..5)
        .map(|i| {
            polar(
                center,
                radius,
                turn + (i * 2) as f32 * std::f32::consts::TAU / 5.0,
            )
        })
        .collect();
    polygon(&points)
}

/// A closed curve through `points` as Catmull-Rom splines, one cubic each.
fn smooth(points: &[Point]) -> Outline {
    let n = points.len();
    let at = |i: usize| points[i % n];
    let segments = (0..n)
        .map(|i| {
            let (p0, p1, p2, p3) = (at(i + n - 1), at(i), at(i + 1), at(i + 2));
            Segment::Cubic(
                [p1[0] + (p2[0] - p0[0]) / 6.0, p1[1] + (p2[1] - p0[1]) / 6.0],
                [p2[0] - (p3[0] - p1[0]) / 6.0, p2[1] - (p3[1] - p1[1]) / 6.0],
                p2,
            )
        })
        .collect();
    Outline {
        start: points[0],
        segments,
        closed: true,
    }
}

fn blob(random: &mut Random, center: Point, radius: f32) -> Outline {
    let lobes = 5 + random.below(4);
    let turn = random.range(0.0, std::f32::consts::TAU);
    let points: Vec<Point> = (0..lobes)
        .map(|i| {
            let r = radius * random.range(0.6, 1.0);
            polar(
                center,
                r,
                turn + i as f32 * std::f32::consts::TAU / lobes as f32,
            )
        })
        .collect();
    smooth(&points)
}

/// Two cubics from tip to tip.
fn leaf(center: Point, radius: f32, turn: f32) -> Outline {
    let (a, b) = (
        polar(center, radius, turn),
        polar(center, radius, turn + std::f32::consts::PI),
    );
    let side = |sign: f32| {
        polar(
            center,
            radius * 1.1,
            turn + sign * std::f32::consts::FRAC_PI_2,
        )
    };
    let (left, right) = (side(1.0), side(-1.0));
    let mid = |p: Point, q: Point| [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0];
    Outline {
        start: a,
        segments: vec![
            Segment::Cubic(mid(a, left), left, b),
            Segment::Cubic(right, mid(a, right), a),
        ],
        closed: true,
    }
}

fn walk(random: &mut Random, viewport: Point) -> Outline {
    let mut point = [
        random.range(0.0, viewport[0]),
        random.range(0.0, viewport[1]),
    ];
    let mut heading = random.range(0.0, std::f32::consts::TAU);
    let start = point;
    let segments = (0..20 + random.below(31))
        .map(|_| {
            // Mostly gentle turns, some sharp enough to bevel at miter limit 4.
            heading += if random.below(6) == 0 {
                random.range(-2.6, 2.6)
            } else {
                random.range(-0.5, 0.5)
            };
            let step = random.range(6.0, 22.0);
            point = [
                (point[0] + step * heading.cos()).clamp(0.0, viewport[0]),
                (point[1] + step * heading.sin()).clamp(0.0, viewport[1]),
            ];
            Segment::Line(point)
        })
        .collect();
    Outline {
        start,
        segments,
        closed: false,
    }
}

/// The `paths` scene at frame 0.
pub fn paths() -> Vec<Shape> {
    let viewport = [1440.0, 900.0];
    let mut random = Random(0x5eed_0001);
    let mut shapes = Vec::with_capacity(FILLS + STROKES + CIRCLES);
    for index in 0..FILLS {
        let center = [
            random.range(0.0, viewport[0]),
            random.range(0.0, viewport[1]),
        ];
        let radius = random.range(8.0, 36.0);
        let turn = random.range(0.0, std::f32::consts::TAU);
        let outline = match index % 8 {
            0 => pentagram(center, radius, turn),
            1..=3 => star(center, radius, 5 + random.below(4), turn),
            4..=5 => blob(&mut random, center, radius),
            _ => leaf(center, radius, turn),
        };
        shapes.push(Shape::Fill {
            outline,
            color: random.color((0.35, 0.9)),
        });
    }
    for _ in 0..STROKES {
        let outline = walk(&mut random, viewport);
        shapes.push(Shape::Stroke {
            outline,
            width: random.range(0.75, 6.0),
            color: random.color((0.5, 1.0)),
        });
    }
    for _ in 0..CIRCLES {
        shapes.push(Shape::Circle {
            center: [
                random.range(0.0, viewport[0]),
                random.range(0.0, viewport[1]),
            ],
            radius: random.range(3.0, 30.0),
            color: random.color((0.3, 0.8)),
        });
    }
    shapes
}

impl Outline {
    /// The outline with every point mapped by `f`.
    pub fn map(&self, f: impl Fn(Point) -> Point) -> Outline {
        Outline {
            start: f(self.start),
            segments: self
                .segments
                .iter()
                .map(|segment| match *segment {
                    Segment::Line(p) => Segment::Line(f(p)),
                    Segment::Cubic(a, b, p) => Segment::Cubic(f(a), f(b), f(p)),
                })
                .collect(),
            closed: self.closed,
        }
    }

    /// Turned by `angle` radians about the center of its points' bounds.
    pub fn rotated(&self, angle: f32) -> Outline {
        let mut bounds = [self.start[0], self.start[1], self.start[0], self.start[1]];
        for segment in &self.segments {
            let p = match *segment {
                Segment::Line(p) | Segment::Cubic(_, _, p) => p,
            };
            bounds = [
                bounds[0].min(p[0]),
                bounds[1].min(p[1]),
                bounds[2].max(p[0]),
                bounds[3].max(p[1]),
            ];
        }
        let center = [(bounds[0] + bounds[2]) / 2.0, (bounds[1] + bounds[3]) / 2.0];
        let (sin, cos) = angle.sin_cos();
        self.map(|[x, y]| {
            let (dx, dy) = (x - center[0], y - center[1]);
            [
                center[0] + dx * cos - dy * sin,
                center[1] + dx * sin + dy * cos,
            ]
        })
    }

    /// Points no further than `tolerance` from the outline, as a polyline; a
    /// closed outline does not repeat its start.
    pub fn flatten(&self, tolerance: f32, points: &mut Vec<Point>) {
        points.clear();
        points.push(self.start);
        let mut from = self.start;
        for segment in &self.segments {
            match *segment {
                Segment::Line(to) => {
                    points.push(to);
                    from = to;
                }
                Segment::Cubic(a, b, to) => {
                    // A chord of step h strays at most h^2 / 8 times the largest
                    // second derivative, 6 times the largest second difference.
                    let second = |p: Point, q: Point, r: Point| {
                        let (x, y) = (p[0] - 2.0 * q[0] + r[0], p[1] - 2.0 * q[1] + r[1]);
                        (x * x + y * y).sqrt()
                    };
                    let largest = second(from, a, b).max(second(a, b, to));
                    let steps = (0.75 * largest / tolerance).sqrt().ceil().clamp(1.0, 256.0);
                    for step in 1..=steps as usize {
                        let t = step as f32 / steps;
                        let mt = 1.0 - t;
                        let (w0, w1, w2, w3) =
                            (mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t);
                        points.push([
                            w0 * from[0] + w1 * a[0] + w2 * b[0] + w3 * to[0],
                            w0 * from[1] + w1 * a[1] + w2 * b[1] + w3 * to[1],
                        ]);
                    }
                    from = to;
                }
            }
        }
        if self.closed && points.len() > 1 && points[0] == points[points.len() - 1] {
            points.pop();
        }
    }
}

/// The scene of frame `turn`: `shapes` with fill [`MOVING`] turned.
pub fn moving(shapes: &[Shape], turn: usize) -> Shape {
    match &shapes[MOVING] {
        Shape::Fill { outline, color } => Shape::Fill {
            outline: outline.rotated(turn as f32 * 0.05),
            color: *color,
        },
        _ => unreachable!("the moving shape is a fill"),
    }
}

/// The 12 icon designs, each 20 paths in a 24 by 24 box.
pub fn icon_designs() -> Vec<Vec<Shape>> {
    let mut random = Random(0x5eed_0002);
    (0..ICON_DESIGNS)
        .map(|_| {
            (0..ICON_PATHS)
                .map(|index| {
                    let center = [random.range(5.0, 19.0), random.range(5.0, 19.0)];
                    let color = random.color((0.6, 1.0));
                    match index % 4 {
                        0 => {
                            let radius = random.range(2.0, 5.0);
                            Shape::Fill {
                                outline: blob(&mut random, center, radius),
                                color,
                            }
                        }
                        1 => Shape::Fill {
                            outline: star(
                                center,
                                random.range(2.0, 5.0),
                                5,
                                random.range(0.0, 6.0),
                            ),
                            color,
                        },
                        2 => {
                            let points: Vec<Point> = (0..4)
                                .map(|_| [random.range(2.0, 22.0), random.range(2.0, 22.0)])
                                .collect();
                            Shape::Stroke {
                                outline: Outline {
                                    start: points[0],
                                    segments: points[1..]
                                        .iter()
                                        .map(|&p| Segment::Line(p))
                                        .collect(),
                                    closed: false,
                                },
                                width: 1.5,
                                color,
                            }
                        }
                        _ => Shape::Circle {
                            center,
                            radius: random.range(1.0, 3.0),
                            color,
                        },
                    }
                })
                .collect()
        })
        .collect()
}

/// Icon `index`'s design and top-left corner at frame `turn`.
pub fn icon_place(index: usize, turn: usize) -> (usize, Point) {
    let (column, row) = (index % ICON_GRID.0, index / ICON_GRID.0);
    let mut origin = [
        column as f32 * ICON_CELL.0 + (ICON_CELL.0 - ICON_SIZE) / 2.0,
        row as f32 * ICON_CELL.1 + (ICON_CELL.1 - ICON_SIZE) / 2.0,
    ];
    if index == MOVING_ICON {
        origin[0] += (turn % 8) as f32;
    }
    (index * 7 % ICON_DESIGNS, origin)
}

fn hex(color: [u8; 4]) -> String {
    format!("#{:02x}{:02x}{:02x}", color[0], color[1], color[2])
}

/// An icon design as SVG markup, for the sides that draw SVG.
pub fn icon_svg(design: &[Shape]) -> String {
    let mut markup =
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {ICON_BOX} {ICON_BOX}\">");
    let data = |outline: &Outline| {
        let mut d = format!("M{} {}", outline.start[0], outline.start[1]);
        for segment in &outline.segments {
            match *segment {
                Segment::Line(p) => d.push_str(&format!("L{} {}", p[0], p[1])),
                Segment::Cubic(a, b, p) => d.push_str(&format!(
                    "C{} {} {} {} {} {}",
                    a[0], a[1], b[0], b[1], p[0], p[1]
                )),
            }
        }
        if outline.closed {
            d.push('Z');
        }
        d
    };
    let opacity = |color: [u8; 4]| color[3] as f32 / 255.0;
    for shape in design {
        match shape {
            Shape::Fill { outline, color } => markup.push_str(&format!(
                "<path d=\"{}\" fill=\"{}\" fill-opacity=\"{}\"/>",
                data(outline),
                hex(*color),
                opacity(*color)
            )),
            Shape::Stroke { outline, width, color } => markup.push_str(&format!(
                "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-opacity=\"{}\" stroke-width=\"{width}\" stroke-linejoin=\"miter\" stroke-miterlimit=\"{MITER_LIMIT}\"/>",
                data(outline),
                hex(*color),
                opacity(*color)
            )),
            Shape::Circle { center, radius, color } => markup.push_str(&format!(
                "<circle cx=\"{}\" cy=\"{}\" r=\"{radius}\" fill=\"{}\" fill-opacity=\"{}\"/>",
                center[0],
                center[1],
                hex(*color),
                opacity(*color)
            )),
        }
    }
    markup.push_str("</svg>");
    markup
}
