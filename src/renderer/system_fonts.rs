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
    generation: AtomicUsize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FontLoadError {
    pub(crate) path: String,
    pub(crate) kind: std::io::ErrorKind,
    pub(crate) message: String,
}

impl FontLoadError {
    fn new(path: &str, error: std::io::Error) -> Self {
        Self {
            path: path.to_string(),
            kind: error.kind(),
            message: error.to_string(),
        }
    }
}

pub(crate) struct ResourceLoad<T> {
    resources: Vec<T>,
    cacheable: bool,
    error: Option<FontLoadError>,
}

impl<T> ResourceLoad<T> {
    fn complete(resources: Vec<T>) -> Self {
        Self {
            resources,
            cacheable: true,
            error: None,
        }
    }

    #[cfg(test)]
    fn retryable(resources: Vec<T>) -> Self {
        Self {
            resources,
            cacheable: false,
            error: None,
        }
    }

    fn retryable_with_error(resources: Vec<T>, error: FontLoadError) -> Self {
        Self {
            resources,
            cacheable: false,
            error: Some(error),
        }
    }
}

pub(crate) struct ResourceSnapshot<T> {
    pub(crate) resources: Arc<Vec<T>>,
    pub(crate) retryable: bool,
    pub(crate) generation: usize,
    pub(crate) error: Option<FontLoadError>,
}

impl<T> RetryableResourceSet<T> {
    pub(crate) const fn new() -> Self {
        Self {
            slot: Mutex::new(None),
            loads: AtomicUsize::new(0),
            generation: AtomicUsize::new(0),
        }
    }

    /// Number of times the loader has actually run. Used by tests to prove that
    /// a successful load is not repeated, and that an empty one is retried.
    #[cfg(test)]
    pub(crate) fn load_count(&self) -> usize {
        self.loads.load(Ordering::Relaxed)
    }

