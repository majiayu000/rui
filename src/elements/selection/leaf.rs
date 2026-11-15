//! What a selectable text tells its scope when painted, and the cluster
//! geometry hit-testing and highlighting shape from it.

use crate::core::SharedStr;
use crate::core::geometry::{Bounds, Point};
use crate::elements::text::TextAlign;
use crate::renderer::text::{
    BaseDirection, RichPieceText, RichWrapped, TextMeasureCache, TextRequest, WrappedText,
};
use std::cell::{OnceCell, RefCell};
use std::ops::Range;
use std::sync::Arc;

thread_local! {
    /// Shapes selected text. Fonts are the process-wide set the renderer and
    /// layout shape with; it is made on the first selection.
    static MEASURER: RefCell<Option<TextMeasureCache>> = const { RefCell::new(None) };
}

/// The font a run of a selectable text is shaped with.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LeafFont {
    pub size: f32,
    pub weight: u16,
    pub family: Option<Arc<str>>,
    pub line_height: f32,
}

/// How a selectable text laid out its lines.
#[derive(Debug, Clone)]
pub(crate) enum LeafLayout {
    /// One line, painted as one text primitive in the leaf's bounds.
    Line { font: LeafFont, align: TextAlign },
    /// Lines of one font, each painted in a row as tall as its line box.
    Wrapped {
        font: LeafFont,
        align: TextAlign,
        lines: Arc<WrappedText>,
    },
    /// Lines of pieces in the fonts of their spans.
    Rich {
        fonts: Vec<LeafFont>,
        align: TextAlign,
        lines: Arc<RichWrapped>,
    },
}

/// A text a selection scope can select, as painted this frame.
#[derive(Debug, Clone)]
pub(crate) struct SelectableLeaf {
    pub bounds: Bounds,
    /// The clip it was painted under, which its highlight keeps to.
    pub clip: Option<Bounds>,
    pub content: SharedStr,
    pub layout: LeafLayout,
    geometry: OnceCell<LeafGeometry>,
}

impl SelectableLeaf {
    pub fn new(
        bounds: Bounds,
        clip: Option<Bounds>,
        content: SharedStr,
        layout: LeafLayout,
    ) -> Self {
        Self {
            bounds,
            clip,
            content,
            layout,
            geometry: OnceCell::new(),
        }
    }

    /// Whether `other` is this text laid out at the same place, so the
    /// geometry shaped for one holds for the other.
    pub fn same_layout(&self, other: &Self) -> bool {
        let same_lines = match (&self.layout, &other.layout) {
            (LeafLayout::Line { font, align }, LeafLayout::Line { font: f, align: a }) => {
                font == f && align == a
            }
            (
                LeafLayout::Wrapped { font, align, lines },
                LeafLayout::Wrapped {
                    font: f,
                    align: a,
                    lines: l,
                },
            ) => font == f && align == a && Arc::ptr_eq(lines, l),
            (
                LeafLayout::Rich {
                    fonts,
                    align,
                    lines,
                },
                LeafLayout::Rich {
                    fonts: f,
                    align: a,
                    lines: l,
                },
            ) => fonts == f && align == a && Arc::ptr_eq(lines, l),
            _ => false,
        };
        same_lines && self.bounds == other.bounds && self.content == other.content
    }

    /// Take the geometry `previous` shaped, when it is laid out the same.
    pub fn adopt_geometry(&mut self, previous: &mut Self) {
        if self.geometry.get().is_none() && self.same_layout(previous) {
            self.geometry = std::mem::take(&mut previous.geometry);
        }
    }

    pub fn geometry(&self) -> &LeafGeometry {
        self.geometry.get_or_init(|| {
            MEASURER.with(|measurer| {
                let mut measurer = measurer.borrow_mut();
                LeafGeometry::shape(self, measurer.get_or_insert_with(TextMeasureCache::new))
            })
        })
    }

    /// Whether `point` is over this text's lines.
    pub fn contains(&self, point: Point) -> bool {
        self.bounds.contains(point) && self.clip.is_none_or(|clip| clip.contains(point))
    }

