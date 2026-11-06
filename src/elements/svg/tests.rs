use super::geometry::{
    LineCap, LineJoin, Point, StrokeShape, Transform, area, fill_polygon, parse_path_data,
    stroke_outline,
};
use super::*;
use crate::elements::div;
use crate::testing::mount;

fn points(data: &str) -> Vec<(Vec<Point>, bool)> {
    parse_path_data(data)
        .expect("path data")
        .iter()
        .map(|subpath| (subpath.flatten(0.01), subpath.closed))
        .collect()
}

fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-3 && (a.y - b.y).abs() < 1e-3
}

#[test]
fn path_data_in_every_command_and_compact_form() {
    let lines = points("M10 10 L20 10 20 20z m5,5 h2 v2 H5 V1");
    assert_eq!(lines.len(), 2);
    assert!(lines[0].1, "closed by z");
    assert_eq!(
        lines[0].0,
        [
            Point::new(10.0, 10.0),
            Point::new(20.0, 10.0),
            Point::new(20.0, 20.0)
        ]
    );
    // Relative to where the closed subpath started.
    assert_eq!(
        lines[1].0,
        [
            Point::new(15.0, 15.0),
            Point::new(17.0, 15.0),
            Point::new(17.0, 17.0),
            Point::new(5.0, 17.0),
            Point::new(5.0, 1.0),
        ]
    );
    // Numbers run together: 1.5 .5, then -1 -2.
    let compact = points("M0,0l1.5.5-1-2");
    assert_eq!(
        compact[0].0,
        [
            Point::new(0.0, 0.0),
            Point::new(1.5, 0.5),
            Point::new(0.5, -1.5)
        ]
    );
    // Curves end where they say, smooth ones reflecting the last control.
    for data in [
        "M0 0 C0 10 10 10 10 0 S20 -10 20 0",
        "M0 0 Q5 10 10 0 T20 0",
        "M0 0 c0 10 10 10 10 0 s10 -10 10 0",
    ] {
        let line = &points(data)[0].0;
        assert!(near(*line.last().unwrap(), Point::new(20.0, 0.0)), "{data}");
        assert!(line.len() > 10, "{data} was not flattened");
    }
    let s = &points("M0 0 C0 10 10 10 10 0 S20 -10 20 0")[0].0;
    // The S half mirrors the C half through (10, 0).
    let lowest = s.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    let highest = s.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    assert!((lowest + highest).abs() < 1e-2, "{lowest} {highest}");
}

#[test]
fn arcs_follow_the_circle_the_flags_pick() {
    for (data, through) in [
        ("M0 0 A10 10 0 0 1 20 0", Point::new(10.0, -10.0)),
        ("M0 0 A10 10 0 0 0 20 0", Point::new(10.0, 10.0)),
        // Flags without separators, and radii too small, which scale up.
        ("M0 0a1 1 0 0020 0", Point::new(10.0, 10.0)),
    ] {
        let line = &points(data)[0].0;
        assert!(near(*line.last().unwrap(), Point::new(20.0, 0.0)), "{data}");
        for point in line {
            let radius = Point::new(point.x - 10.0, point.y).x.hypot(point.y);
            assert!(
                (radius - 10.0).abs() < 0.02,
                "{data}: {point:?} off the circle"
            );
        }
        // Over the top or under the bottom of the circle.
        assert!(
            line.iter().any(|point| (point.y - through.y).abs() < 0.05),
            "{data} does not pass {through:?}"
        );
    }
}

#[test]
fn compact_arcs_as_icon_sets_write_them() {
    let line = &points("M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8")[0].0;
    assert!(near(*line.last().unwrap(), Point::new(9.0, 21.0)));
    assert!(
        line.iter()
            .any(|point| near(*point, Point::new(10.0, 12.0)))
    );
}

#[test]
fn bad_path_data_is_an_error() {
    for data in ["10 10", "M10", "M0 0 X1 1", "M0 0 A1 1 0 2 0 1 1"] {
        assert!(parse_path_data(data).is_err(), "{data}");
    }
}

#[test]
fn contours_fill_as_one_polygon_whose_joins_add_nothing() {
    let square = |x: f32, size: f32, clockwise: bool| {
        let mut contour = vec![
            Point::new(x, 0.0),
            Point::new(x + size, 0.0),
            Point::new(x + size, size),
            Point::new(x, size),
        ];
        if !clockwise {
            contour.reverse();
        }
        contour
    };
    let as_points = |vertices: Vec<crate::renderer::PathVertex>| {
        vertices
            .iter()
            .map(|vertex| Point::new(vertex.position[0], vertex.position[1]))
            .collect::<Vec<_>>()
    };
    let one = as_points(fill_polygon(&[square(0.0, 4.0, true)]));
    assert_eq!(area(&one), 16.0);
    let apart = as_points(fill_polygon(&[
        square(0.0, 4.0, true),
        square(10.0, 2.0, true),
    ]));
    assert_eq!(area(&apart), 20.0);
    // A hole, wound the other way.
    let ring = as_points(fill_polygon(&[
        square(0.0, 4.0, true),
        square(1.0, 2.0, false),
    ]));
    assert_eq!(area(&ring), 12.0);
    assert!(fill_polygon(&[vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)]]).is_empty());
}

