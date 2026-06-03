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
    pub read_only: bool,
    pub invalid: bool,
    pub focused: bool,
    pub loading: bool,
    pub error: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlColors {
    pub background: Color,
    pub foreground: Color,
    pub border: Color,
}

impl ControlColors {
    pub fn with_alpha(self, alpha: f32) -> Self {
        Self {
            background: self.background.with_alpha(alpha),
            foreground: self.foreground.with_alpha(alpha),
            border: self.border.with_alpha(alpha),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
    HighContrast,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlVariantPalette {
    pub rest: ControlColors,
    pub hovered: ControlColors,
    pub pressed: ControlColors,
}

impl ControlVariantPalette {
    pub fn new(rest: ControlColors, hovered: ControlColors, pressed: ControlColors) -> Self {
        Self {
            rest,
            hovered,
            pressed,
        }
    }

    pub fn solid(rest: Color, hovered: Color, pressed: Color, foreground: Color) -> Self {
        Self::new(
            ControlColors {
                background: rest,
                foreground,
                border: Color::TRANSPARENT,
            },
            ControlColors {
                background: hovered,
                foreground,
                border: Color::TRANSPARENT,
            },
            ControlColors {
                background: pressed,
                foreground,
                border: Color::TRANSPARENT,
            },
        )
    }

    pub fn resolve(self, state: ControlState) -> ControlColors {
        if state.pressed || state.selected {
            self.pressed
        } else if state.hovered {
            self.hovered
        } else {
            self.rest
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeColors {
    pub surface: Color,
    pub surface_muted: Color,
    pub text: Color,
    pub text_on_accent: Color,
    pub border: Color,
    pub primary: ControlVariantPalette,
    pub secondary: ControlVariantPalette,
    pub outline: ControlVariantPalette,
    pub ghost: ControlVariantPalette,
    pub danger: ControlVariantPalette,
    pub success: ControlVariantPalette,
    pub progress_track: Color,
    pub progress_fill: Color,
    pub tooltip_background: Color,
    pub tooltip_text: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeRadius {
    pub control: f32,
    pub indicator: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeSpacing {
    pub control_gap: f32,
    pub toolbar_padding: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTypography {
    pub text_scale: f32,
    pub control_weight: u16,
    pub label_weight: u16,
    pub selected_weight: u16,
    pub line_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeDensity {
    pub scale: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeStateTokens {
    pub disabled_opacity: f32,
    pub read_only_opacity: f32,
    pub invalid_border: Color,
    pub focus_ring: Color,
    pub hover_surface: Color,
    pub pressed_surface: Color,
    pub loading_surface: Color,
    pub error: Color,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub mode: ThemeMode,
    pub colors: ThemeColors,
    pub radius: ThemeRadius,
    pub spacing: ThemeSpacing,
    pub typography: ThemeTypography,
    pub density: ThemeDensity,
    pub state: ThemeStateTokens,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            colors: ThemeColors {
                surface: Color::WHITE,
                surface_muted: Color::hex(0xf3f4f6),
                text: Color::hex(0x111827),
                text_on_accent: Color::WHITE,
                border: Color::hex(0xd1d5db),
                primary: ControlVariantPalette::solid(
                    Color::hex(0x3b82f6),
                    Color::hex(0x2563eb),
                    Color::hex(0x1d4ed8),
                    Color::WHITE,
                ),
                secondary: ControlVariantPalette::solid(
                    Color::hex(0x6b7280),
                    Color::hex(0x4b5563),
                    Color::hex(0x374151),
                    Color::WHITE,
                ),
                outline: outline_palette(Color::hex(0x1d4ed8), Color::hex(0x93c5fd)),
                ghost: ControlVariantPalette::solid(
                    Color::TRANSPARENT,
                    Color::hex(0xf3f4f6),
                    Color::hex(0xe5e7eb),
                    Color::hex(0x374151),
                ),
                danger: ControlVariantPalette::solid(
                    Color::hex(0xef4444),
                    Color::hex(0xdc2626),
                    Color::hex(0xb91c1c),
                    Color::WHITE,
                ),
                success: ControlVariantPalette::solid(
                    Color::hex(0x22c55e),
                    Color::hex(0x16a34a),
                    Color::hex(0x15803d),
                    Color::WHITE,
                ),
                progress_track: Color::hex(0xe5e7eb),
                progress_fill: Color::hex(0x2563eb),
                tooltip_background: Color::hex(0x111827),
                tooltip_text: Color::WHITE,
            },
            radius: ThemeRadius {
                control: CONTROL_RADIUS,
                indicator: CONTROL_RADIUS / 2.0,
            },
            spacing: ThemeSpacing {
                control_gap: CONTROL_GAP,
                toolbar_padding: 6.0,
            },
            typography: ThemeTypography {
                text_scale: 1.0,
                control_weight: 600,
                label_weight: 500,
                selected_weight: 700,
                line_height: 1.2,
            },
            density: ThemeDensity { scale: 1.0 },
            state: ThemeStateTokens {
                disabled_opacity: 0.5,
                read_only_opacity: 0.72,
                invalid_border: Color::hex(0xdc2626),
                focus_ring: Color::hex(0x2563eb),
                hover_surface: Color::hex(0xf3f4f6),
                pressed_surface: Color::hex(0xe5e7eb),
                loading_surface: Color::hex(0xe0f2fe),
                error: Color::hex(0xdc2626),
            },
        }
    }

    pub fn dark() -> Self {
        let mut theme = Self::light();
        theme.mode = ThemeMode::Dark;
        theme.colors.surface = Color::hex(0x111827);
        theme.colors.surface_muted = Color::hex(0x1f2937);
        theme.colors.text = Color::hex(0xf9fafb);
        theme.colors.border = Color::hex(0x374151);
        theme.colors.outline = outline_palette(Color::hex(0x93c5fd), Color::hex(0x60a5fa));
        theme.colors.ghost = ControlVariantPalette::solid(
            Color::TRANSPARENT,
            Color::hex(0x1f2937),
            Color::hex(0x374151),
            Color::hex(0xf9fafb),
        );
        theme.colors.progress_track = Color::hex(0x374151);
        theme.colors.tooltip_background = Color::hex(0xf9fafb);
        theme.colors.tooltip_text = Color::hex(0x111827);
        theme.state.hover_surface = Color::hex(0x1f2937);
        theme.state.pressed_surface = Color::hex(0x374151);
        theme.state.loading_surface = Color::hex(0x0f172a);
        theme
    }

    pub fn high_contrast() -> Self {
        let mut theme = Self::light();
        theme.mode = ThemeMode::HighContrast;
        theme.colors.surface = Color::BLACK;
        theme.colors.surface_muted = Color::hex(0x1a1a1a);
        theme.colors.text = Color::WHITE;
        theme.colors.text_on_accent = Color::BLACK;
        theme.colors.border = Color::WHITE;
        theme.colors.primary = ControlVariantPalette::solid(
            Color::hex(0xffff00),
            Color::hex(0xffd700),
            Color::hex(0xffbf00),
            Color::BLACK,
        );
        theme.colors.secondary = ControlVariantPalette::solid(
            Color::WHITE,
            Color::hex(0xe5e7eb),
            Color::hex(0xd1d5db),
            Color::BLACK,
        );
        theme.colors.outline = outline_palette(Color::WHITE, Color::WHITE);
        theme.colors.ghost = ControlVariantPalette::solid(
            Color::TRANSPARENT,
            Color::hex(0x333333),
            Color::hex(0x4d4d4d),
            Color::WHITE,
        );
        theme.colors.progress_track = Color::BLACK;
        theme.colors.progress_fill = Color::hex(0xffff00);
        theme.colors.tooltip_background = Color::WHITE;
        theme.colors.tooltip_text = Color::BLACK;
        theme.state.disabled_opacity = 0.65;
        theme.state.invalid_border = Color::hex(0xff0000);
        theme.state.focus_ring = Color::hex(0x00ffff);
        theme.state.hover_surface = Color::hex(0x333333);
        theme.state.pressed_surface = Color::hex(0x4d4d4d);
        theme.state.error = Color::hex(0xff0000);
        theme
    }

    pub fn with_colors(mut self, colors: ThemeColors) -> Self {
        self.colors = colors;
        self
    }

    pub fn with_radius(mut self, radius: ThemeRadius) -> Self {
        self.radius = radius;
        self
    }

    pub fn with_spacing(mut self, spacing: ThemeSpacing) -> Self {
        self.spacing = spacing;
        self
    }

    pub fn with_typography(mut self, typography: ThemeTypography) -> Self {
        self.typography = typography;
        self
    }

    pub fn with_density(mut self, density: ThemeDensity) -> Self {
        self.density = density;
        self
    }

    pub fn control_colors(self, variant: ControlVariant, state: ControlState) -> ControlColors {
        let mut colors = self.variant_palette(variant).resolve(state);
        if state.loading {
            colors.background = self.state.loading_surface;
        }
        colors.border = self.state_border_color(state, colors.border);
        if state.disabled {
            colors.with_alpha(self.state.disabled_opacity)
        } else if state.read_only {
            colors.with_alpha(self.state.read_only_opacity)
        } else {
            colors
        }
    }

    pub fn state_border_color(self, state: ControlState, default: Color) -> Color {
        if state.error {
            self.state.error
        } else if state.invalid {
            self.state.invalid_border
        } else if state.focused {
            self.state.focus_ring
        } else {
            default
        }
    }

    pub fn validation_border_color(self, invalid: bool, default: Color) -> Color {
        if invalid {
            self.state.invalid_border
        } else {
            default
        }
    }

    pub fn surface_color_for_state(self, state: ControlState) -> Color {
        let color = if state.disabled || state.loading {
            self.colors.surface_muted
        } else if state.hovered {
            self.state.hover_surface
        } else if state.pressed {
            self.state.pressed_surface
        } else {
            self.colors.surface
        };
        if state.disabled {
            color.with_alpha(self.state.disabled_opacity)
        } else if state.read_only {
            color.with_alpha(self.state.read_only_opacity)
        } else {
            color
        }
    }

    pub fn control_height(self, size: ControlSize) -> f32 {
        size.control_height() * self.density.scale
    }

    pub fn text_size(self, size: ControlSize) -> f32 {
        size.text_size() * self.typography.text_scale
    }

    pub fn horizontal_padding(self, size: ControlSize) -> f32 {
        size.horizontal_padding() * self.density.scale
    }

    pub fn indicator_extent(self, size: ControlSize) -> f32 {
        size.indicator_extent() * self.density.scale
    }

    pub fn control_gap(self) -> f32 {
        self.spacing.control_gap * self.density.scale
    }

    pub fn toolbar_padding(self) -> f32 {
        self.spacing.toolbar_padding * self.density.scale
    }

    pub fn control_radius(self) -> f32 {
        self.radius.control
    }

    pub fn indicator_radius(self) -> f32 {
        self.radius.indicator
    }

    fn variant_palette(self, variant: ControlVariant) -> ControlVariantPalette {
        match variant {
            ControlVariant::Primary => self.colors.primary,
            ControlVariant::Secondary => self.colors.secondary,
            ControlVariant::Outline => self.colors.outline,
            ControlVariant::Ghost => self.colors.ghost,
            ControlVariant::Danger => self.colors.danger,
            ControlVariant::Success => self.colors.success,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::light()
    }
}

fn outline_palette(foreground: Color, border: Color) -> ControlVariantPalette {
    ControlVariantPalette::new(
        ControlColors {
            background: Color::TRANSPARENT,
            foreground,
            border,
        },
        ControlColors {
            background: Color::hex(0xeff6ff),
            foreground,
            border: foreground,
        },
        ControlColors {
            background: Color::hex(0xdbeafe),
            foreground,
            border: foreground,
        },
    )
}
