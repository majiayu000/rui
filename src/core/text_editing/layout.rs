use super::error::TextEditError;
use super::types::TextRange;
use crate::core::color::Rgba;
use crate::core::geometry::{Bounds, Edges, Point, Size};
use crate::core::style::Corners;
use crate::renderer::Primitive;
use crate::renderer::text::{TextDirection, TextShapePlan};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextLine {
    range: TextRange,
    origin: Point,
    size: Size,
}

impl TextLine {
    pub fn text_range(&self) -> TextRange {
        self.range
    }

    pub fn origin(&self) -> Point {
        self.origin
    }

    pub fn measured_size(&self) -> Size {
        self.size
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaretGeometry {
    pub position: Point,
    pub height: f32,
    pub line_index: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectionRect {
    pub bounds: Bounds,
    pub range: TextRange,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct EditClusterGeometry {
    byte_start: usize,
    byte_end: usize,
    line_index: usize,
    x_offset: f32,
    advance_width: f32,
    direction: TextDirection,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextEditPaintStyle {
    pub caret_width: f32,
    pub caret_color: Rgba,
    pub selection_color: Rgba,
}

impl TextEditPaintStyle {
    pub const fn new(caret_width: f32, caret_color: Rgba, selection_color: Rgba) -> Self {
        Self {
            caret_width,
            caret_color,
            selection_color,
        }
    }
}

impl Default for TextEditPaintStyle {
    fn default() -> Self {
        Self {
            caret_width: 1.5,
            caret_color: Rgba::new(0.388, 0.4, 0.945, 1.0),
            selection_color: Rgba::new(0.388, 0.4, 0.945, 0.28),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextEditLayout {
    text: String,
    lines: Vec<TextLine>,
    clusters: Vec<EditClusterGeometry>,
    line_height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextInputGeometry {
    layout: TextEditLayout,
    origin: Point,
}

impl TextInputGeometry {
    pub fn new(layout: TextEditLayout, origin: Point) -> Self {
        Self { layout, origin }
    }

    pub fn first_bounds_for_range(
        &self,
        range: TextRange,
    ) -> Result<Option<(TextRange, Bounds)>, TextEditError> {
        if range.is_empty() {
            let caret = self.layout.caret_for_offset(range.start())?;
            return Ok(Some((
                range,
                Bounds::from_xywh(
                    self.origin.x + caret.position.x,
                    self.origin.y + caret.position.y,
                    0.0,
                    caret.height,
                ),
            )));
        }
        Ok(self.layout.selection_rects(range)?.first().map(|rect| {
            (
                rect.range,
                Bounds::from_xywh(
                    self.origin.x + rect.bounds.x(),
                    self.origin.y + rect.bounds.y(),
                    rect.bounds.width(),
                    rect.bounds.height(),
                ),
            )
        }))
    }

    pub fn offset_for_point(&self, point: Point) -> usize {
        self.layout
            .offset_for_point(Point::new(point.x - self.origin.x, point.y - self.origin.y))
    }

    pub fn text_offset_for_point(&self, point: Point) -> Option<usize> {
        self.layout
            .text_offset_for_point(Point::new(point.x - self.origin.x, point.y - self.origin.y))
    }
}

impl TextEditLayout {
    pub fn new(text: impl Into<String>, grapheme_width: f32, line_height: f32) -> Self {
        let text = text.into();
        let grapheme_width = grapheme_width.max(0.0);
        let line_height = line_height.max(0.0);
        let mut lines = Vec::new();
        let mut clusters = Vec::new();
        let mut start = 0;
        let mut y = 0.0;

        for part in text.split_inclusive('\n') {
            let end = start + part.trim_end_matches('\n').len();
            let line = line_for(&text, start, end, y, grapheme_width, line_height);
            push_fixed_clusters(
                &text,
                line.text_range(),
                lines.len(),
                grapheme_width,
                &mut clusters,
            );
            lines.push(line);
            start += part.len();
            y += line_height;
        }

        if lines.is_empty() || text.ends_with('\n') {
            let line = line_for(&text, start, start, y, grapheme_width, line_height);
            lines.push(line);
        }

        Self {
            text,
            lines,
            clusters,
            line_height,
        }
    }

    pub fn from_shape_plan(
        text: impl Into<String>,
        plan: &TextShapePlan,
    ) -> Result<Self, TextEditError> {
        let line_height = plan.metrics().size.height;
        Self::from_shape_plan_with_line_height(text, plan, line_height)
    }

    pub fn from_shape_plan_with_line_height(
        text: impl Into<String>,
        plan: &TextShapePlan,
        line_height: f32,
    ) -> Result<Self, TextEditError> {
        let text = text.into();
        let line_height = line_height.max(0.0);
        let line = TextLine {
            range: TextRange::ordered(0, text.len()),
            origin: Point::new(0.0, 0.0),
            size: Size::new(plan.metrics().size.width, line_height),
        };
        let mut clusters = Vec::new();
        for cluster in plan.clusters() {
            if cluster.byte_end > text.len()
                || !text.is_char_boundary(cluster.byte_start)
                || !text.is_char_boundary(cluster.byte_end)
                || text[cluster.byte_start..cluster.byte_end] != cluster.text
            {
                return Err(TextEditError::InvalidRange {
                    start: cluster.byte_start,
                    end: cluster.byte_end,
                });
            }
            clusters.push(EditClusterGeometry {
                byte_start: cluster.byte_start,
                byte_end: cluster.byte_end,
                line_index: 0,
                x_offset: cluster.x_offset,
                advance_width: cluster.advance_width,
                direction: cluster.direction,
            });
        }

        Ok(Self {
            text,
            lines: vec![line],
            clusters,
            line_height,
        })
    }

    pub fn from_line_shape_plans(
        text: impl Into<String>,
        plans: &[TextShapePlan],
        line_height: f32,
    ) -> Result<Self, TextEditError> {
        let text = text.into();
        let line_height = line_height.max(0.0);
        let ranges = line_ranges(&text);
        if ranges.len() != plans.len() {
            return Err(TextEditError::InvalidRange {
                start: plans.len(),
                end: ranges.len(),
            });
        }

        let mut lines = Vec::with_capacity(ranges.len());
        let mut clusters = Vec::new();
        for (line_index, ((start, end), plan)) in ranges.into_iter().zip(plans).enumerate() {
            let line_text = &text[start..end];
            lines.push(TextLine {
                range: TextRange::ordered(start, end),
                origin: Point::new(0.0, line_index as f32 * line_height),
                size: Size::new(plan.metrics().size.width, line_height),
            });
            for cluster in plan.clusters() {
                if cluster.byte_end > line_text.len()
                    || !line_text.is_char_boundary(cluster.byte_start)
                    || !line_text.is_char_boundary(cluster.byte_end)
                    || line_text[cluster.byte_start..cluster.byte_end] != cluster.text
                {
                    return Err(TextEditError::InvalidRange {
                        start: start + cluster.byte_start,
                        end: start + cluster.byte_end,
                    });
                }
                clusters.push(EditClusterGeometry {
                    byte_start: start + cluster.byte_start,
                    byte_end: start + cluster.byte_end,
                    line_index,
                    x_offset: cluster.x_offset,
                    advance_width: cluster.advance_width,
                    direction: cluster.direction,
                });
            }
        }

        Ok(Self {
            text,
            lines,
            clusters,
            line_height,
        })
    }

    pub fn lines(&self) -> &[TextLine] {
        &self.lines
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn caret_for_offset(&self, offset: usize) -> Result<CaretGeometry, TextEditError> {
        self.ensure_layout_boundary(offset)?;
        let line_index = self.line_index_for_offset(offset);
        let line = self.lines[line_index];
        let clamped = offset.clamp(line.range.start(), line.range.end());
        let column = self.text[line.range.start()..clamped]
            .graphemes(true)
            .count();
        Ok(CaretGeometry {
            position: Point::new(
                self.x_for_offset_on_line(line_index, clamped),
                line.origin.y,
            ),
            height: self.line_height,
            line_index,
            column,
        })
    }

    pub fn selection_rects(&self, range: TextRange) -> Result<Vec<SelectionRect>, TextEditError> {
        self.ensure_layout_range(range)?;
        if range.is_empty() {
            return Ok(Vec::new());
        }

        let mut rects = Vec::new();
        for (line_index, line) in self.lines.iter().enumerate() {
            let start = range.start().max(line.range.start());
            let end = range.end().min(line.range.end());
            if start >= end {
                continue;
            }

            let (start_x, end_x) = self
                .visual_bounds_for_range_on_line(line_index, start, end)
                .unwrap_or_else(|| {
                    (
                        self.x_for_offset_on_line(line_index, start),
                        self.x_for_offset_on_line(line_index, end),
                    )
                });
            rects.push(SelectionRect {
                bounds: Bounds::from_xywh(
                    start_x,
                    line.origin.y,
                    (end_x - start_x).max(0.0),
                    self.line_height,
                ),
                range: TextRange::ordered(start, end),
            });
        }
        Ok(rects)
    }

    pub fn offset_for_point(&self, point: Point) -> usize {
        let line_index = if self.line_height <= 0.0 {
            0
        } else {
            (point.y / self.line_height)
                .floor()
                .max(0.0)
                .min(self.lines.len().saturating_sub(1) as f32) as usize
        };
        self.closest_offset_on_line(line_index, point.x)
    }

    pub fn text_offset_for_point(&self, point: Point) -> Option<usize> {
        let line_index = self.lines.iter().position(|line| {
            point.y >= line.origin.y && point.y < line.origin.y + self.line_height
        })?;
        self.clusters
            .iter()
            .filter(|cluster| cluster.line_index == line_index)
            .find(|cluster| {
                let start = cluster
                    .x_offset
                    .min(cluster.x_offset + cluster.advance_width);
                let end = cluster
                    .x_offset
                    .max(cluster.x_offset + cluster.advance_width);
                point.x >= start && point.x < end
            })
            .map(|cluster| cluster.byte_start)
    }

    pub fn visual_offset_left(&self, offset: usize) -> Result<usize, TextEditError> {
        self.visual_offset_horizontal(offset, false)
    }

    pub fn visual_offset_right(&self, offset: usize) -> Result<usize, TextEditError> {
        self.visual_offset_horizontal(offset, true)
    }

    pub fn visual_offset_up(&self, offset: usize) -> Result<usize, TextEditError> {
        self.visual_offset_vertical(offset, false)
    }

    pub fn visual_offset_down(&self, offset: usize) -> Result<usize, TextEditError> {
        self.visual_offset_vertical(offset, true)
    }

    pub fn visual_line_start(&self, offset: usize) -> Result<usize, TextEditError> {
        self.visual_line_edge(offset, false)
    }

    pub fn visual_line_end(&self, offset: usize) -> Result<usize, TextEditError> {
        self.visual_line_edge(offset, true)
    }

    pub fn visual_selection_edge(
        &self,
        range: TextRange,
        right: bool,
    ) -> Result<usize, TextEditError> {
        self.ensure_layout_range(range)?;
        let start_line = self.line_index_for_offset(range.start());
        let end_line = self.line_index_for_offset(range.end());
        if start_line != end_line {
            return Ok(if right { range.end() } else { range.start() });
        }
        let start_x = self.x_for_offset_on_line(start_line, range.start());
        let end_x = self.x_for_offset_on_line(end_line, range.end());
        Ok(if (start_x <= end_x) == right {
            range.end()
        } else {
            range.start()
        })
    }

    pub fn caret_primitive(
        &self,
        offset: usize,
        paint_origin: impl Into<Point>,
        style: TextEditPaintStyle,
    ) -> Result<Primitive, TextEditError> {
        let paint_origin = paint_origin.into();
        let caret = self.caret_for_offset(offset)?;
        Ok(Primitive::Quad {
            bounds: Bounds::from_xywh(
                paint_origin.x + caret.position.x,
                paint_origin.y + caret.position.y,
                style.caret_width,
                caret.height,
            ),
            background: style.caret_color,
            border_color: Rgba::TRANSPARENT,
            border_widths: Edges::ZERO,
            corner_radii: Corners::ZERO,
        })
    }

    pub fn selection_primitives(
        &self,
        range: TextRange,
        paint_origin: impl Into<Point>,
        style: TextEditPaintStyle,
    ) -> Result<Vec<Primitive>, TextEditError> {
        let paint_origin = paint_origin.into();
        Ok(self
            .selection_rects(range)?
            .into_iter()
            .map(|rect| Primitive::Quad {
                bounds: Bounds::from_xywh(
                    paint_origin.x + rect.bounds.x(),
                    paint_origin.y + rect.bounds.y(),
                    rect.bounds.width(),
                    rect.bounds.height(),
                ),
                background: style.selection_color,
                border_color: Rgba::TRANSPARENT,
                border_widths: Edges::ZERO,
                corner_radii: Corners::ZERO,
            })
            .collect())
    }

    fn ensure_layout_range(&self, range: TextRange) -> Result<(), TextEditError> {
        if range.end() > self.text.len() {
            return Err(TextEditError::InvalidRange {
                start: range.start(),
                end: range.end(),
            });
        }
        self.ensure_layout_boundary(range.start())?;
        self.ensure_layout_boundary(range.end())
    }

    fn visual_offset_horizontal(
        &self,
        offset: usize,
        move_right: bool,
    ) -> Result<usize, TextEditError> {
        self.ensure_layout_boundary(offset)?;
        let line_index = self.line_index_for_offset(offset);
        let mut stops = self.caret_stops_on_line(line_index);
        stops.sort_by(|left, right| left.1.total_cmp(&right.1));
        let current_x = self.x_for_offset_on_line(line_index, offset);
        let target = if move_right {
            stops
                .iter()
                .find(|(_, x)| *x > current_x + f32::EPSILON)
                .map(|(offset, _)| *offset)
                .or_else(|| {
                    (line_index + 1 < self.lines.len())
                        .then(|| self.visual_edge_offset(line_index + 1, false))
                })
        } else {
            stops
                .iter()
                .rev()
                .find(|(_, x)| *x < current_x - f32::EPSILON)
                .map(|(offset, _)| *offset)
                .or_else(|| (line_index > 0).then(|| self.visual_edge_offset(line_index - 1, true)))
        };
        Ok(target.unwrap_or(offset))
    }

    fn visual_offset_vertical(
        &self,
        offset: usize,
        move_down: bool,
    ) -> Result<usize, TextEditError> {
        self.ensure_layout_boundary(offset)?;
        let line_index = self.line_index_for_offset(offset);
        let target_line = if move_down {
            (line_index + 1 < self.lines.len()).then_some(line_index + 1)
        } else {
            line_index.checked_sub(1)
        };
        Ok(target_line
            .map(|line| {
                self.closest_offset_on_line(line, self.x_for_offset_on_line(line_index, offset))
            })
            .unwrap_or(offset))
    }

    fn visual_line_edge(&self, offset: usize, right: bool) -> Result<usize, TextEditError> {
        self.ensure_layout_boundary(offset)?;
        Ok(self.visual_edge_offset(self.line_index_for_offset(offset), right))
    }

    fn visual_edge_offset(&self, line_index: usize, right: bool) -> usize {
        self.caret_stops_on_line(line_index)
            .into_iter()
            .min_by(|left, right_stop| {
                let order = left.1.total_cmp(&right_stop.1);
                if right { order.reverse() } else { order }
            })
            .map(|(offset, _)| offset)
            .unwrap_or(self.lines[line_index].range.start())
    }

    fn closest_offset_on_line(&self, line_index: usize, x: f32) -> usize {
        self.caret_stops_on_line(line_index)
            .into_iter()
            .min_by(|left, right| (left.1 - x).abs().total_cmp(&(right.1 - x).abs()))
            .map(|(offset, _)| offset)
            .unwrap_or(self.lines[line_index].range.start())
    }

    fn caret_stops_on_line(&self, line_index: usize) -> Vec<(usize, f32)> {
        let line = self.lines[line_index];
        let mut offsets = vec![line.range.start(), line.range.end()];
        for cluster in self
            .clusters
            .iter()
            .filter(|cluster| cluster.line_index == line_index)
        {
            offsets.push(cluster.byte_start);
            offsets.push(cluster.byte_end);
        }
        offsets.sort_unstable();
        offsets.dedup();
        offsets
            .into_iter()
            .map(|offset| (offset, self.x_for_offset_on_line(line_index, offset)))
            .collect()
    }

    fn ensure_layout_boundary(&self, offset: usize) -> Result<(), TextEditError> {
        if offset > self.text.len() || !self.text.is_char_boundary(offset) {
            return Err(TextEditError::InvalidBoundary { index: offset });
        }

        if offset == self.text.len()
            || self
                .lines
                .iter()
                .any(|line| offset == line.range.start() || offset == line.range.end())
            || self
                .clusters
                .iter()
                .any(|cluster| offset == cluster.byte_start || offset == cluster.byte_end)
        {
            Ok(())
        } else {
            Err(TextEditError::InvalidBoundary { index: offset })
        }
    }

    fn line_index_for_offset(&self, offset: usize) -> usize {
        self.lines
            .iter()
            .position(|line| offset <= line.range.end())
            .unwrap_or_else(|| self.lines.len().saturating_sub(1))
    }

    fn x_for_offset_on_line(&self, line_index: usize, offset: usize) -> f32 {
        let line = self.lines[line_index];
        if let Some(x) = self
            .clusters
            .iter()
            .filter(|cluster| cluster.line_index == line_index)
            .find_map(|cluster| {
                let rtl = cluster.direction == TextDirection::RightToLeft;
                if offset == cluster.byte_start {
                    Some(
                        line.origin.x
                            + cluster.x_offset
                            + if rtl { cluster.advance_width } else { 0.0 },
                    )
                } else if offset == cluster.byte_end {
                    Some(
                        line.origin.x
                            + cluster.x_offset
                            + if rtl { 0.0 } else { cluster.advance_width },
                    )
                } else {
                    None
                }
            })
        {
            return x;
        }

        if offset <= line.range.start() {
            return line.origin.x;
        }
        if offset >= line.range.end() {
            return line.origin.x + line.size.width;
        }

        line.origin.x + line.size.width
    }

    fn visual_bounds_for_range_on_line(
        &self,
        line_index: usize,
        start: usize,
        end: usize,
    ) -> Option<(f32, f32)> {
        let line = self.lines[line_index];
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;

        for cluster in self
            .clusters
            .iter()
            .filter(|cluster| cluster.line_index == line_index)
            .filter(|cluster| cluster.byte_end > start && cluster.byte_start < end)
        {
            min_x = min_x.min(line.origin.x + cluster.x_offset);
            max_x = max_x.max(line.origin.x + cluster.x_offset + cluster.advance_width);
        }

        min_x.is_finite().then_some((min_x, max_x))
    }
}

fn line_for(
    text: &str,
    start: usize,
    end: usize,
    y: f32,
    grapheme_width: f32,
    line_height: f32,
) -> TextLine {
    let width = text[start..end].graphemes(true).count() as f32 * grapheme_width;
    TextLine {
        range: TextRange::ordered(start, end),
        origin: Point::new(0.0, y),
        size: Size::new(width, line_height),
    }
}

fn line_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for part in text.split_inclusive('\n') {
        let end = start + part.trim_end_matches('\n').len();
        ranges.push((start, end));
        start += part.len();
    }
    if ranges.is_empty() || text.ends_with('\n') {
        ranges.push((start, start));
    }
    ranges
}

fn push_fixed_clusters(
    text: &str,
    range: TextRange,
    line_index: usize,
    grapheme_width: f32,
    clusters: &mut Vec<EditClusterGeometry>,
) {
    let mut x_offset = 0.0;
    for (relative_start, grapheme) in text[range.start()..range.end()].grapheme_indices(true) {
        let byte_start = range.start() + relative_start;
        clusters.push(EditClusterGeometry {
            byte_start,
            byte_end: byte_start + grapheme.len(),
            line_index,
            x_offset,
            advance_width: grapheme_width,
            direction: TextDirection::LeftToRight,
        });
        x_offset += grapheme_width;
    }
}