    /// How far `point` is from this text, up or down first, then across.
    pub fn distance(&self, point: Point) -> (f32, f32) {
        let gap = |value: f32, start: f32, end: f32| {
            if value < start {
                start - value
            } else if value > end {
                value - end
            } else {
                0.0
            }
        };
        let bounds = self.bounds;
        (
            gap(point.y, bounds.y(), bounds.y() + bounds.height()),
            gap(point.x, bounds.x(), bounds.x() + bounds.width()),
        )
    }
}

/// One shaped cluster, in window coordinates and bytes of the leaf's text.
#[derive(Debug, Clone, Copy)]
struct Cluster {
    bytes: (usize, usize),
    x: f32,
    width: f32,
    rtl: bool,
}

#[derive(Debug, Clone)]
struct Line {
    top: f32,
    height: f32,
    /// Where a point left of an empty line lands.
    start: usize,
    clusters: Vec<Cluster>,
}

/// Where each cluster of a leaf was painted.
#[derive(Debug, Clone, Default)]
pub(crate) struct LeafGeometry {
    lines: Vec<Line>,
}

/// A text primitive the leaf painted: its text, the bytes of the leaf it
/// shows, and the bounds and alignment it was painted at.
struct Run<'a> {
    line: usize,
    text: &'a str,
    bytes: Range<usize>,
    bounds: (f32, f32),
    align: TextAlign,
    font: &'a LeafFont,
    direction: BaseDirection,
}

impl LeafGeometry {
    fn shape(leaf: &SelectableLeaf, measurer: &mut TextMeasureCache) -> Self {
        let bounds = leaf.bounds;
        let content = leaf.content.as_str();
        let mut lines = Vec::new();
        let mut runs = Vec::new();
        match &leaf.layout {
            LeafLayout::Line { font, align } => {
                lines.push(empty_line(bounds.y(), bounds.height(), 0));
                runs.push(Run {
                    line: 0,
                    text: content,
                    bytes: 0..content.len(),
                    bounds: (bounds.x(), bounds.width()),
                    align: *align,
                    font,
                    direction: BaseDirection::Auto,
                });
            }
            LeafLayout::Wrapped {
                font,
                align,
                lines: wrapped,
            } => {
                let line_box = font.size * font.line_height;
                let mut end = 0;
                for (index, line) in wrapped.lines.iter().enumerate() {
                    let text = line.content.as_str();
                    let bytes = line
                        .content
                        .shared_range()
                        .or_else(|| cut_line_bytes(content, end, text))
                        .unwrap_or(end..end);
                    lines.push(empty_line(
                        bounds.y() + index as f32 * line_box,
                        line_box,
                        bytes.start,
                    ));
                    end = bytes.end;
                    runs.push(Run {
                        line: index,
                        text,
                        bytes,
                        bounds: (bounds.x(), bounds.width()),
                        align: *align,
                        font,
                        direction: line.direction,
                    });
                }
            }
            LeafLayout::Rich {
                fonts,
                align,
                lines: wrapped,
            } => {
                let mut y = bounds.y();
                let mut end = 0;
                for (index, line) in wrapped.lines.iter().enumerate() {
                    let offset = match align {
                        TextAlign::Left => 0.0,
                        TextAlign::Center => (bounds.width() - line.width) * 0.5,
                        TextAlign::Right => bounds.width() - line.width,
                    };
                    let start = line
                        .pieces
                        .iter()
                        .find_map(|piece| match &piece.text {
                            RichPieceText::Content(bytes) => Some(bytes.start),
                            RichPieceText::Ellipsis => None,
                        })
                        .unwrap_or(end);
                    lines.push(empty_line(y, line.height, start));
                    for piece in &line.pieces {
                        let RichPieceText::Content(bytes) = &piece.text else {
                            continue;
                        };
                        let Some(font) = fonts.get(piece.span) else {
                            continue;
                        };
                        end = bytes.end;
                        runs.push(Run {
                            line: index,
                            text: &content[bytes.clone()],
                            bytes: bytes.clone(),
                            bounds: (bounds.x() + offset + piece.x + piece.ink_x, piece.width),
                            align: TextAlign::Left,
                            font,
                            direction: BaseDirection::Auto,
                        });
                    }
                    y += line.height;
                }
            }
        }
        for run in runs {
            let request = TextRequest::new(
                run.text,
                run.font.size,
                run.font.weight,
                run.font.family.as_deref(),
                run.font.line_height,
            )
            .with_direction(run.direction);
            let plan = match measurer.shape_single_line(request) {
                Ok(plan) => plan,
                Err(err) => {
                    log::error!("selectable text could not be shaped: {err:?}");
                    continue;
                }
            };
            // Placed as the renderer places a text primitive: its ink box at
            // the bounds' edge its alignment picks, glyphs relative to the ink.
            let ink = plan.metrics().ink_bounds;
            let (x, width) = run.bounds;
            let x = match run.align {
                TextAlign::Left => x,
                TextAlign::Center => x + (width - ink.width()) * 0.5,
                TextAlign::Right => x + width - ink.width(),
            };
            let origin = x - ink.x();
            let end = run.bytes.end;
            let clusters = &mut lines[run.line].clusters;
            for cluster in plan.clusters() {
                let start = (run.bytes.start + cluster.byte_start).min(end);
                clusters.push(Cluster {
                    bytes: (start, (run.bytes.start + cluster.byte_end).min(end)),
                    x: origin + cluster.x_offset,
                    width: cluster.advance_width,
                    rtl: cluster.is_rtl(),
                });
            }
        }
        for line in &mut lines {
            line.clusters.sort_by(|a, b| a.x.total_cmp(&b.x));
        }
        Self { lines }
    }

