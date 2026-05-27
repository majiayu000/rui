use super::error::TextEditError;
use super::types::TextRange;
use crate::core::color::Rgba;
use crate::core::geometry::{Bounds, Edges, Point, Size};
use crate::core::style::Corners;
use crate::renderer::Primitive;
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
    grapheme_width: f32,
    line_height: f32,
}

impl TextEditLayout {
    pub fn new(text: impl Into<String>, grapheme_width: f32, line_height: f32) -> Self {
        let text = text.into();
        let grapheme_width = grapheme_width.max(0.0);
        let line_height = line_height.max(0.0);
        let mut lines = Vec::new();
        let mut start = 0;
        let mut y = 0.0;

        for part in text.split_inclusive('\n') {
            let end = start + part.trim_end_matches('\n').len();
            lines.push(line_for(&text, start, end, y, grapheme_width, line_height));
            start += part.len();
            y += line_height;
        }

        if lines.is_empty() || text.ends_with('\n') {
            lines.push(line_for(
                &text,
                start,
                start,
                y,
                grapheme_width,
                line_height,
            ));
        }

        Self {
            text,
            lines,
            grapheme_width,
            line_height,
        }
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
                line.origin.x + column as f32 * self.grapheme_width,
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
        for line in &self.lines {
            let start = range.start().max(line.range.start());
            let end = range.end().min(line.range.end());
            if start >= end {
                continue;
            }

            let start_column = self.text[line.range.start()..start].graphemes(true).count();
            let selected = self.text[start..end].graphemes(true).count();
            rects.push(SelectionRect {
                bounds: Bounds::from_xywh(
                    line.origin.x + start_column as f32 * self.grapheme_width,
                    line.origin.y,
                    selected as f32 * self.grapheme_width,
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
        style: TextEditPaintStyle,
    ) -> Result<Primitive, TextEditError> {
        let caret = self.caret_for_offset(offset)?;
        Ok(Primitive::Quad {
            bounds: Bounds::from_xywh(
                caret.position.x,
                caret.position.y,
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
        style: TextEditPaintStyle,
    ) -> Result<Vec<Primitive>, TextEditError> {
        Ok(self
            .selection_rects(range)?
            .into_iter()
            .map(|rect| Primitive::Quad {
                bounds: rect.bounds,
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
        if offset <= self.text.len() && self.text.is_char_boundary(offset) {
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
