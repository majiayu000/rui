//! Process-level loading of the system font set used for text measurement.
//!
//! Font faces are parsed once per process and shared through an `Arc`, because
//! the measurement caches that hold them are rebuilt far more often than the
//! font set changes. Loading is deliberately *retryable*: an empty result is
//! never stored, so a transient read failure cannot freeze an empty font set
//! into the rest of the process lifetime.

use crate::renderer::text_shaping::TextShapingFont;
use rusttype::Font;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Maximum number of faces probed inside a single font collection file.
const MAX_FACES_PER_FILE: u32 = 16;

const FONT_CANDIDATES: [(&str, &str); 19] = [
    ("Arial", "/System/Library/Fonts/Supplemental/Arial.ttf"),
    ("Arial", "/Library/Fonts/Arial.ttf"),
    (
        "Arial Unicode",
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    ),
    ("Arial Unicode", "/Library/Fonts/Arial Unicode.ttf"),
    ("Helvetica", "/System/Library/Fonts/Helvetica.ttc"),
    ("Helvetica Neue", "/System/Library/Fonts/HelveticaNeue.ttc"),
    ("Geneva", "/System/Library/Fonts/Geneva.ttf"),
    ("SF Pro", "/System/Library/Fonts/SFNS.ttf"),
    ("SF Mono", "/System/Library/Fonts/SFNSMono.ttf"),
    ("SF Hebrew", "/System/Library/Fonts/SFHebrew.ttf"),
    ("SF Arabic", "/System/Library/Fonts/SFArabic.ttf"),
    ("Geeza Pro", "/System/Library/Fonts/GeezaPro.ttc"),
    (
        "Hiragino Sans GB",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
    ),
    ("STHeiti", "/System/Library/Fonts/STHeiti Medium.ttc"),
    (
        "Apple Color Emoji",
        "/System/Library/Fonts/Apple Color Emoji.ttc",
    ),
    ("Apple Symbols", "/System/Library/Fonts/Apple Symbols.ttf"),
    (
        "DejaVu Sans",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ),
    (
        "Liberation Sans",
        "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
    ),
    (
        "Noto Sans CJK",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ),
];

/// A lazily populated, process-wide resource set that tolerates load failures.
///
/// An empty load result is handed back to the caller but never cached, so the
/// next caller retries the load instead of inheriting a permanently empty set.
/// Callers are expected to turn an empty set into an explicit error rather than
/// rendering without it.
pub(crate) struct RetryableResourceSet<T> {
    slot: Mutex<Option<Arc<Vec<T>>>>,
    loads: AtomicUsize,
}

impl<T> RetryableResourceSet<T> {
    pub(crate) const fn new() -> Self {
        Self {
            slot: Mutex::new(None),
            loads: AtomicUsize::new(0),
        }
    }

    /// Number of times the loader has actually run. Used by tests to prove that
    /// a successful load is not repeated, and that an empty one is retried.
    #[cfg(test)]
    pub(crate) fn load_count(&self) -> usize {
        self.loads.load(Ordering::Relaxed)
    }

    pub(crate) fn get_or_load(&self, load: impl FnOnce() -> Vec<T>) -> Arc<Vec<T>> {
        // The slot is only written after `load` returns, so a loader that panics
        // cannot leave a half-built value behind: a poisoned lock still guards a
        // valid `Option`. Recovering it keeps one caller's panic from turning
        // every later load into an unrelated panic.
        let mut slot = match self.slot.lock() {
            Ok(slot) => slot,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(cached) = slot.as_ref() {
            return Arc::clone(cached);
        }

        self.loads.fetch_add(1, Ordering::Relaxed);
        let loaded = Arc::new(load());
        if !loaded.is_empty() {
            *slot = Some(Arc::clone(&loaded));
        }
        loaded
    }
}

static SYSTEM_TEXT_FONTS: RetryableResourceSet<TextShapingFont> = RetryableResourceSet::new();

/// Shared system font set for this process, loading it on first use.
///
/// Returns an empty set when no candidate font could be read; callers must
/// surface that as [`crate::renderer::text::TextError::MissingFont`] instead of
/// measuring without fonts.
pub(crate) fn load_system_fonts() -> Arc<Vec<TextShapingFont>> {
    SYSTEM_TEXT_FONTS.get_or_load(load_system_fonts_uncached)
}

#[cfg(test)]
pub(crate) fn system_font_load_count() -> usize {
    SYSTEM_TEXT_FONTS.load_count()
}

fn load_system_fonts_uncached() -> Vec<TextShapingFont> {
    let mut fonts = Vec::new();
    for (family, path) in FONT_CANDIDATES {
        load_font_faces(family, path, &mut fonts);
    }
    fonts
}

fn load_font_faces(family: &str, path: &str, fonts: &mut Vec<TextShapingFont>) {
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let data = Arc::new(bytes);

    for index in 0..MAX_FACES_PER_FILE {
        let Some(font) = Font::try_from_vec_and_index(data.as_ref().clone(), index) else {
            break;
        };
        if rustybuzz::Face::from_slice(data.as_slice(), index).is_none() {
            break;
        }
        fonts.push(TextShapingFont::new(family, index, data.clone(), font));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_load_is_not_cached_and_is_retried() {
        let set = RetryableResourceSet::<u32>::new();

        let first = set.get_or_load(Vec::new);
        assert!(first.is_empty());
        assert_eq!(set.load_count(), 1);

        // A second caller must get a fresh attempt rather than the empty set.
        let second = set.get_or_load(|| vec![7]);
        assert_eq!(second.as_slice(), [7]);
        assert_eq!(set.load_count(), 2);
    }

    #[test]
    fn successful_load_is_cached_and_shared() {
        let set = RetryableResourceSet::<u32>::new();

        let first = set.get_or_load(|| vec![1, 2, 3]);
        let second =
            set.get_or_load(|| panic!("loader must not run again after a successful load"));

        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(set.load_count(), 1);
    }

    #[test]
    fn system_font_set_is_loaded_at_most_once() {
        let first = load_system_fonts();
        let loads_after_first = system_font_load_count();
        let second = load_system_fonts();

        if first.is_empty() {
            // No candidate font exists on this machine. The retry policy applies:
            // the empty set must not have been cached.
            assert!(
                system_font_load_count() > loads_after_first,
                "an empty font set must stay retryable"
            );
        } else {
            assert!(
                Arc::ptr_eq(&first, &second),
                "loaded fonts should be shared, not re-parsed"
            );
            assert_eq!(
                system_font_load_count(),
                loads_after_first,
                "a successful font load must not repeat"
            );
        }
    }
}