    pub(crate) fn get_or_load(
        &self,
        load: impl FnOnce() -> ResourceLoad<T>,
    ) -> ResourceSnapshot<T> {
        // The slot is only written after `load` returns, so a loader that panics
        // cannot leave a half-built value behind: a poisoned lock still guards a
        // valid `Option`. Recovering it keeps one caller's panic from turning
        // every later load into an unrelated panic.
        let mut slot = match self.slot.lock() {
            Ok(slot) => slot,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(cached) = slot.as_ref() {
            return ResourceSnapshot {
                resources: Arc::clone(cached),
                retryable: false,
                generation: self.generation.load(Ordering::Acquire),
                error: None,
            };
        }

        self.loads.fetch_add(1, Ordering::Relaxed);
        let loaded = load();
        let resources = Arc::new(loaded.resources);
        let retryable = resources.is_empty() || !loaded.cacheable;
        let generation = if !retryable {
            *slot = Some(Arc::clone(&resources));
            self.generation.fetch_add(1, Ordering::AcqRel) + 1
        } else {
            self.generation.load(Ordering::Acquire)
        };
        ResourceSnapshot {
            resources,
            retryable,
            generation,
            error: loaded.error,
        }
    }

    pub(crate) fn reload(&self, load: impl FnOnce() -> ResourceLoad<T>) -> ResourceSnapshot<T> {
        let mut slot = match self.slot.lock() {
            Ok(slot) => slot,
            Err(poisoned) => poisoned.into_inner(),
        };
        self.loads.fetch_add(1, Ordering::Relaxed);
        let loaded = load();
        let resources = Arc::new(loaded.resources);
        let retryable = resources.is_empty() || !loaded.cacheable;
        let generation = if !retryable {
            *slot = Some(Arc::clone(&resources));
            self.generation.fetch_add(1, Ordering::AcqRel) + 1
        } else {
            self.generation.load(Ordering::Acquire)
        };
        ResourceSnapshot {
            resources,
            retryable,
            generation,
            error: loaded.error,
        }
    }
}

static SYSTEM_TEXT_FONTS: RetryableResourceSet<TextShapingFont> = RetryableResourceSet::new();

/// Shared system font set for this process, loading it on first use.
///
/// Returns an empty set when no candidate font could be read; callers must
/// surface that as [`crate::renderer::text::TextError::MissingFont`] instead of
/// measuring without fonts.
pub(crate) fn load_system_fonts() -> ResourceSnapshot<TextShapingFont> {
    SYSTEM_TEXT_FONTS.get_or_load(load_system_fonts_uncached)
}

pub(crate) fn system_font_generation() -> usize {
    SYSTEM_TEXT_FONTS.generation.load(Ordering::Acquire)
}

pub(crate) fn reload_system_fonts_for_family(
    family: &str,
) -> Result<Option<ResourceSnapshot<TextShapingFont>>, FontLoadError> {
    let has_available_candidate =
        has_available_candidate_with(family, |path| std::path::Path::new(path).try_exists())?;
    Ok(has_available_candidate.then(|| SYSTEM_TEXT_FONTS.reload(load_system_fonts_uncached)))
}

fn has_available_candidate_with(
    family: &str,
    mut try_exists: impl FnMut(&str) -> std::io::Result<bool>,
) -> Result<bool, FontLoadError> {
    for (candidate_family, path) in FONT_CANDIDATES {
        if candidate_family.eq_ignore_ascii_case(family) {
            match try_exists(path) {
                Ok(true) => return Ok(true),
                Ok(false) => {}
                Err(error) => return Err(FontLoadError::new(path, error)),
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
pub(crate) fn system_font_load_count() -> usize {
    SYSTEM_TEXT_FONTS.load_count()
}

fn load_system_fonts_uncached() -> ResourceLoad<TextShapingFont> {
    let mut fonts = Vec::new();
    let mut first_error = None;
    for (family, path) in FONT_CANDIDATES {
        if let Err(error) = load_font_faces(family, path, &mut fonts)
            && first_error.is_none()
        {
            first_error = Some(error);
        }
    }
    match first_error {
        Some(error) => ResourceLoad::retryable_with_error(fonts, error),
        None => ResourceLoad::complete(fonts),
    }
}

fn load_font_faces(
    family: &str,
    path: &str,
    fonts: &mut Vec<TextShapingFont>,
) -> Result<(), FontLoadError> {
    load_font_faces_with(family, path, fonts, |candidate_path| {
        std::fs::read(candidate_path)
    })
}

fn load_font_faces_with(
    family: &str,
    path: &str,
    fonts: &mut Vec<TextShapingFont>,
    read: impl FnOnce(&str) -> std::io::Result<Vec<u8>>,
) -> Result<(), FontLoadError> {
    let bytes = match read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(FontLoadError::new(path, error)),
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_load_is_not_cached_and_is_retried() {
        let set = RetryableResourceSet::<u32>::new();

        let first = set.get_or_load(|| ResourceLoad::complete(Vec::new()));
        assert!(first.resources.is_empty());
        assert!(first.retryable);
        assert_eq!(set.load_count(), 1);

        // A second caller must get a fresh attempt rather than the empty set.
        let second = set.get_or_load(|| ResourceLoad::complete(vec![7]));
        assert_eq!(second.resources.as_slice(), [7]);
        assert!(!second.retryable);
        assert_eq!(set.load_count(), 2);
    }

    #[test]
    fn partial_load_is_not_cached_and_is_retried() {
        let set = RetryableResourceSet::<u32>::new();

        let first = set.get_or_load(|| ResourceLoad::retryable(vec![1]));
        assert_eq!(first.resources.as_slice(), [1]);
        assert!(first.retryable);

        let second = set.get_or_load(|| ResourceLoad::complete(vec![1, 2]));
        assert_eq!(second.resources.as_slice(), [1, 2]);
        assert!(!second.retryable);
        assert_eq!(set.load_count(), 2);
    }

    #[test]
    fn retrying_caller_reuses_a_snapshot_published_by_another_caller() {
        let set = RetryableResourceSet::<u32>::new();
        let first = set.get_or_load(|| ResourceLoad::complete(Vec::new()));
        assert!(first.retryable);

        let published = set.get_or_load(|| ResourceLoad::complete(vec![9]));
        let recovered = set.get_or_load(|| panic!("a published snapshot must prevent reparsing"));

        assert_eq!(set.load_count(), 2);
        assert_eq!(published.generation, recovered.generation);
        assert!(Arc::ptr_eq(&published.resources, &recovered.resources));
    }

    #[test]
    fn non_not_found_io_error_preserves_path_and_kind() {
        let mut fonts = Vec::new();
        let error = match load_font_faces_with("Test", "/denied/font.ttf", &mut fonts, |_| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "permission denied by test",
            ))
        }) {
            Ok(()) => panic!("permission denied must not be treated as a missing optional font"),
            Err(error) => error,
        };

        assert_eq!(error.path, "/denied/font.ttf");
        assert_eq!(error.kind, std::io::ErrorKind::PermissionDenied);
        assert!(error.message.contains("permission denied by test"));
        assert!(fonts.is_empty());
    }

    #[test]
    fn candidate_metadata_error_preserves_path_and_kind() {
        let error = has_available_candidate_with("Arial", |path| {
            if path.ends_with("Arial.ttf") {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "metadata denied by test",
                ))
            } else {
                Ok(false)
            }
        })
        .expect_err("metadata errors must not look like an unavailable font family");

        assert!(error.path.ends_with("Arial.ttf"));
        assert_eq!(error.kind, std::io::ErrorKind::PermissionDenied);
        assert!(error.message.contains("metadata denied by test"));
    }

    #[test]
    fn successful_load_is_cached_and_shared() {
        let set = RetryableResourceSet::<u32>::new();

        let first = set.get_or_load(|| ResourceLoad::complete(vec![1, 2, 3]));
        let second =
            set.get_or_load(|| panic!("loader must not run again after a successful load"));

        assert!(Arc::ptr_eq(&first.resources, &second.resources));
        assert_eq!(set.load_count(), 1);
    }

    #[test]
    fn system_font_set_is_loaded_at_most_once() {
        let first = load_system_fonts();
        let loads_after_first = system_font_load_count();
        let second = load_system_fonts();

        if first.retryable {
            // No candidate font exists on this machine. The retry policy applies:
            // empty or partial sets must not have been cached.
            assert!(
                system_font_load_count() > loads_after_first,
                "an empty or partial font set must stay retryable"
            );
        } else {
            assert!(
                Arc::ptr_eq(&first.resources, &second.resources),
                "loaded fonts should be shared, not re-parsed"
            );
            assert_eq!(
                system_font_load_count(),
                loads_after_first,
                "a successful font load must not repeat"
            );
            assert_eq!(first.generation, second.generation);
        }
    }
}
