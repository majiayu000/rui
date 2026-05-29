use super::error::TextEditError;
use super::types::TextRange;
use crate::core::color::Rgba;
use crate::core::geometry::{Bounds, Edges, Point, Size};
use crate::core::style::Corners;
use crate::renderer::Primitive;
use crate::renderer::text::TextShapePlan;
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
        let text = text.into();
        let line_height = plan.metrics().size.height;
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
            });
        }

        Ok(Self {
            text,
            lines: vec![line],
            clusters,
            line_height,
        })
    }

    pub fn lines(&self) -> &[TextLine] {
        &self.lines
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
                if offset == cluster.byte_start {
                    Some(line.origin.x + cluster.x_offset)
                } else if offset == cluster.byte_end {
                    Some(line.origin.x + cluster.x_offset + cluster.advance_width)
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
        });
        x_offset += grapheme_width;
    }
}
