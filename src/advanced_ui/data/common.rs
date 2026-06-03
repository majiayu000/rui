use crate::advanced_ui::state::require_finite_non_negative;
use crate::advanced_ui::tokens::{ControlSize, ControlState, Theme};
use crate::core::color::Color;
use crate::core::geometry::{Bounds, Edges, Point};
use crate::core::style::{Corners, Style};
use crate::elements::element::PaintContext;
use crate::elements::text::TextAlign;
use crate::renderer::Primitive;

pub(super) type DataValueHandler = Box<dyn Fn(&str)>;
pub(super) type DataRowHandler = Box<dyn Fn()>;

pub(super) const DEFAULT_DATA_WIDTH: f32 = 280.0;
pub(super) const DATA_PADDING_X: f32 = 12.0;
const DATA_DETAIL_ALPHA: f32 = 0.72;

pub(super) struct DataRowPaint<'a> {
    pub label: &'a str,
    pub detail: Option<&'a str>,
    pub depth: usize,
    pub indent_width: f32,
    pub marker: Option<&'a str>,
    pub selected: bool,
    pub hovered: bool,
    pub pressed: bool,
    pub disabled: bool,
    pub invalid: bool,
    pub size: ControlSize,
    pub theme: Theme,
    pub align: TextAlign,
}

pub(super) fn base_data_style(radius: f32) -> Style {
    let mut style = Style::new();
    style.border.radius = Corners::all(radius);
    style
}

pub(super) fn validate_dimension(value: f32, message: &str) {
    require_finite_non_negative(value, message);
}

pub(super) fn paint_surface(
    cx: &mut PaintContext,
    bounds: Bounds,
    invalid: bool,
    disabled: bool,
    corners: Corners,
    theme: Theme,
) {
    let state = ControlState {
        disabled,
        invalid,
        ..ControlState::default()
    };
    cx.paint(Primitive::Quad {
        bounds,
        background: theme.surface_color_for_state(state).to_rgba(),
        border_color: theme
            .validation_border_color(invalid, theme.colors.border)
            .to_rgba(),
        border_widths: Edges::all(1.0),
        corner_radii: corners,
    });
}

pub(super) fn paint_data_row(cx: &mut PaintContext, bounds: Bounds, row: DataRowPaint<'_>) {
    cx.paint(Primitive::Quad {
        bounds,
        background: row_background(
            row.selected,
            row.hovered,
            row.pressed,
            row.disabled,
            row.theme,
        )
        .to_rgba(),
        border_color: row
            .theme
            .validation_border_color(row.invalid, Color::TRANSPARENT)
            .to_rgba(),
        border_widths: if row.invalid {
            Edges::all(1.0)
        } else {
            Edges::ZERO
        },
        corner_radii: Corners::ZERO,
    });

    let text_fg = disabled_aware_color(row.theme.colors.text, row.disabled);
    let text_size = row.theme.text_size(row.size);
    let marker_indent = row.depth as f32 * row.indent_width;
    let content_x =
        bounds.x() + DATA_PADDING_X + marker_indent + if row.marker.is_some() { 16.0 } else { 0.0 };

    if let Some(marker) = row.marker {
        cx.paint(Primitive::Text {
            bounds: Bounds::from_xywh(
                bounds.x() + DATA_PADDING_X + marker_indent,
                bounds.y(),
                14.0,
                bounds.height(),
            ),
            content: marker.to_string(),
            color: text_fg.to_rgba(),
            font_size: text_size,
            font_weight: row.theme.typography.control_weight,
            font_family: None,
            line_height: row.theme.typography.line_height,
            align: TextAlign::Center,
        });
    }

    let label_height = if row.detail.is_some() {
        bounds.height() * 0.55
    } else {
        bounds.height()
    };
    cx.paint(Primitive::Text {
        bounds: Bounds::from_xywh(
            content_x,
            bounds.y(),
            (bounds.max_x() - content_x - DATA_PADDING_X).max(0.0),
            label_height,
        ),
        content: row.label.to_string(),
        color: text_fg.to_rgba(),
        font_size: text_size,
        font_weight: if row.selected {
            row.theme.typography.selected_weight
        } else {
            row.theme.typography.label_weight
        },
        font_family: None,
        line_height: row.theme.typography.line_height,
        align: row.align,
    });

    if let Some(detail) = row.detail {
        cx.paint(Primitive::Text {
            bounds: Bounds::from_xywh(
                content_x,
                bounds.y() + label_height,
                (bounds.max_x() - content_x - DATA_PADDING_X).max(0.0),
                bounds.height() - label_height,
            ),
            content: detail.to_string(),
            color: disabled_aware_color(row.theme.colors.text, row.disabled)
                .with_alpha(DATA_DETAIL_ALPHA)
                .to_rgba(),
            font_size: (text_size - 2.0).max(10.0),
            font_weight: 400,
            font_family: None,
            line_height: row.theme.typography.line_height,
            align: row.align,
        });
    }

    cx.paint(Primitive::Quad {
        bounds: Bounds::from_xywh(bounds.x(), bounds.max_y() - 1.0, bounds.width(), 1.0),
        background: row.theme.colors.border.to_rgba(),
        border_color: Color::TRANSPARENT.to_rgba(),
        border_widths: Edges::ZERO,
        corner_radii: Corners::ZERO,
    });
}

pub(super) fn row_background(
    selected: bool,
    hovered: bool,
    pressed: bool,
    disabled: bool,
    theme: Theme,
) -> Color {
    if disabled {
        theme.surface_color_for_state(ControlState {
            disabled: true,
            ..ControlState::default()
        })
    } else if pressed {
        theme.state.pressed_surface
    } else if selected {
        theme.colors.primary.rest.background.with_alpha(0.12)
    } else if hovered {
        theme.surface_color_for_state(ControlState {
            hovered: true,
            ..ControlState::default()
        })
    } else {
        theme.colors.surface
    }
}

pub(super) fn disabled_aware_color(color: Color, disabled: bool) -> Color {
    if disabled {
        color.with_alpha(0.5)
    } else {
        color
    }
}

pub(super) fn index_at_row(
    bounds: Bounds,
    position: Point,
    row_height: f32,
    row_count: usize,
) -> Option<usize> {
    if row_height <= 0.0 || row_count == 0 || !bounds.contains(position) {
        return None;
    }
    let index = ((position.y - bounds.y()) / row_height).floor() as usize;
    (index < row_count).then_some(index)
}

pub(super) fn row_bounds(bounds: Bounds, row_height: f32, index: usize) -> Option<Bounds> {
    if row_height <= 0.0 {
        return None;
    }
    let y = bounds.y() + index as f32 * row_height;
    let height = (bounds.max_y() - y).min(row_height);
    (height > 0.0).then(|| Bounds::from_xywh(bounds.x(), y, bounds.width(), height))
}

pub(super) fn data_corners(theme: Theme) -> Corners {
    Corners::all(theme.control_radius())
}
