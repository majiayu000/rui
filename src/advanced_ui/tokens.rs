use crate::core::color::Color;

pub const CONTROL_RADIUS: f32 = 6.0;
pub const CONTROL_GAP: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl ControlSize {
    pub fn control_height(self) -> f32 {
        match self {
            Self::Small => 28.0,
            Self::Medium => 36.0,
            Self::Large => 44.0,
        }
    }

    pub fn text_size(self) -> f32 {
        match self {
            Self::Small => 12.0,
            Self::Medium => 14.0,
            Self::Large => 16.0,
        }
    }

    pub fn horizontal_padding(self) -> f32 {
        match self {
            Self::Small => 12.0,
            Self::Medium => 16.0,
            Self::Large => 20.0,
        }
    }

    pub fn indicator_extent(self) -> f32 {
        match self {
            Self::Small => 14.0,
            Self::Medium => 16.0,
            Self::Large => 18.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlVariant {
    #[default]
    Primary,
    Secondary,
    Outline,
    Ghost,
    Danger,
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ControlState {
    pub hovered: bool,
    pub pressed: bool,
    pub selected: bool,
    pub disabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlColors {
    pub background: Color,
    pub foreground: Color,
    pub border: Color,
}

pub fn control_colors(variant: ControlVariant, state: ControlState) -> ControlColors {
    let selected = state.selected;
    let pressed = state.pressed || selected;
    let alpha = if state.disabled { 0.5 } else { 1.0 };

    let colors = match variant {
        ControlVariant::Primary => {
            let background = if pressed {
                Color::hex(0x1d4ed8)
            } else if state.hovered {
                Color::hex(0x2563eb)
            } else {
                Color::hex(0x3b82f6)
            };
            ControlColors {
                background,
                foreground: Color::WHITE,
                border: Color::TRANSPARENT,
            }
        }
        ControlVariant::Secondary => {
            let background = if pressed {
                Color::hex(0x374151)
            } else if state.hovered {
                Color::hex(0x4b5563)
            } else {
                Color::hex(0x6b7280)
            };
            ControlColors {
                background,
                foreground: Color::WHITE,
                border: Color::TRANSPARENT,
            }
        }
        ControlVariant::Outline => {
            let border = if pressed {
                Color::hex(0x1d4ed8)
            } else if state.hovered {
                Color::hex(0x2563eb)
            } else {
                Color::hex(0x93c5fd)
            };
            ControlColors {
                background: if pressed {
                    Color::hex(0xdbeafe)
                } else if state.hovered {
                    Color::hex(0xeff6ff)
                } else {
                    Color::TRANSPARENT
                },
                foreground: Color::hex(0x1d4ed8),
                border,
            }
        }
        ControlVariant::Ghost => ControlColors {
            background: if pressed {
                Color::hex(0xe5e7eb)
            } else if state.hovered {
                Color::hex(0xf3f4f6)
            } else {
                Color::TRANSPARENT
            },
            foreground: Color::hex(0x374151),
            border: Color::TRANSPARENT,
        },
        ControlVariant::Danger => {
            let background = if pressed {
                Color::hex(0xb91c1c)
            } else if state.hovered {
                Color::hex(0xdc2626)
            } else {
                Color::hex(0xef4444)
            };
            ControlColors {
                background,
                foreground: Color::WHITE,
                border: Color::TRANSPARENT,
            }
        }
        ControlVariant::Success => {
            let background = if pressed {
                Color::hex(0x15803d)
            } else if state.hovered {
                Color::hex(0x16a34a)
            } else {
                Color::hex(0x22c55e)
            };
            ControlColors {
                background,
                foreground: Color::WHITE,
                border: Color::TRANSPARENT,
            }
        }
    };

    if state.disabled {
        ControlColors {
            background: colors.background.with_alpha(alpha),
            foreground: colors.foreground.with_alpha(alpha),
            border: colors.border.with_alpha(alpha),
        }
    } else {
        colors
    }
}

pub fn surface_color() -> Color {
    Color::WHITE
}

pub fn control_border_color() -> Color {
    Color::hex(0xd1d5db)
}

pub fn text_color() -> Color {
    Color::hex(0x111827)
}

pub fn disabled_surface_color() -> Color {
    Color::hex(0xf3f4f6)
}
