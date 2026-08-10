use super::{
    INPUT_CARET_WIDTH, INPUT_GRAPHEME_WIDTH, INPUT_HORIZONTAL_PADDING,
    INPUT_MARKED_UNDERLINE_HEIGHT, Input, InputType, PASSWORD_MASK,
};
use crate::core::ElementId;
use crate::core::color::{Color, Rgba};
use crate::core::event::Cursor;
use crate::core::geometry::{Bounds, Edges, Point};
use crate::core::style::Corners;
use crate::core::text_editing::{TextEditLayout, TextEditPaintStyle, TextInputSnapshot, TextRange};
use crate::elements::element::{EventContext, PaintContext};
use crate::renderer::Primitive;
use unicode_segmentation::UnicodeSegmentation;

impl Input {
    pub(super) fn native_text_input_snapshot(
        &self,
        focused: ElementId,
    ) -> Option<TextInputSnapshot> {
        (self.id == Some(focused)).then(|| {
            TextInputSnapshot::new(
                self.state.value.clone(),
                self.state_selection(),
                self.state.composition_range,
            )
            .with_caret_bounds(self.caret_bounds)
        })
    }

    pub(super) fn display_offset_for_value_offset(&self, offset: usize) -> Option<usize> {
        if offset > self.state.value.len() || !self.state.value.is_char_boundary(offset) {
            return None;
        }

        if self.input_type == InputType::Password {
            Some(self.state.value[..offset].graphemes(true).count() * PASSWORD_MASK.len())
        } else {
            Some(offset)
        }
    }

    pub(super) fn display_range_for_value_range(&self, range: TextRange) -> Option<TextRange> {
        let start = self.display_offset_for_value_offset(range.start())?;
        let end = self.display_offset_for_value_offset(range.end())?;
        TextRange::new(start, end).ok()
    }

    pub(super) fn sync_focus_from_context(&mut self, cx: &mut EventContext<'_>) -> bool {
        let focused = cx.is_focused(self.id);
        if self.state.focused == focused {
            return false;
        }
        self.state.focused = focused;
        if focused {
            if let Some(handler) = &self.on_focus {
                handler();
            }
        } else if let Some(handler) = &self.on_blur {
            handler();
        }
        cx.request_redraw();
        true
    }

    /// Get display text (masked for password)
    pub(super) fn display_text(&self) -> String {
        if self.input_type == InputType::Password {
            PASSWORD_MASK.repeat(self.state.value.graphemes(true).count())
        } else {
            self.state.value.clone()
        }
    }

    pub(super) fn colors(&self) -> (Color, Color, Color) {
        if let Some(tokens) = self.paint_tokens {
            let text = if self.state.value.is_empty() {
                tokens.placeholder
            } else {
                tokens.text
            };
            return (tokens.background, text, tokens.border);
        }

        let bg = if self.state.disabled {
            Color::hex(0xf3f4f6)
        } else {
            Color::WHITE
        };
        let text = if self.state.value.is_empty() {
            Color::hex(0x9ca3af)
        } else if self.state.disabled {
            Color::hex(0x6b7280)
        } else {
            Color::hex(0x111827)
        };
        let border = if self.state.invalid {
            Color::hex(0xdc2626)
        } else if self.state.focused {
            Color::hex(0x6366f1)
        } else if self.state.hovered {
            Color::hex(0x9ca3af)
        } else {
            Color::hex(0xd1d5db)
        };
        (bg, text, border)
    }

    pub fn cursor(&self) -> Cursor {
        if self.state.disabled {
            Cursor::NotAllowed
        } else if self.state.read_only {
            Cursor::Default
        } else {
            Cursor::Text
        }
    }

    fn input_layout_text(&self) -> String {
        if self.state.value.is_empty() {
            String::new()
        } else {
            self.display_text()
        }
    }

    fn text_layout_for_bounds(&self, bounds: Bounds) -> TextEditLayout {
        TextEditLayout::new(
            self.input_layout_text(),
            INPUT_GRAPHEME_WIDTH,
            self.cursor_height(bounds),
        )
    }

    fn text_origin(&self, bounds: Bounds) -> Point {
        let cursor_height = self.cursor_height(bounds);
        Point::new(
            bounds.x() + INPUT_HORIZONTAL_PADDING,
            bounds.y() + (bounds.height() - cursor_height).max(0.0) / 2.0,
        )
    }

