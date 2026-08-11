use super::{
    TEXT_AREA_CARET_WIDTH, TEXT_AREA_HORIZONTAL_PADDING, TEXT_AREA_LINE_HEIGHT,
    TEXT_AREA_MARKED_UNDERLINE_HEIGHT, TEXT_AREA_VERTICAL_PADDING, TextArea,
};
use crate::core::color::{Color, Rgba};
use crate::core::event::{KeyCode, KeyEvent};
use crate::core::geometry::{Bounds, Edges, Point};
use crate::core::style::Corners;
use crate::core::text_editing::{
    TextEditError, TextEditLayout, TextEditOutcome, TextEditPaintStyle, TextInputCommand,
    TextInputGeometry, TextRange, TextSelection,
};
use crate::elements::element::{EventContext, PaintContext};
use crate::renderer::Primitive;
use crate::renderer::text::{TextMeasureCache, TextRequest};

impl TextArea {
    pub fn apply_text_input_command(
        &mut self,
        command: TextInputCommand,
    ) -> Result<TextEditOutcome, TextEditError> {
        if !self.can_edit() {
            return Ok(TextEditOutcome::default());
        }
        self.sync_editor_from_public_state_if_needed()?;
        self.visual_caret = None;
        let outcome = self.editor.apply_text_input_command(command)?;
        self.sync_state_from_editor();
        self.emit_change_if_needed(outcome.changed);
        Ok(outcome)
    }

    pub(super) fn handle_text_input_command_impl(
        &mut self,
        cx: &mut EventContext,
        command: &TextInputCommand,
    ) -> bool {
        if !cx.is_focused(self.id) && !self.state.focused {
            return false;
        }
        match self.apply_text_input_command(command.clone()) {
            Ok(_) => {
                cx.request_redraw();
                true
            }
            Err(err) => {
                log::error!("text area text input command failed: {err}");
                false
            }
        }
    }

    pub(super) fn apply_shaped_navigation(
        &mut self,
        event: &KeyEvent,
    ) -> Option<Result<TextEditOutcome, TextEditError>> {
        if event.modifiers.alt || event.modifiers.ctrl || event.modifiers.meta {
            return None;
        }
        let selection = self.state_selection();
        let layout = self.current_text_layout()?;
        let current = self
            .visual_caret
            .filter(|caret| caret.offset() == selection.head())
            .map(Ok)
            .unwrap_or_else(|| layout.visual_caret_for_offset(selection.head()));
        let current = match current {
            Ok(caret) => caret,
            Err(err) => return Some(Err(err)),
        };
        let visual_target = if !event.modifiers.shift && !selection.is_collapsed() {
            match event.key {
                KeyCode::ArrowLeft => {
                    layout.visual_selection_caret(selection.normalized_range(), false)
                }
                KeyCode::ArrowRight => {
                    layout.visual_selection_caret(selection.normalized_range(), true)
                }
                _ => return None,
            }
        } else {
            match event.key {
                KeyCode::ArrowLeft => layout.visual_caret_horizontal(current, false),
                KeyCode::ArrowRight => layout.visual_caret_horizontal(current, true),
                KeyCode::ArrowUp => layout.visual_caret_vertical(current, false),
                KeyCode::ArrowDown => layout.visual_caret_vertical(current, true),
                KeyCode::Home => layout.visual_line_edge_caret(selection.head(), false),
                KeyCode::End => layout.visual_line_edge_caret(selection.head(), true),
                _ => return None,
            }
        };
        let visual_target = match visual_target {
            Ok(target) => target,
            Err(err) => return Some(Err(err)),
        };
        let target = visual_target.offset();
        Some((|| {
            self.sync_editor_from_public_state_if_needed()?;
            let selection = if event.modifiers.shift {
                TextSelection::new(self.editor.selection().anchor(), target)
            } else {
                TextSelection::collapsed(target)
            };
            self.editor.set_selection(selection)?;
            self.visual_caret = Some(visual_target);
            self.sync_state_from_editor();
            Ok(TextEditOutcome::default())
        })())
    }

    pub(super) fn set_cursor_from_shaped_point(
        &mut self,
        point: Point,
        bounds: Bounds,
    ) -> Result<bool, TextEditError> {
        let origin = self.text_origin(bounds);
        let visual_target = match self.current_text_layout() {
            Some(layout) => {
                layout.visual_caret_for_point(Point::new(point.x - origin.x, point.y - origin.y))
            }
            None => return Ok(false),
        };
        let target = visual_target.offset();
        self.sync_editor_from_public_state_if_needed()?;
        self.editor.set_cursor(target)?;
        self.visual_caret = Some(visual_target);
        self.sync_state_from_editor();
        Ok(true)
    }

    pub(super) fn current_text_layout(&self) -> Option<&TextEditLayout> {
        self.text_layout
            .as_ref()
            .filter(|layout| layout.text() == self.state.value)
    }