#[test]
fn strokes_cover_their_segments_joins_and_caps() {
    let stroke = |cap, join| StrokeShape {
        width: 2.0,
        cap,
        join,
        miter_limit: 4.0,
    };
    let outline = |points: &[Point], closed, shape| {
        let mut contours = Vec::new();
        stroke_outline(points, closed, shape, 0.001, &mut contours);
        assert!(
            contours.iter().all(|contour| area(contour) > 0.0),
            "wound alike"
        );
        contours.iter().map(|contour| area(contour)).sum::<f32>()
    };
    let line = [Point::new(0.0, 0.0), Point::new(10.0, 0.0)];
    assert_eq!(
        outline(&line, false, stroke(LineCap::Butt, LineJoin::Miter)),
        20.0
    );
    assert_eq!(
        outline(&line, false, stroke(LineCap::Square, LineJoin::Miter)),
        24.0
    );
    let round = outline(&line, false, stroke(LineCap::Round, LineJoin::Miter));
    assert!(
        (round - (20.0 + std::f32::consts::PI)).abs() < 1e-2,
        "{round}"
    );

    // A right angle: the miter fills the outer corner square, a bevel half of it,
    // a round join a quarter circle.
    let corner = [
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(10.0, 10.0),
    ];
    let butt = |join| outline(&corner, false, stroke(LineCap::Butt, join));
    assert!((butt(LineJoin::Miter) - 41.0).abs() < 1e-3);
    assert!((butt(LineJoin::Bevel) - 40.5).abs() < 1e-3);
    let quarter = std::f32::consts::PI / 4.0;
    assert!((butt(LineJoin::Round) - (40.0 + quarter)).abs() < 1e-2);
    // Too sharp for the miter limit: beveled.
    let spike = [
        Point::new(0.0, 0.0),
        Point::new(10.0, 0.0),
        Point::new(0.0, 0.5),
    ];
    let mut limited = stroke(LineCap::Butt, LineJoin::Miter);
    limited.miter_limit = 1.0;
    let mut beveled = limited;
    beveled.join = LineJoin::Bevel;
    assert_eq!(
        outline(&spike, false, limited),
        outline(&spike, false, beveled)
    );
}

#[test]
fn transforms_compose_as_svg_lists_them() {
    let rotate = Transform::translate(10.0, 10.0)
        .then(Transform::rotate(90.0))
        .then(Transform::translate(-10.0, -10.0));
    assert!(near(
        rotate.apply(Point::new(20.0, 10.0)),
        Point::new(10.0, 20.0)
    ));
    let document = parse::parse(
        r#"<svg viewBox="0 0 10 10"><g transform="translate(1 2) scale(2)"><path d="M1 1 L2 1 L2 2z" transform="rotate(90 1 1)"/></g></svg>"#,
    )
    .expect("parse");
    let start = document.shapes[0].subpaths[0].start;
    assert!(near(start, Point::new(3.0, 4.0)), "{start:?}");
    let second = match document.shapes[0].subpaths[0].segments[0] {
        geometry::Segment::Line(point) => point,
        other => panic!("{other:?}"),
    };
    // (2, 1) turns about (1, 1) to (1, 2), then scales and moves.
    assert!(near(second, Point::new(3.0, 6.0)), "{second:?}");
}

#[test]
fn colors_opacities_and_inheritance() {
    let document = parse::parse(
        r##"<svg viewBox="0 0 4 4" fill="#f00" stroke-width="3">
            <!-- a comment -->
            <title>icon</title>
            <g opacity="0.5" style="stroke: rgb(0, 0, 255); stroke-linecap: round">
                <rect width="4" height="4" fill-opacity="0.5"/>
                <circle cx="2" cy="2" r="1" fill="currentColor" color="#0f0"/>
                <line x1="0" y1="0" x2="4" y2="4" stroke="#abc"/>
            </g>
        </svg>"##,
    )
    .expect("parse");
    let [rect, circle, line] = document.shapes.as_slice() else {
        panic!("{:?}", document.shapes);
    };
    let red = ColorValue::Rgba(Rgba::from_u8(255, 0, 0, 255));
    assert_eq!(
        rect.fill.map(|paint| (paint.color, paint.opacity)),
        Some((red, 0.25))
    );
    let (stroke, shape) = rect.stroke.expect("inherited stroke");
    assert_eq!(
        stroke.color,
        ColorValue::Rgba(Rgba::from_u8(0, 0, 255, 255))
    );
    assert_eq!((shape.width, shape.cap), (3.0, LineCap::Round));
    assert_eq!(
        circle.fill.map(|paint| paint.color),
        Some(ColorValue::Rgba(Rgba::from_u8(0, 255, 0, 255)))
    );
    assert!(line.fill.is_none(), "a line has nothing to fill");
    assert_eq!(
        line.stroke.map(|(paint, _)| paint.color),
        Some(ColorValue::Rgba(Rgba::from_u8(0xaa, 0xbb, 0xcc, 255)))
    );
}