    pub(super) fn text_width(&self, bounds: Bounds) -> f32 {
        bounds.width() - (INPUT_HORIZONTAL_PADDING * 2.0)
    }

    pub(super) fn font_size(&self, bounds: Bounds) -> f32 {
        let requested = self
            .paint_tokens
            .map(|tokens| tokens.font_size)
            .unwrap_or(14.0);
        requested.min((bounds.height() - 4.0).max(1.0))
    }

    fn cursor_height(&self, bounds: Bounds) -> f32 {
        let requested = self
            .paint_tokens
            .map(|tokens| tokens.font_size * 1.4)
            .unwrap_or(20.0);
        requested.min((bounds.height() - 4.0).max(1.0))
    }

    pub(super) fn paint_selection_and_marked_text(&self, cx: &mut PaintContext, bounds: Bounds) {
        if !self.state.focused {
            return;
        }

        let layout = self.text_layout_for_bounds(bounds);
        let style = TextEditPaintStyle::new(
            INPUT_CARET_WIDTH,
            self.paint_tokens
                .map(|tokens| tokens.focus_ring)
                .unwrap_or_else(|| Color::hex(0x6366f1))
                .to_rgba(),
            self.paint_tokens
                .map(|tokens| tokens.focus_ring)
                .unwrap_or_else(|| Color::hex(0x6366f1))
                .with_alpha(0.22)
                .to_rgba(),
        );
        let paint_origin = self.text_origin(bounds);

        if let (Some(start), Some(end)) = (self.state.selection_start, self.state.selection_end)
            && let Ok(range) = TextRange::new(start, end)
            && let Some(display_range) = self.display_range_for_value_range(range)
        {
            match layout.selection_primitives(display_range, paint_origin, style) {
                Ok(primitives) => {
                    for primitive in primitives {
                        cx.paint(primitive);
                    }
                }
                Err(err) => log::error!("input selection paint failed: {err}"),
            }
        }

        if let Some(range) = self.state.composition_range
            && let Some(display_range) = self.display_range_for_value_range(range)
        {
            match layout.selection_rects(display_range) {
                Ok(rects) => {
                    for rect in rects {
                        cx.paint(Primitive::Quad {
                            bounds: Bounds::from_xywh(
                                paint_origin.x + rect.bounds.x(),
                                paint_origin.y + rect.bounds.y() + rect.bounds.height()
                                    - INPUT_MARKED_UNDERLINE_HEIGHT,
                                rect.bounds.width(),
                                INPUT_MARKED_UNDERLINE_HEIGHT,
                            ),
                            background: Color::hex(0x6366f1).to_rgba(),
                            border_color: Rgba::TRANSPARENT,
                            border_widths: Edges::ZERO,
                            corner_radii: Corners::ZERO,
                        });
                    }
                }
                Err(err) => log::error!("input marked text paint failed: {err}"),
            }
        }
    }

    pub(super) fn paint_cursor(&self, cx: &mut PaintContext, bounds: Bounds) -> Option<Bounds> {
        if !self.state.focused {
            return None;
        }

        let Some(cursor) = self.display_offset_for_value_offset(self.normalize_cursor_position())
        else {
            log::error!(
                "input cursor paint failed: cursor {} is not a valid display offset",
                self.state.cursor_position
            );
            return None;
        };

        let layout = self.text_layout_for_bounds(bounds);
        let style = TextEditPaintStyle::new(
            INPUT_CARET_WIDTH,
            self.paint_tokens
                .map(|tokens| tokens.focus_ring)
                .unwrap_or_else(|| Color::hex(0x6366f1))
                .to_rgba(),
            self.paint_tokens
                .map(|tokens| tokens.focus_ring)
                .unwrap_or_else(|| Color::hex(0x6366f1))
                .with_alpha(0.22)
                .to_rgba(),
        );
        match layout.caret_primitive(cursor, self.text_origin(bounds), style) {
            Ok(primitive) => {
                let caret_bounds = match &primitive {
                    Primitive::Quad { bounds, .. } => Some(*bounds),
                    _ => None,
                };
                cx.paint(primitive);
                caret_bounds
            }
            Err(err) => {
                log::error!("input caret paint failed: {err}");
                None
            }
        }
    }
}
