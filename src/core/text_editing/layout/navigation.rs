use super::{CaretAffinity, CaretGeometry, TextEditLayout, VisualCaret};
use crate::core::text_editing::{TextEditError, TextRange};
use crate::renderer::text::TextDirection;
use unicode_segmentation::UnicodeSegmentation;

impl TextEditLayout {
    pub(crate) fn caret_geometry_for_visual_caret(
        &self,
        caret: VisualCaret,
    ) -> Result<CaretGeometry, TextEditError> {
        self.ensure_layout_boundary(caret.offset)?;
        let line_index = self.line_index_for_offset(caret.offset);
        let caret = self.resolve_visual_caret(line_index, caret);
        let line = self.lines[line_index];
        let clamped = caret.offset.clamp(line.range.start(), line.range.end());
        let column = self.text[line.range.start()..clamped]
            .graphemes(true)
            .count();
        Ok(CaretGeometry {
            position: crate::core::geometry::Point::new(caret.x, line.origin.y),
            height: self.line_height,
            line_index,
            column,
        })
    }

    pub fn visual_offset_left(&self, offset: usize) -> Result<usize, TextEditError> {
        self.visual_caret_horizontal(self.visual_caret_for_offset(offset)?, false)
            .map(VisualCaret::offset)
    }

    pub fn visual_offset_right(&self, offset: usize) -> Result<usize, TextEditError> {
        self.visual_caret_horizontal(self.visual_caret_for_offset(offset)?, true)
            .map(VisualCaret::offset)
    }

    pub fn visual_offset_up(&self, offset: usize) -> Result<usize, TextEditError> {
        self.visual_caret_vertical(self.visual_caret_for_offset(offset)?, false)
            .map(VisualCaret::offset)
    }

    pub fn visual_offset_down(&self, offset: usize) -> Result<usize, TextEditError> {
        self.visual_caret_vertical(self.visual_caret_for_offset(offset)?, true)
            .map(VisualCaret::offset)
    }

    pub fn visual_line_start(&self, offset: usize) -> Result<usize, TextEditError> {
        self.visual_line_edge_caret(offset, false)
            .map(VisualCaret::offset)
    }

    pub fn visual_line_end(&self, offset: usize) -> Result<usize, TextEditError> {
        self.visual_line_edge_caret(offset, true)
            .map(VisualCaret::offset)
    }

    pub fn visual_selection_edge(
        &self,
        range: TextRange,
        right: bool,
    ) -> Result<usize, TextEditError> {
        self.visual_selection_caret(range, right)
            .map(VisualCaret::offset)
    }

    pub(crate) fn visual_caret_for_offset(
        &self,
        offset: usize,
    ) -> Result<VisualCaret, TextEditError> {
        self.ensure_layout_boundary(offset)?;
        let line_index = self.line_index_for_offset(offset);
        let expected_x = self.x_for_offset_on_line(line_index, offset);
        self.caret_stops_on_line(line_index)
            .into_iter()
            .filter(|stop| stop.offset == offset)
            .min_by(|left, right| {
                (left.x - expected_x)
                    .abs()
                    .total_cmp(&(right.x - expected_x).abs())
            })
            .ok_or(TextEditError::InvalidBoundary { index: offset })
    }

    pub(crate) fn visual_caret_for_point(
        &self,
        point: crate::core::geometry::Point,
    ) -> VisualCaret {
        let line_index = if self.line_height <= 0.0 {
            0
        } else {
            (point.y / self.line_height)
                .floor()
                .max(0.0)
                .min(self.lines.len().saturating_sub(1) as f32) as usize
        };
        self.closest_caret_on_line(line_index, point.x)
    }