#[test]
fn what_is_not_supported_names_the_element() {
    for (markup, expected) in [
        (
            r#"<svg viewBox="0 0 1 1"><text>hi</text></svg>"#,
            "svg <text>: this element is not supported",
        ),
        (
            r#"<svg viewBox="0 0 1 1"><path id="a" d="M0 0" filter="url(#f)"/></svg>"#,
            "svg <path id=\"a\">: the attribute filter is not supported",
        ),
        (
            r#"<svg viewBox="0 0 1 1"><rect fill="url(#g)" width="1" height="1"/></svg>"#,
            "svg <rect>: fill: \"url(#g)\": gradients and patterns are not supported",
        ),
        (
            r#"<svg viewBox="0 0 1 1"><path d="M0 0L1 1z" fill-rule="evenodd"/></svg>"#,
            "svg <path>: fill-rule: \"evenodd\" is not supported, only nonzero",
        ),
        (
            r#"<svg viewBox="0 0 1 1"><path d="M0 0 Q1"/></svg>"#,
            "svg <path>: d: expected a number, found the end",
        ),
        (
            r#"<svg viewBox="0 0 1 1"><rect width="50%" height="1"/></svg>"#,
            "svg <rect>: width: \"50%\" is not a length in user units or px",
        ),
        (
            r#"<svg viewBox="0 0 1 1"><defs><linearGradient/></defs></svg>"#,
            "svg <defs>: this element is not supported",
        ),
        (
            r#"<svg><path d="M0 0"/></svg>"#,
            "svg <svg>: needs a viewBox, or a width and a height",
        ),
        (
            r#"<svg viewBox="0 0 1 1"><g>"#,
            "svg: line 1: <g> is never closed",
        ),
        (r#"<div/>"#, "svg <div>: the document is not an <svg>"),
    ] {
        let error = Svg::parse(markup).err().expect(markup);
        assert_eq!(error.to_string(), expected);
    }
}

const ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M5 12h14"/><path d="M12 5v14"/><circle cx="12" cy="12" r="10" fill="#000" stroke="none"/></svg>"##;

fn paths(primitives: &[Primitive]) -> Vec<(Rgba, Bounds)> {
    primitives
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Path {
                vertices, color, ..
            } => {
                let xs = vertices.iter().map(|vertex| vertex.position[0]);
                let ys = vertices.iter().map(|vertex| vertex.position[1]);
                let (x0, x1) = (
                    xs.clone().fold(f32::MAX, f32::min),
                    xs.fold(f32::MIN, f32::max),
                );
                let (y0, y1) = (
                    ys.clone().fold(f32::MAX, f32::min),
                    ys.fold(f32::MIN, f32::max),
                );
                Some((*color, Bounds::from_xywh(x0, y0, x1 - x0, y1 - y0)))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn an_icon_paints_paths_inside_its_bounds_in_its_tint() {
    let session = mount(Size::new(100.0, 100.0), |_| {
        div().p(10.0).child(
            svg(ICON)
                .size(Size::new(48.0, 48.0))
                .color(Color::hex(0x336699)),
        )
    })
    .expect("mount");
    let drawn = paths(session.primitives());
    assert_eq!(drawn.len(), 3, "{drawn:?}");
    let tint = Color::hex(0x336699).to_rgba();
    for (color, bounds) in &drawn {
        assert_eq!(*color, tint);
        assert!(
            bounds.x() >= 10.0
                && bounds.y() >= 10.0
                && bounds.x() + bounds.width() <= 58.0
                && bounds.y() + bounds.height() <= 58.0,
            "{bounds:?} outside the icon"
        );
    }
    // The plus is 14 of 24 units long, plus its round caps: 2 units at 2x.
    let (_, plus) = drawn[0];
    assert!((plus.width() - 32.0).abs() < 0.01, "{plus:?}");
}

#[test]
fn an_icon_is_parsed_once_and_shaped_once_per_size_and_tint() {
    let a = Svg::parse(ICON).expect("parse");
    let b = svg(ICON);
    assert!(a.same_props(&b));
    assert!(!a.same_props(&svg(ICON).color(Color::WHITE)));
    let parsed = b.parsed.as_ref().expect("parsed");
    let size = Size::new(16.0, 16.0);
    let first = shaped(parsed, size, None);
    assert!(Rc::ptr_eq(&first, &shaped(parsed, size, None)));
    assert!(!Rc::ptr_eq(
        &first,
        &shaped(parsed, Size::new(17.0, 16.0), None)
    ));
}

#[test]
fn an_unsupported_icon_shows_its_error() {
    let session = mount(Size::new(300.0, 40.0), |_| {
        div().child(svg(r#"<svg viewBox="0 0 1 1"><text/></svg>"#).size(Size::new(300.0, 20.0)))
    })
    .expect("mount");
    let shown: Vec<_> = session
        .primitives()
        .iter()
        .filter_map(|primitive| match primitive {
            Primitive::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(shown, ["svg <text>: this element is not supported"]);
}
