//! What shaping reports: clusters, runs, glyphs and diagnostics.

use crate::renderer::text::TextMetrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextScript {
    Latin,
    Cjk,
    Emoji,
    Rtl,
    Number,
    Whitespace,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextDirection {
    LeftToRight,
    RightToLeft,
    Neutral,
    Mixed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShapedGlyph {
    pub byte_start: usize,
    pub byte_end: usize,
    pub glyph_id: u16,
    pub font_family: String,
    pub x_offset: f32,
    pub y_offset: f32,
    pub advance_width: f32,
    pub(crate) font_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextCluster {
    pub byte_start: usize,
    pub byte_end: usize,
    pub text: String,
    pub script: TextScript,
    /// The direction of the cluster's own characters: neutral for spaces,
    /// digits and punctuation, whichever way they are laid out.
    pub direction: TextDirection,
    /// UAX #9 embedding level the cluster is laid out at; odd is right to left.
    pub bidi_level: u8,
    pub font_family: String,
    pub x_offset: f32,
    pub advance_width: f32,
}

impl TextCluster {
    /// Whether the cluster is laid out right to left, so that its logical
    /// start is its right edge.
    pub fn is_rtl(&self) -> bool {
        self.bidi_level % 2 == 1
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextRun {
    pub byte_start: usize,
    pub byte_end: usize,
    pub script: TextScript,
    /// ISO 15924 code of the script the run was shaped as: `Latn`, `Arab`,
    /// `Hani` and so on, `Zsye` for emoji, `Zyyy` for text no script owns.
    pub script_tag: [u8; 4],
    pub direction: TextDirection,
    pub font_family: String,
    pub x_offset: f32,
    pub advance_width: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TextShapeDiagnostic {
    MissingGlyph {
        byte_start: usize,
        byte_end: usize,
        cluster: String,
    },
    FallbackRequired {
        byte_start: usize,
        byte_end: usize,
        cluster: String,
        script: TextScript,
    },
    FallbackApplied {
        byte_start: usize,
        byte_end: usize,
        cluster: String,
        requested_family: String,
        fallback_family: String,
    },
    FallbackFailed {
        byte_start: usize,
        byte_end: usize,
        cluster: String,
        script: TextScript,
    },
    UnsupportedScript {
        byte_start: usize,
        byte_end: usize,
        cluster: String,
        script: TextScript,
    },
    LigatureSubstitution {
        byte_start: usize,
        byte_end: usize,
        text: String,
        glyph_count: usize,
        grapheme_count: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextShapePlan {
    pub(crate) clusters: Vec<TextCluster>,
    pub(crate) runs: Vec<TextRun>,
    pub(crate) glyphs: Vec<ShapedGlyph>,
    pub(crate) diagnostics: Vec<TextShapeDiagnostic>,
    pub(crate) direction: TextDirection,
    pub(crate) metrics: TextMetrics,
}

impl TextShapePlan {
    pub fn empty() -> Self {
        Self {
            clusters: Vec::new(),
            runs: Vec::new(),
            glyphs: Vec::new(),
            diagnostics: Vec::new(),
            direction: TextDirection::Neutral,
            metrics: TextMetrics::empty(),
        }
    }

    pub fn clusters(&self) -> &[TextCluster] {
        &self.clusters
    }

    pub fn runs(&self) -> &[TextRun] {
        &self.runs
    }

    pub fn glyphs(&self) -> &[ShapedGlyph] {
        &self.glyphs
    }

    pub fn diagnostics(&self) -> &[TextShapeDiagnostic] {
        &self.diagnostics
    }

    pub fn direction(&self) -> TextDirection {
        self.direction
    }

    pub fn metrics(&self) -> TextMetrics {
        self.metrics
    }
}