    pub(crate) fn visual_caret_horizontal(
        &self,
        caret: VisualCaret,
        move_right: bool,
    ) -> Result<VisualCaret, TextEditError> {
        self.ensure_layout_boundary(caret.offset)?;
        let line_index = self.line_index_for_offset(caret.offset);
        let current = self.resolve_visual_caret(line_index, caret);
        let stops = self.caret_stops_on_line(line_index);
        let target = if move_right {
            stops
                .into_iter()
                .find(|stop| stop.x > current.x + f32::EPSILON)
                .or_else(|| {
                    (line_index + 1 < self.lines.len())
                        .then(|| self.visual_edge_caret(line_index + 1, false))
                })
        } else {
            stops
                .into_iter()
                .rev()
                .find(|stop| stop.x < current.x - f32::EPSILON)
                .or_else(|| (line_index > 0).then(|| self.visual_edge_caret(line_index - 1, true)))
        };
        Ok(target.unwrap_or(current))
    }

    pub(crate) fn visual_caret_vertical(
        &self,
        caret: VisualCaret,
        move_down: bool,
    ) -> Result<VisualCaret, TextEditError> {
        self.ensure_layout_boundary(caret.offset)?;
        let line_index = self.line_index_for_offset(caret.offset);
        let current = self.resolve_visual_caret(line_index, caret);
        let target_line = if move_down {
            (line_index + 1 < self.lines.len()).then_some(line_index + 1)
        } else {
            line_index.checked_sub(1)
        };
        Ok(target_line
            .map(|line| self.closest_caret_on_line(line, current.x))
            .unwrap_or(current))
    }

    pub(crate) fn visual_line_edge_caret(
        &self,
        offset: usize,
        right: bool,
    ) -> Result<VisualCaret, TextEditError> {
        self.ensure_layout_boundary(offset)?;
        Ok(self.visual_edge_caret(self.line_index_for_offset(offset), right))
    }

    pub(crate) fn visual_selection_caret(
        &self,
        range: TextRange,
        right: bool,
    ) -> Result<VisualCaret, TextEditError> {
        self.ensure_layout_range(range)?;
        let start_line = self.line_index_for_offset(range.start());
        let end_line = self.line_index_for_offset(range.end());
        if start_line != end_line {
            return self.visual_caret_for_offset(if right { range.end() } else { range.start() });
        }
        self.caret_stops_on_line(start_line)
            .into_iter()
            .filter(|stop| stop.offset == range.start() || stop.offset == range.end())
            .min_by(|left, right_stop| {
                let order = left.x.total_cmp(&right_stop.x);
                if right { order.reverse() } else { order }
            })
            .ok_or(TextEditError::InvalidRange {
                start: range.start(),
                end: range.end(),
            })
    }

    pub(super) fn closest_offset_on_line(&self, line_index: usize, x: f32) -> usize {
        self.closest_caret_on_line(line_index, x).offset
    }

    fn closest_caret_on_line(&self, line_index: usize, x: f32) -> VisualCaret {
        self.caret_stops_on_line(line_index)
            .into_iter()
            .min_by(|left, right| (left.x - x).abs().total_cmp(&(right.x - x).abs()))
            .unwrap_or_else(|| self.empty_line_caret(line_index))
    }

    fn visual_edge_caret(&self, line_index: usize, right: bool) -> VisualCaret {
        self.caret_stops_on_line(line_index)
            .into_iter()
            .min_by(|left, right_stop| {
                let order = left.x.total_cmp(&right_stop.x);
                if right { order.reverse() } else { order }
            })
            .unwrap_or_else(|| self.empty_line_caret(line_index))
    }

    fn resolve_visual_caret(&self, line_index: usize, caret: VisualCaret) -> VisualCaret {
        self.caret_stops_on_line(line_index)
            .into_iter()
            .filter(|stop| stop.offset == caret.offset)
            .min_by(|left, right| {
                let left_affinity = usize::from(left.affinity != caret.affinity);
                let right_affinity = usize::from(right.affinity != caret.affinity);
                left_affinity.cmp(&right_affinity).then_with(|| {
                    (left.x - caret.x)
                        .abs()
                        .total_cmp(&(right.x - caret.x).abs())
                })
            })
            .unwrap_or(caret)
    }