    /// The byte offset closest to `point`: on the line it is level with, or
    /// the nearest line above or below.
    pub fn offset_at(&self, point: Point) -> usize {
        let Some(line) = self
            .lines
            .iter()
            .find(|line| point.y < line.top + line.height)
            .or(self.lines.last())
        else {
            return 0;
        };
        let (Some(first), Some(last)) = (line.clusters.first(), line.clusters.last()) else {
            return line.start;
        };
        // The edge of a cluster that is first in reading order leads it.
        let leading = |cluster: &Cluster| {
            if cluster.rtl {
                cluster.bytes.1
            } else {
                cluster.bytes.0
            }
        };
        let trailing = |cluster: &Cluster| {
            if cluster.rtl {
                cluster.bytes.0
            } else {
                cluster.bytes.1
            }
        };
        if point.x < first.x {
            return leading(first);
        }
        for cluster in &line.clusters {
            if point.x < cluster.x + cluster.width {
                return if point.x < cluster.x + cluster.width * 0.5 {
                    leading(cluster)
                } else {
                    trailing(cluster)
                };
            }
        }
        trailing(last)
    }

    /// Rectangles covering the clusters of `range`, one per run of adjacent
    /// clusters on a line.
    pub fn rects(&self, range: Range<usize>) -> Vec<Bounds> {
        let mut rects = Vec::new();
        if range.is_empty() {
            return rects;
        }
        for line in &self.lines {
            let mut open: Option<(f32, f32)> = None;
            for cluster in &line.clusters {
                let selected = cluster.bytes.0 < range.end && cluster.bytes.1 > range.start;
                match (selected, &mut open) {
                    (true, Some((_, end))) if cluster.x <= *end + 0.5 => {
                        *end = end.max(cluster.x + cluster.width);
                    }
                    (true, _) => {
                        if let Some((start, end)) = open.take() {
                            rects.push(Bounds::from_xywh(
                                start,
                                line.top,
                                end - start,
                                line.height,
                            ));
                        }
                        open = Some((cluster.x, cluster.x + cluster.width));
                    }
                    (false, _) => {
                        if let Some((start, end)) = open.take() {
                            rects.push(Bounds::from_xywh(
                                start,
                                line.top,
                                end - start,
                                line.height,
                            ));
                        }
                    }
                }
            }
            if let Some((start, end)) = open {
                rects.push(Bounds::from_xywh(start, line.top, end - start, line.height));
            }
        }
        rects
    }
}

fn empty_line(top: f32, height: f32, start: usize) -> Line {
    Line {
        top,
        height,
        start,
        clusters: Vec::new(),
    }
}

/// The bytes of `content` a line cut off with an ellipsis shows: its text
/// before the ellipsis, found from `from`.
fn cut_line_bytes(content: &str, from: usize, line: &str) -> Option<Range<usize>> {
    let shown = line.strip_suffix('\u{2026}').unwrap_or(line);
    let rest = content.get(from..)?;
    let start = from + rest.find(shown)?;
    Some(start..start + shown.len())
}
