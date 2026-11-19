//! Fonts the GPUI fixtures share. `run.sh` copies this file next to `rui_compare.rs`.

use super::{EDITOR_FONT, EDITOR_FONT_PATH};

/// Loads the SF Mono file RUI reads, and fails rather than measure a fallback font.
/// CoreText hides SF Mono from apps.
pub(crate) fn load_sf_mono(text_system: &gpui::TextSystem) {
    let sf_mono = match std::fs::read(EDITOR_FONT_PATH) {
        Ok(bytes) => bytes,
        Err(err) => panic!("failed to read {EDITOR_FONT_PATH}: {err}"),
    };
    if let Err(err) = text_system.add_fonts(vec![sf_mono.into()]) {
        panic!("GPUI failed to load {EDITOR_FONT_PATH}: {err}");
    }
    let resolved =
        text_system.get_font_for_id(text_system.resolve_font(&gpui::font(EDITOR_FONT)));
    if resolved.as_ref().map(|font| font.family.as_ref()) != Some(EDITOR_FONT) {
        panic!("GPUI resolved {EDITOR_FONT} to {resolved:?}");
    }
}