    fn caret_stops_on_line(&self, line_index: usize) -> Vec<VisualCaret> {
        let line = self.lines[line_index];
        let mut stops = Vec::new();
        for cluster in self
            .clusters
            .iter()
            .filter(|cluster| cluster.line_index == line_index)
        {
            let rtl = cluster.direction == TextDirection::RightToLeft;
            stops.push(VisualCaret {
                offset: cluster.byte_start,
                affinity: CaretAffinity::Downstream,
                x: line.origin.x + cluster.x_offset + if rtl { cluster.advance_width } else { 0.0 },
            });
            stops.push(VisualCaret {
                offset: cluster.byte_end,
                affinity: CaretAffinity::Upstream,
                x: line.origin.x + cluster.x_offset + if rtl { 0.0 } else { cluster.advance_width },
            });
        }
        if stops.is_empty() {
            stops.push(self.empty_line_caret(line_index));
        }
        stops.sort_by(|left, right| left.x.total_cmp(&right.x));
        stops.dedup_by(|right, left| {
            right.offset == left.offset && (right.x - left.x).abs() <= f32::EPSILON
        });
        stops
    }

    fn empty_line_caret(&self, line_index: usize) -> VisualCaret {
        VisualCaret {
            offset: self.lines[line_index].range.start(),
            affinity: CaretAffinity::Downstream,
            x: self.lines[line_index].origin.x,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::text::{TextMeasureCache, TextRequest};

    fn mixed_layout() -> (TextEditLayout, usize) {
        let text = "abc שלום";
        let mut cache = TextMeasureCache::new();
        let plan = cache
            .shape_single_line(TextRequest::new(text, 18.0, 400, None, 1.0))
            .unwrap_or_else(|err| panic!("mixed bidi shaping failed: {err:?}"));
        let boundary = plan
            .clusters()
            .iter()
            .find(|cluster| cluster.direction == TextDirection::RightToLeft)
            .map(|cluster| cluster.byte_start)
            .unwrap_or_else(|| panic!("mixed bidi plan had no RTL cluster"));
        let layout = TextEditLayout::from_shape_plan(text, &plan)
            .unwrap_or_else(|err| panic!("mixed bidi layout failed: {err}"));
        (layout, boundary)
    }

    #[test]
    fn mixed_bidi_navigation_retains_both_caret_affinities_at_run_boundaries() {
        let (layout, boundary) = mixed_layout();
        let stops = layout.caret_stops_on_line(0);
        let boundary_stops = stops
            .iter()
            .filter(|stop| stop.offset == boundary)
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(boundary_stops.len(), 2);
        assert!(boundary_stops[0].x < boundary_stops[1].x);
        assert_ne!(boundary_stops[0].affinity, boundary_stops[1].affinity);

        let mut current = layout.visual_edge_caret(0, false);
        let mut visited = vec![current];
        let mut visual_positions = stops.iter().map(|stop| stop.x).collect::<Vec<_>>();
        visual_positions.dedup_by(|right, left| (*right - *left).abs() <= f32::EPSILON);
        for _ in 1..visual_positions.len() {
            current = layout
                .visual_caret_horizontal(current, true)
                .unwrap_or_else(|err| panic!("right navigation failed: {err}"));
            visited.push(current);
        }
        assert!(
            visited.windows(2).all(|pair| pair[0].x < pair[1].x),
            "visited={visited:?} stops={stops:?}"
        );
        assert_eq!(
            visited.iter().map(|caret| caret.x).collect::<Vec<_>>(),
            visual_positions
        );

        for expected in boundary_stops {
            let hit = layout.visual_caret_for_point(crate::core::geometry::Point::new(
                expected.x,
                layout.line_height / 2.0,
            ));
            assert_eq!(hit.offset, boundary);
            assert!((hit.x - expected.x).abs() <= f32::EPSILON);
        }
    }
}