    pub(super) fn native_text_input_geometry(&self) -> Option<TextInputGeometry> {
        let layout = self.current_text_layout()?;
        let selection_head = self.state_selection().head();
        let caret = match self
            .visual_caret
            .filter(|caret| caret.offset() == selection_head)
        {
            Some(caret) => layout.caret_geometry_for_visual_caret(caret).ok()?,
            None => layout.caret_for_offset(selection_head).ok()?,
        };
        let bounds = self.caret_bounds?;
        Some(
            TextInputGeometry::new(
                layout.clone(),
                Point::new(bounds.x() - caret.position.x, bounds.y() - caret.position.y),
            )
            .with_visual_caret(self.visual_caret),
        )
    }

    pub(super) fn update_text_layout(&mut self, cache: &mut TextMeasureCache) {
        let plans = self
            .state
            .value
            .split('\n')
            .map(|line| {
                cache
                    .shape_single_line(TextRequest::new(line, 14.0, 400, None, 1.0))
                    .unwrap_or_else(|err| panic!("text area shaping failed: {err:?}"))
            })
            .collect::<Vec<_>>();
        self.text_layout = Some(
            TextEditLayout::from_line_shape_plans(
                self.state.value.clone(),
                &plans,
                TEXT_AREA_LINE_HEIGHT,
            )
            .unwrap_or_else(|err| panic!("text area layout failed: {err}")),
        );
    }

    pub(super) fn refresh_text_layout_if_stale(&mut self, cache: &mut TextMeasureCache) {
        if self.current_text_layout().is_none() {
            self.update_text_layout(cache);
        }
    }

    pub(super) fn text_layout(&self) -> &TextEditLayout {
        self.text_layout
            .as_ref()
            .unwrap_or_else(|| panic!("text area layout was not prepared before paint"))
    }

    pub(super) fn text_origin(&self, bounds: Bounds) -> Point {
        Point::new(
            bounds.x() + TEXT_AREA_HORIZONTAL_PADDING,
            bounds.y() + TEXT_AREA_VERTICAL_PADDING,
        )
    }

    pub(super) fn text_width(&self, bounds: Bounds) -> f32 {
        bounds.width() - (TEXT_AREA_HORIZONTAL_PADDING * 2.0)
    }

    pub(super) fn paint_selection_and_marked_text(&self, cx: &mut PaintContext, bounds: Bounds) {
        if !self.state.focused {
            return;
        }
        let layout = self.text_layout();
        let style = TextEditPaintStyle::new(
            TEXT_AREA_CARET_WIDTH,
            Color::hex(0x6366f1).to_rgba(),
            Color::hex(0x6366f1).with_alpha(0.22).to_rgba(),
        );
        let paint_origin = self.text_origin(bounds);

        if let (Some(start), Some(end)) = (self.state.selection_start, self.state.selection_end)
            && let Ok(range) = TextRange::new(start, end)
            && let Ok(primitives) = layout.selection_primitives(range, paint_origin, style)
        {
            for primitive in primitives {
                cx.paint(primitive);
            }
        }

        if let Some(range) = self.state.composition_range
            && let Ok(rects) = layout.selection_rects(range)
        {
            for rect in rects {
                cx.paint(Primitive::Quad {
                    bounds: Bounds::from_xywh(
                        paint_origin.x + rect.bounds.x(),
                        paint_origin.y + rect.bounds.y() + rect.bounds.height()
                            - TEXT_AREA_MARKED_UNDERLINE_HEIGHT,
                        rect.bounds.width(),
                        TEXT_AREA_MARKED_UNDERLINE_HEIGHT,
                    ),
                    background: Color::hex(0x6366f1).to_rgba(),
                    border_color: Rgba::TRANSPARENT,
                    border_widths: Edges::ZERO,
                    corner_radii: Corners::ZERO,
                });
            }
        }
    }

    pub(super) fn paint_cursor(&self, cx: &mut PaintContext, bounds: Bounds) -> Option<Bounds> {
        if !self.state.focused {
            return None;
        }
        let style = TextEditPaintStyle::new(
            TEXT_AREA_CARET_WIDTH,
            Color::hex(0x6366f1).to_rgba(),
            Color::hex(0x6366f1).with_alpha(0.22).to_rgba(),
        );
        match self.text_layout().caret_primitive_for_visual_caret(
            self.normalize_cursor_position(),
            self.visual_caret,
            self.text_origin(bounds),
            style,
        ) {
            Ok(primitive) => {
                let caret_bounds = match &primitive {
                    Primitive::Quad { bounds, .. } => Some(*bounds),
                    _ => None,
                };
                cx.paint(primitive);
                caret_bounds
            }
            Err(err) => {
                log::error!("text area caret paint failed: {err}");
                None
            }
        }
    }
}
