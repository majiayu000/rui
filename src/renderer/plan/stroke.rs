//! A stroked path as polygon edges: a rectangle per segment and a miter join
//! at each vertex between two segments, all wound the same way, so where they
//! overlap the nonzero coverage counts the stroke once.

/// Miters longer than this many stroke widths are cut to bevels, as SVG's
/// default `stroke-miterlimit` cuts them.
pub(super) const MITER_LIMIT: f32 = 4.0;

/// Appends the edges of `points` stroked `half_width` either side, butt ended,
/// to `edges`. Coincident points are skipped.
pub(super) fn stroke_edges(
    points: impl IntoIterator<Item = [f32; 2]>,
    half_width: f32,
    edges: &mut Vec<[f32; 4]>,
) {
    let mut points = points.into_iter();
    let Some(mut from) = points.next() else {
        return;
    };
    // The previous segment's end and direction.
    let mut previous: Option<([f32; 2], [f32; 2])> = None;
    for to in points {
        let ([x0, y0], [x1, y1]) = (from, to);
        let length = ((x1 - x0) * (x1 - x0) + (y1 - y0) * (y1 - y0)).sqrt();
        if length <= f32::EPSILON {
            continue;
        }
        from = to;
        let direction = [(x1 - x0) / length, (y1 - y0) / length];
        if let Some((corner, before)) = previous {
            push_join(corner, before, direction, half_width, edges);
        }
        // The segment's rectangle: the normal turns with the segment, so every
        // rectangle is wound the same way, negatively.
        let (nx, ny) = (-direction[1] * half_width, direction[0] * half_width);
        push_polygon(
            &[
                [x0 + nx, y0 + ny],
                [x1 + nx, y1 + ny],
                [x1 - nx, y1 - ny],
                [x0 - nx, y0 - ny],
            ],
            edges,
        );
        previous = Some(([x1, y1], direction));
    }
}

/// The wedge the two rectangles meeting at `corner` leave open on the outside
/// of the turn: a miter, or a bevel past [`MITER_LIMIT`].
fn push_join(
    corner: [f32; 2],
    before: [f32; 2],
    after: [f32; 2],
    half_width: f32,
    edges: &mut Vec<[f32; 4]>,
) {
    let turn = before[0] * after[1] - before[1] * after[0];
    if turn.abs() < 1e-6 {
        // Straight on leaves no gap; a full reversal has no outside.
        return;
    }
    // The outer side is away from the turn.
    let side = if turn > 0.0 { -half_width } else { half_width };
    let n0 = [-before[1] * side, before[0] * side];
    let n1 = [-after[1] * side, after[0] * side];
    let o0 = [corner[0] + n0[0], corner[1] + n0[1]];
    let o1 = [corner[0] + n1[0], corner[1] + n1[1]];
    // The miter is 1 / sin(half the angle between the segments) widths long.
    let cos_angle = -(before[0] * after[0] + before[1] * after[1]);
    let sin_half = ((1.0 - cos_angle) * 0.5).max(0.0).sqrt();
    let bisector = [n0[0] + n1[0], n0[1] + n1[1]];
    let bisector_length = (bisector[0] * bisector[0] + bisector[1] * bisector[1]).sqrt();
    if sin_half > 1e-6 && 1.0 / sin_half <= MITER_LIMIT && bisector_length > 0.0 {
        let reach = half_width / (bisector_length * sin_half);
        let tip = [
            corner[0] + bisector[0] * reach,
            corner[1] + bisector[1] * reach,
        ];
        push_polygon(&[corner, o0, tip, o1], edges);
    } else {
        push_polygon(&[corner, o0, o1], edges);
    }
}

/// Appends the closed polygon's edges, wound negatively like the rectangles.
/// The polygon is convex, so its first three points tell its winding.
fn push_polygon(points: &[[f32; 2]], edges: &mut Vec<[f32; 4]>) {
    let ([ax, ay], [bx, by], [cx, cy]) = (points[0], points[1], points[2]);
    let reversed = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax) > 0.0;
    let count = points.len();
    for index in 0..count {
        let (a, b) = (points[index], points[(index + 1) % count]);
        let (a, b) = if reversed { (b, a) } else { (a, b) };
        if a != b {
            edges.push([a[0], a[1], b[0], b[1]]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The winding of `edges` around `point`, by a ray to the right.
    fn winding(edges: &[[f32; 4]], [px, py]: [f32; 2]) -> i32 {
        edges
            .iter()
            .filter(|&&[_, ay, _, by]| (ay <= py) != (by <= py))
            .map(|&[ax, ay, bx, by]| {
                let x = ax + (py - ay) / (by - ay) * (bx - ax);
                match (x > px, by > ay) {
                    (true, true) => 1,
                    (true, false) => -1,
                    _ => 0,
                }
            })
            .sum()
    }

    #[test]
    fn a_right_angle_is_mitered_and_wound_like_its_segments() {
        let mut edges = Vec::new();
        // Right, then down: the outside of the turn is up and right.
        stroke_edges([[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]], 2.0, &mut edges);
        let inside = winding(&edges, [5.0, 0.5]);
        assert_ne!(inside, 0);
        // The miter's corner, which butt-ended rectangles leave empty.
        assert_eq!(winding(&edges, [11.5, -1.5]).signum(), inside.signum());
        assert_eq!(winding(&edges, [12.5, -2.5]), 0, "past the miter tip");
    }

    #[test]
    fn a_sharp_turn_past_the_limit_is_beveled() {
        let mut edges = Vec::new();
        // Out and almost straight back: a miter would reach about 20 widths.
        stroke_edges([[0.0, 0.0], [20.0, 0.0], [0.0, 2.0]], 1.0, &mut edges);
        // Past both segments' ends, between them: only the bevel covers it.
        assert_ne!(winding(&edges, [20.05, 0.3]), 0);
        assert_eq!(winding(&edges, [23.0, 0.0]), 0, "no miter spike");
    }
}
